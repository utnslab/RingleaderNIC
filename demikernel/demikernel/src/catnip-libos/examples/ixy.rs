// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#![feature(try_blocks)]
// use crate::memory::{MemoryConfig, MemoryManager};
use anyhow::Error;
use catnip::{
    libos::LibOS,
    operations::OperationResult,
    scheduler::SchedulerHandle,
};
use catnip_libos::memory::Ixybuf;
use must_let::must_let;
use rand::distributions::uniform::SampleBorrow;
use std::{
    borrow::BorrowMut,
    cell::{
        RefCell,
        UnsafeCell,
    },
    collections::HashMap,
    env,
    rc::Rc,
    thread::JoinHandle,
};

use anyhow::format_err;
use catnip::{
    protocols::{
        ethernet2::MacAddress,
        ip::Port,
        ipv4::Endpoint,
    },
    runtime::Runtime,
};
use catnip_libos::runtime::{
    IxyRuntime,
    Ixydev,
};
use histogram::Histogram;
use ixy_rs::{
    deregister_app,
    memory_allocate_mempool,
    mqnic_port_reset_monitor,
    pkt_buf_alloc,
    test_link_success,
};
use std::{
    convert::{
        TryFrom,
        TryInto,
    },
    ffi::CString,
    fs::File,
    io::Read,
    net::Ipv4Addr,
    str::FromStr,
    thread,
    time,
    time::{
        Duration,
        Instant,
        SystemTime,
        UNIX_EPOCH,
    },
};
use yaml_rust::{
    Yaml,
    YamlLoader,
};

use ixy_rs::process_work;

#[derive(Debug)]
pub struct Config {
    pub local_ipv4_addr: Ipv4Addr,
    pub buffer_size: usize,
    pub config_obj: Yaml,
    pub mss: usize,
    pub strict: bool,
    pub udp_checksum_offload: bool,
}

impl Config {
    pub fn initialize(
        config_path: String,
        dev: Ixydev,
        queue_id: u16,
    ) -> Result<(Self, IxyRuntime), Error> {
        let mut config_s = String::new();
        File::open(config_path)?.read_to_string(&mut config_s)?;
        let config = YamlLoader::load_from_str(&config_s)?;
        let config_obj = match &config[..] {
            &[ref c] => c,
            _ => Err(format_err!("Wrong number of config objects"))?,
        };
        let local_ipv4_addr: Ipv4Addr = config_obj["catnip"]["my_ipv4_addr"]
            .as_str()
            .ok_or_else(|| format_err!("Couldn't find my_ipv4_addr in config"))?
            .parse()?;
        if local_ipv4_addr.is_unspecified() || local_ipv4_addr.is_broadcast() {
            Err(format_err!("Invalid IPv4 address"))?;
        }

        let remote_ipv4_addr: Ipv4Addr = config_obj["catnip"]["remote_ipv4_addr"]
            .as_str()
            .ok_or_else(|| format_err!("Couldn't find remote_ipv4_addr in config"))?
            .parse()?;
        if remote_ipv4_addr.is_unspecified() || remote_ipv4_addr.is_broadcast() {
            Err(format_err!("Invalid IPv4 address"))?;
        }

        let remote_link_addr_str = config_obj["catnip"]["remote_mac"]
            .as_str()
            .ok_or_else(|| format_err!("Couldn't find link_addr in config"))?;
        let remote_link_addr = MacAddress::parse_str(remote_link_addr_str)?;
        print!(
            "ARP: remote_ipv4_addr, remote_MAC {}, {}",
            remote_ipv4_addr, remote_link_addr
        );

        let local_link_addr_str = config_obj["catnip"]["local_mac"]
            .as_str()
            .ok_or_else(|| format_err!("Couldn't find link_addr in config"))?;
        let local_link_addr = MacAddress::parse_str(local_link_addr_str)?;

        print!(
            "ARP: local_link_addr, local_MAC {}, {}",
            local_ipv4_addr, local_link_addr
        );

        let mut arp_table = HashMap::new();
        arp_table.insert(remote_ipv4_addr, remote_link_addr);

        let disable_arp = false;

        let use_jumbo_frames = env::var("USE_JUMBO").is_ok();
        let mtu: u16 = env::var("MTU")?.parse()?;
        let mss: usize = env::var("MSS")?.parse()?;
        let udp_checksum_offload = true; // diable checksum
        let strict = env::var("STRICT").is_ok();

        let buffer_size: usize = env::var("BUFFER_SIZE")?.parse()?;

        let runtime = catnip_libos::dpdk::create_runtime(
            // memory,
            local_link_addr,
            local_ipv4_addr,
            arp_table,
            disable_arp,
            use_jumbo_frames,
            mtu,
            mss,
            udp_checksum_offload,
            udp_checksum_offload,
            dev,
            queue_id,
        )?;

        let config = Self {
            local_ipv4_addr,
            buffer_size,
            mss,
            strict,
            udp_checksum_offload,
            config_obj: config_obj.clone(),
        };
        Ok((config, runtime))
    }

    pub fn addr(&self, k1: &str, k2: &str) -> Result<Endpoint, Error> {
        let addr = &self.config_obj[k1][k2];
        let host_s = addr["host"].as_str().ok_or(format_err!("Missing host"))?;
        let host = Ipv4Addr::from_str(host_s)?;
        let port_i = addr["port"].as_i64().ok_or(format_err!("Missing port"))?;
        let port = Port::try_from(port_i as u16)?;
        Ok(Endpoint::new(host, port))
    }

    pub fn new_port(&self, k1: &str, k2: &str, pt: u16) -> Result<Endpoint, Error> {
        let addr = &self.config_obj[k1][k2];
        let host_s = addr["host"].as_str().ok_or(format_err!("Missing host"))?;
        let host = Ipv4Addr::from_str(host_s)?;
        let port_i = addr["port"].as_i64().ok_or(format_err!("Missing port"))?;
        let port = Port::try_from(pt)?;
        Ok(Endpoint::new(host, port))
    }
}

fn run_threads(dev: Ixydev, config_path2: String, queue_id: u16) {
    let (config, runtime) = Config::initialize(config_path2, dev.clone(), queue_id).unwrap();
    let mut client_addr = config.addr("server", "client").unwrap();

    let mut app_count = 0;
    let mut fd_to_appid: HashMap<u32, u16> = HashMap::new();

    let mut libos = LibOS::new(runtime.clone(), queue_id as usize, CORE_COUNT).unwrap();

    // app 1's socket
    let sockfd1 = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();
    // app 2's socket
    let sockfd2 = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();

    // Config app1.
    let app_id_1 = 1;
    let local_addr1 = config.new_port("server", "bind", 5678).unwrap();
    fd_to_appid.insert(sockfd1, app_id_1);

    // Config NIC's MAT pipeline config_app_mat(appid, port number, priority).
    // Smaller priority number means this applilcation has higher priority in the requst scheduler.
    runtime.config_app_mat(app_id_1, 5678, 1);

    // Config NIC's load monitor. Load monitor is not enabled in this testbench.
    // runtime.config_monitor(app_id_1, 10);

    // Config NIC's load balancer. Tell the NIC that this application is running on core X, with priority Y.
    // Note that in the load balancer, higher priority number means that this applilcation has higher priority when calculating the rank.
    // TODO: We should make the load balancer use the same priority order as the scheduler. This should be fixed in the future.
    runtime.register_app(queue_id, app_id_1, 1);

    // Config libos.
    libos.core_alloc_reg_app(queue_id, app_id_1);
    libos.bind(sockfd1, local_addr1).unwrap();
    app_count += 1;

    // Same as app1, config app2.
    let app_id_2 = 2;
    let local_addr2 = config.new_port("server", "bind", 1234).unwrap();
    fd_to_appid.insert(sockfd2, app_id_2);
    runtime.config_app_mat(app_id_2, 1234, 2);
    // runtime.config_monitor(app_id_2, 10);
    libos.core_alloc_reg_app(queue_id, app_id_2);
    runtime.register_app(queue_id, app_id_2, 0);
    libos.bind(sockfd2, local_addr2).unwrap();
    app_count += 1;

    let ten_millis = time::Duration::from_millis(10);
    thread::sleep(ten_millis);

    let mut hi_qtokens = Vec::new();
    let mut lo_qtokens = Vec::new();

    // insert application's sche token into libos
    // app 1's request has priority 0, which is the highest priority
    hi_qtokens.push(libos.popbatch(sockfd1, 0).unwrap());
    // app 2's request has priority 2, which is the lowest priority
    lo_qtokens.push(libos.popprio(sockfd2, 2).unwrap());
    // when app 2 yield, the uncompleted request will have higher priority than app 2's other requsts.
    // Thus priorty 1 is reserved for yielded requests.

    let mut context: Vec<(Endpoint, Ixybuf)> = Vec::with_capacity(1);

    let mut count = 0;
    let mut start = Instant::now();
    let mut pkcounter: u64 = 0;
    let mut batch_count: u64 = 0;

    let mut if_yield = 0;

    // Coroutine
    loop {
        // pop the next request
        let (qtoken_group, i, fd, result, hint_array) =
            libos.prio_wait_any2(&queue_id, &hi_qtokens, &lo_qtokens);

        // do core allocation processing
        for k in 0..hint_array.len() {
            println!("Warning, in this test should not receive core allocation hints.");
        }

        if (qtoken_group == 100) {
            // meaning this is the core allocator message, no packet is carried
            continue;
        } else if (qtoken_group == 0) {
            // if qtoken_group == 0 means this request is from hi_qtokens.
            hi_qtokens.swap_remove(i);
        } else {
            // if qtoken_group == 1 means this request is from lo_qtokens.
            lo_qtokens.swap_remove(i);
        }

        match result {
            OperationResult::Pop(sender, buf) => {
                assert!(if_yield == 0);
                // we assume only app 2 is using pop, app 1 is using pop batch
                assert!(qtoken_group == 1);

                // App 1 could preempt App 2, so app 2 need yield back to coroutine every fixed interval.
                let mut if_finish: u8 = 0;
                unsafe {
                    if_finish =
                        process_work(buf.buf_addr_phy(), 1 as u8, (app_count > 1) as u8, 5000);
                }

                // If this request completes.
                if (if_finish == 1) {
                    match sender {
                        Some(endpoint) => client_addr.port = endpoint.port,
                        None => todo!(),
                    }
                    let app_id = fd_to_appid.get(fd.borrow()).unwrap();
                    let size = buf.data_length;

                    // reinsert token.
                    lo_qtokens.push(libos.popprio(fd, 2).unwrap());

                    // send response
                    libos.directpushto2(fd, buf, client_addr).unwrap();

                    // load feedback to NIC
                    runtime.send_app_feedback(queue_id, *app_id, 1);
                    count = count + 1;
                    pkcounter = pkcounter + 1;
                } else {
                    // If this request not complete.
                    if_yield = 1;
                    match sender {
                        Some(endpoint) => {
                            //save context
                            context.push((endpoint, buf));
                            // we assume only app 2 will yield
                            // reinsert dyield token, which priority is 1 (higher priority than app2's new requet).
                            lo_qtokens.push(libos.dyield(fd, 1).unwrap());
                        },
                        None => todo!(),
                    }
                }
            },
            OperationResult::PopBatch(bufs) => {
                // we assume only app 1 is using pop batch, app 2 is using pop
                assert!(qtoken_group == 0);

                let bn = bufs.len() as u16;
                let app_id = fd_to_appid.get(fd.borrow()).unwrap();

                // reinsert token.
                hi_qtokens.push(libos.popbatch(fd, 0).unwrap());

                let mut send_buf: Vec<(Endpoint, Ixybuf)> = Vec::with_capacity(bufs.len());
                let mut batched_feedback = 0;

                for (sender, buf) in bufs {
                    unsafe {
                        process_work(buf.buf_addr_phy(), 1 as u8, 0 as u8, 0 as u32);
                    }
                    match sender {
                        Some(endpoint) => {
                            let x: u16 = endpoint.port().into();
                            client_addr.port = Port::try_from(x & 0xFF00).unwrap()
                        },
                        None => todo!(),
                    }
                    batched_feedback = batched_feedback + 1;
                    if (batched_feedback >= 2) {
                        runtime.send_app_feedback(queue_id, *app_id, batched_feedback);
                        batched_feedback = 0;
                    }

                    send_buf.push((client_addr, buf));
                }
                if (batched_feedback > 0) {
                    runtime.send_app_feedback(queue_id, *app_id, batched_feedback);
                }
                // send response in batch
                libos.directbatchpushto2(fd, send_buf);

                batch_count += 1;
                count = count + bn;
                pkcounter = pkcounter + bn as u64;
            },
            OperationResult::Dyield => {
                // process those incomplete request (from app 2)
                assert!(if_yield == 1);

                let r = context.get(0).unwrap();
                let buf = &(*r).1;

                let mut if_finish: u8 = 0;
                unsafe {
                    if_finish = process_work(buf.buf_addr_phy(), 1 as u8, 1 as u8, 5000);
                }

                if (if_finish == 1) {
                    let r = context.pop().unwrap();
                    let sender = (r).0;
                    let buf = (r).1;

                    client_addr.port = sender.port;

                    let app_id = fd_to_appid.get(fd.borrow()).unwrap();
                    let size = buf.data_length;

                    lo_qtokens.push(libos.popprio(fd, 2).unwrap());

                    libos.directpushto2(fd, buf, client_addr).unwrap();

                    runtime.send_app_feedback(queue_id, *app_id, 1);
                    count = count + 1;
                    pkcounter = pkcounter + 1;
                    if_yield = 0;
                } else {
                    if_yield = 1;
                    lo_qtokens.push(libos.dyield(fd, 1).unwrap());
                }
            },
            OperationResult::Push => {
                // println!("Success Push {}", count);
            },
            r => panic!("{:?}", r),
        }

        if (pkcounter > 1000000) {
            let since_the_epoch = start.elapsed();
            let x = (pkcounter as f64) * 1.0;
            let time =
                since_the_epoch.as_secs() * 1000000 + since_the_epoch.subsec_nanos() as u64 / 1000;
            println!(
                "Queue{}, {}, {} Throughput: {} Mpps, batch size {}",
                queue_id,
                x,
                time,
                x / (time as f64),
                (pkcounter as f32 / batch_count as f32)
            );
            batch_count = 0;
            pkcounter = 0;

            start = Instant::now();
        }
    }
}

const CORE_COUNT: u16 = 32;

fn main() -> Result<(), Error> {
    let config_path = env::args()
        .nth(1)
        .ok_or(format_err!("Config path is first argument"))?;

    let mut config_s = String::new();
    File::open(config_path)?.read_to_string(&mut config_s)?;
    let config = YamlLoader::load_from_str(&config_s)?;
    let config_obj = match &config[..] {
        &[ref c] => c,
        _ => Err(format_err!("Wrong number of config objects"))?,
    };

    let pcie_addr_str = config_obj["catnip"]["pcie"]
        .as_str()
        .ok_or_else(|| format_err!("Couldn't find PCIE addr in config"))?;

    let dev =
        catnip_libos::dpdk::initialze_ixy(CORE_COUNT, CORE_COUNT, CString::new(pcie_addr_str)?);

    unsafe { mqnic_port_reset_monitor(dev.ptr) }
    for i in 0..CORE_COUNT {
        unsafe {
            deregister_app(dev.ptr, i, 1);
            deregister_app(dev.ptr, i, 2);
        }
    }
    let niters: usize = env::var("NUM_ITERS")?.parse()?;

    let mut cores: Vec<JoinHandle<()>> = Vec::new();

    let i = 0;
    for i in 0..CORE_COUNT {
        let config_path = env::args()
            .nth(1)
            .ok_or(format_err!("Config path is first argument"))?;

        let mut alice = thread::spawn(move || {
            run_threads(dev.clone(), config_path, i);
        });
        cores.push(alice);
        let ten_millis = time::Duration::from_millis(10);
        thread::sleep(ten_millis);
    }

    let head = cores.pop().unwrap();

    head.join().unwrap();
    println!("Afte alice join");
    // bob.join().unwrap();

    Ok(())
}
