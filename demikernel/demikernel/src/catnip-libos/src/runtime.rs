use crate::memory::{Ixybuf};
use arrayvec::ArrayVec;
use catnip::{
    collections::bytes::{Bytes, BytesMut},
    interop::{dmtr_sgarray_t, dmtr_sgaseg_t},
    protocols::{arp, ethernet2::frame::MIN_PAYLOAD_SIZE, ethernet2::MacAddress, tcp, udp},
    runtime::RuntimeBuf,
    runtime::{PacketBuf, Runtime, RECEIVE_BATCH_SIZE},
    scheduler::{Operation, Scheduler, SchedulerHandle},
    timer::{Timer, TimerPtr, WaitFuture},
};
use futures::FutureExt;
use rand::{
    distributions::{Distribution, Standard},
    rngs::SmallRng,
    seq::SliceRandom,
    Rng, SeedableRng,
};

use std::{collections::HashMap, intrinsics::transmute};
use std::{
    cell::RefCell,
    future::Future,
    mem,
    mem::MaybeUninit,
    net::Ipv4Addr,
    ptr,
    rc::Rc,
    slice,
    time::{Duration, Instant},
};
use std::os::raw::{c_char, c_int, c_void};
use ixy_rs::{
    ixy_tx_batch,memory_allocate_mempool,ixy_rx_batch, ixy_rx_batch_hints, mqnic_port_reset_monitor, mqnic_port_set_monitor, mqnic_rearm_monitor, ixy_init,pkt_buf_alloc,pkt_buf_free, ixy_device, mempool, nic_hints,pkt_buf,register_app, mqnic_rx_feedback,config_app_mat,mqnic_rearm_scale_down_monitor,
};


#[derive(Clone)]
pub struct TimerRc(Rc<Timer<TimerRc>>);

impl TimerPtr for TimerRc {
    fn timer(&self) -> &Timer<Self> {
        &*self.0
    }
}

#[derive(Clone)]
pub struct IxyRuntime {
    inner: Rc<RefCell<Inner>>,
    scheduler: Scheduler<Operation<Self>>,
}

impl IxyRuntime {
    pub fn new(
        link_addr: MacAddress,
        ipv4_addr: Ipv4Addr,
        arp_table: HashMap<Ipv4Addr, MacAddress>,
        disable_arp: bool,
        mss: usize,
        tcp_checksum_offload: bool,
        udp_checksum_offload: bool,
        device: Ixydev,
        queue_id: u16,
    ) -> Self {
        let mut rng = rand::thread_rng();
        let rng = SmallRng::from_rng(&mut rng).expect("Failed to initialize RNG");
        let now = Instant::now();

        let arp_options = arp::Options::new(
            Duration::from_secs(15),
            Duration::from_secs(20),
            5,
            arp_table,
            disable_arp,
        );

        let mut tcp_options = tcp::Options::default();
        tcp_options.advertised_mss = mss;
        tcp_options.window_scale = 5;
        tcp_options.receive_window_size = 0xffff;
        tcp_options.tx_checksum_offload = tcp_checksum_offload;
        tcp_options.rx_checksum_offload = tcp_checksum_offload;

        let mut udp_options = udp::Options::new(udp_checksum_offload, udp_checksum_offload);
        let mempool = unsafe {memory_allocate_mempool(1024, 2048)};

        let mut nic_hints_array: [nic_hints; RECEIVE_BATCH_SIZE] = unsafe { mem::zeroed() };
        let inner = Inner {
            timer: TimerRc(Rc::new(Timer::new(now))),
            link_addr,
            ipv4_addr,
            rng,
            arp_options,
            tcp_options,
            udp_options,
            device,
            mempool,
            queue_id,
            nic_hints_array,
        };
        Self {
            inner: Rc::new(RefCell::new(inner)),
            scheduler: Scheduler::new(),
        }
    }

    pub fn register_app(&self, queue_id:u16, app_id:u16, priority:u8){
        let mut inner = self.inner.borrow_mut();
        unsafe{
            register_app(
                inner.device.ptr,
                queue_id,
                app_id,
                priority,
            )
        }
    }


    pub fn reset_all_monitors(&self){
        let mut inner = self.inner.borrow_mut();
        unsafe{
            mqnic_port_reset_monitor(
                inner.device.ptr,
            )
        }
    }

    pub fn config_monitor(&self, app_id: u16, cong_eopch_log: u8, scale_down_epoch_log: u8, scale_down_thresh: u8,){
        let mut inner = self.inner.borrow_mut();
        unsafe{
            mqnic_port_set_monitor(
                inner.device.ptr,
                app_id,
                cong_eopch_log,
                scale_down_epoch_log,
                scale_down_thresh,
            )
        }
    }


    pub fn rearm_monitor(&self, queue_id:u16, app_id:u16,){
        let mut inner = self.inner.borrow_mut();
        unsafe{
            mqnic_rearm_monitor(
                inner.device.ptr,
                queue_id,
                app_id,
            )
        }
    }

    pub fn rearm_scale_down_monitor(&self, queue_id:u16, app_id:u16,){
        let mut inner = self.inner.borrow_mut();
        unsafe{
            mqnic_rearm_scale_down_monitor(
                inner.device.ptr,
                queue_id,
                app_id,
            )
        }
    }

    
    
    pub fn config_app_mat(&self, app_id:u16, port_id:u16, priority:u8){
        let mut inner = self.inner.borrow_mut();
        unsafe{
            config_app_mat(
                inner.device.ptr,
                app_id,
                port_id,
                priority,
            )
        }
    }

    pub fn send_app_feedback(&self, queue_id:u16, app_id:u16, update_count:u16){
        let mut inner = self.inner.borrow_mut();
        unsafe{
            mqnic_rx_feedback(
                inner.device.ptr,
                queue_id,
                app_id,
                update_count,
            )
        }
    }
}

struct Inner {
    timer: TimerRc,
    link_addr: MacAddress,
    ipv4_addr: Ipv4Addr,
    rng: SmallRng,
    arp_options: arp::Options,
    tcp_options: tcp::Options<IxyRuntime>,
    udp_options: udp::Options,
    device: Ixydev,
    mempool: *mut mempool,
    queue_id: u16,
    nic_hints_array:  [nic_hints; RECEIVE_BATCH_SIZE],
}


#[derive(Debug, Clone,Copy)]
pub struct Ixydev {
    pub ptr: *mut ixy_device,
}

impl Ixydev {
    pub fn get_dev(&self) ->  *mut ixy_device {
        unsafe {
            let ptr = self.ptr;
            ptr
        }
    }

}

unsafe impl Send for Ixydev {}

// #[derive(Debug)]
// pub struct Ixybuf {
//     pub ptr: *mut pkt_buf,
// }

// impl RuntimeBuf for Ixybuf{
//     fn empty() -> Self {
//         todo!()
//     }

//     fn from_slice(_: &[u8]) -> Self {
//         todo!()
//     }

//     fn adjust(&mut self, num_bytes: usize) {
//         todo!()
//     }

//     fn trim(&mut self, num_bytes: usize) {
//         todo!()
//     }
// }

// impl Clone for Ixybuf {
//     fn clone(&self) -> Self {
//         todo!()
//     }
// }

// impl Deref for Ixybuf {
//     type Target = [u8];

//     fn deref(&self) -> &[u8] {
//         todo!()
//     }
// }

// impl Drop for Ixybuf {
//     fn drop(&mut self) {
//         if self.ptr.is_null() {
//             return;
//         }
//         unsafe {
//             todo!()
//         }
//         self.ptr = ptr::null_mut();
//     }
// }
// impl Ixybuf {
//     pub fn buf_addr_phy(&self) -> *mut u8 {
//         unsafe {
//             let buf_ptr = (*self.ptr).buf_addr_phy as *mut u8;
//             buf_ptr
//         }
//     }


//     pub fn size(&self) -> u32 {
//         unsafe {
//             let buf_size = (*self.ptr).size as u32;
//             buf_size
//         }
//     }
// }

impl Runtime for IxyRuntime {
    type WaitFuture = WaitFuture<TimerRc>;
 
    type Buf =  Ixybuf;
    fn into_sgarray(&self, buf: Self::Buf) -> dmtr_sgarray_t {
        println!("into_sgarray\n");
        // let buf_copy: Box<[u8]> = (&buf[..]).into();
        // let ptr = Box::into_raw(buf_copy);
        let sgaseg = dmtr_sgaseg_t {
            sgaseg_buf: buf.buf_addr_phy() as *mut _,
            sgaseg_len: buf.len() as u32,
        };
        dmtr_sgarray_t {
            sga_buf: buf.ptr as *mut c_void,
            sga_numsegs: 1,
            sga_segs: [sgaseg],
            sga_addr: unsafe { mem::zeroed() },
        }
    }

    fn alloc_sgarray(&self, size: usize) -> dmtr_sgarray_t {
        println!("alloc_sgarray\n");
        let mempool = self.inner.borrow().mempool;
        // TODO uplimit max size
        assert!(size < 2048, "alloc_sgarray size error!");
        
        let buf = unsafe{ pkt_buf_alloc(mempool) };
        unsafe{ (*buf).size =  size as u32 };
        let data_ptr = unsafe{ (*buf).buf_addr_phy };

        let sgaseg = dmtr_sgaseg_t {
            sgaseg_buf: data_ptr as *mut _,
            sgaseg_len: size as u32,
        };
        dmtr_sgarray_t {
            sga_buf: buf as *mut c_void,
            sga_numsegs: 1,
            sga_segs: [sgaseg],
            sga_addr: unsafe { mem::zeroed() },
        }
    }

    fn free_sgarray(&self, sga: dmtr_sgarray_t) {
        panic!("Error Free");
        println!("free_sgarray\n");
        assert_eq!(sga.sga_numsegs, 1);
        let sgaseg = sga.sga_segs[0];
        let buf = sga.sga_buf as *mut pkt_buf;
        let (ptr, len) = (sgaseg.sgaseg_buf, sgaseg.sgaseg_len as usize);
        // TODO, we can change memory.h interface to allow free using id.
 
        unsafe {pkt_buf_free(buf);};
    }

    fn clone_sgarray(&self, sga: &dmtr_sgarray_t) -> Self::Buf {
        // TODO, unimplemented clone
        println!("clone_sgarray\n");
        todo!();
        assert_eq!(sga.sga_numsegs, 1);
        let sgaseg = sga.sga_segs[0];
        let (ptr, len) = (sgaseg.sgaseg_buf, sgaseg.sgaseg_len as usize);

        let mempool = self.inner.borrow().mempool;
        let buf = unsafe{ pkt_buf_alloc(mempool) };

        let mut ixy = Ixybuf{ptr: buf, data_offset: 0, data_length: 0};
        ixy.data_length = ixy.bufsize() as usize;
        ixy
    }
    

    fn transmit(&self, buf: impl PacketBuf<Self::Buf>) {
        // todo!();
        // Alloc header mbuf, check header size.
        // Serialize header.
        // Decide if we can inline the data --
        //   1) How much space is left?
        //   2) Is the body small enough?
        // If we can inline, copy and return.
        // If we can't inline...
        //   1) See if the body is managed => take
        //   2) Not managed => alloc body
        // Chain body buffer.
        // print!("body size??{}", buf.body_size());
        // print!("header size??{}", buf.header_size());

        let inner = self.inner.borrow_mut();
        let mempool = inner.mempool;
        let header_size = buf.header_size();
        if(buf.if_batch() && buf.has_body()){
            unsafe {
                let bodys: &mut Vec<Ixybuf> = &mut *(buf.get_batch());
                let mut packets: [*mut pkt_buf; 64] = unsafe { mem::zeroed() };
                let mut count = 0;
                for body in bodys{
                    let header_space = body.data_offset;
                    
                    body.forward_offset(header_size);
                    buf.write_header_index( &mut body.slice_mut()[..header_size], count);
                    body.update_length(header_size);
                    let body_size = body.len();
                    body.setbufsize(body_size);
                    packets[count] = body.ptr;
                    count += 1;

                }

                // println!("Total Size:{}", body_size);
                let num_sent = unsafe { ixy_tx_batch(inner.device.ptr, inner.queue_id, packets.as_mut_ptr(), count as u32) };
                // drop(buf);
            };
        }
        else if (buf.has_body()){
            unsafe {
                let body: &mut Ixybuf = &mut *(buf.get_body());
                let header_space = body.data_offset;
                // println!(" Size:{}, {}", buf.body_size(), buf.header_size());
                // assert!(header_size == header_space);
                body.forward_offset(header_size);
                buf.write_header( &mut body.slice_mut()[..header_size] );
                body.update_length(header_size);
                let body_size = body.len();
                // if  body_size< MIN_PAYLOAD_SIZE {
                    // let padding_bytes = MIN_PAYLOAD_SIZE - body_size;
                //     let padding_buf =
                //         unsafe { &mut body.slice_mut()[body_size..][..padding_bytes] };
                //     for byte in padding_buf {
                //         *byte = 0;
                //     }
                // }
                body.setbufsize(body_size);
                // println!("Total Size:{}", body_size);
                let num_sent = unsafe { ixy_tx_batch(inner.device.ptr, inner.queue_id, &mut body.ptr, 1) };
                // drop(buf);
            };
            
        }
        else{
            panic!{"Warning! transmit"};
            let hb = unsafe{ pkt_buf_alloc(mempool) };
            let mut header_buf = Ixybuf{ptr: hb, data_offset: 0, data_length: 2048};

            buf.write_header(unsafe { &mut header_buf.slice_mut()[..header_size] });

            if header_size < MIN_PAYLOAD_SIZE {
                let padding_bytes = MIN_PAYLOAD_SIZE - header_size;
                let padding_buf =
                    unsafe { &mut header_buf.slice_mut()[header_size..][..padding_bytes] };
                for byte in padding_buf {
                    *byte = 0;
                }
            }
            let frame_size = std::cmp::max(header_size, MIN_PAYLOAD_SIZE);
            header_buf.trim(header_buf.len() - frame_size);
            let mut header_buf_ptr = header_buf.ptr;
            header_buf.setbufsize(frame_size);
            // println!("Size:{}", frame_size);
            let num_sent = unsafe {ixy_tx_batch(inner.device.ptr, inner.queue_id, &mut header_buf_ptr, 1) };
            assert_eq!(num_sent, 1);
        }
    }


    
    fn transmit_batch(&self, bufs: Vec<impl PacketBuf<Self::Buf>>) {
        let inner = self.inner.borrow_mut();
        let mempool = inner.mempool;
        assert!(bufs.len() < 64);
        let mut packets: [*mut pkt_buf; 64] = unsafe { mem::zeroed() };
        let mut count = 0;
        // for i in 0..bufs.len(){
        //     let buf = bufs.remove(0);
            
        // }
        for buf in &bufs{
            
            let header_size = buf.header_size();
            unsafe {
                let body: &mut Ixybuf = &mut *(buf.get_body());
                let header_space = body.data_offset;
                body.forward_offset(header_size);
                buf.write_header( &mut body.slice_mut()[..header_size] );
                body.update_length(header_size);
                let body_size = body.len();

                body.setbufsize(body_size);
                // println!("Bufsize {}", body_size);
                packets[count] = body.ptr;
                count += 1;

                // savebuf.push(buf as Ixybuf);
            };
        }

       
        let num_sent = unsafe { ixy_tx_batch(inner.device.ptr, inner.queue_id, packets.as_mut_ptr(), count as u32) };
    
        // for buf in bufs{
        //     println!("Sent {}", count);
        // }
        // assert_eq!(num_sent, count as u32);
    }

    fn receive(&self) -> (ArrayVec<(u16, u16),RECEIVE_BATCH_SIZE>, ArrayVec<Self::Buf, RECEIVE_BATCH_SIZE>) {
        let mut inner = self.inner.borrow_mut();
        let mut out = ArrayVec::new();

        let mut packets: [*mut pkt_buf; RECEIVE_BATCH_SIZE] = unsafe { mem::zeroed() };
        let mut hint_counts:u16 = 0;

        let nb_rx = unsafe {
            ixy_rx_batch_hints(
                inner.device.ptr,
                inner.queue_id,
                packets.as_mut_ptr(),
                RECEIVE_BATCH_SIZE as u32,
                1 as u16,
                inner.nic_hints_array.as_mut_ptr(),
                &mut hint_counts as *mut u16,
            )
        };
        let mut hints_array: ArrayVec<(u16, u16),RECEIVE_BATCH_SIZE> = ArrayVec::new();
        for i in 0..hint_counts{
            let tmp_app_id = inner.nic_hints_array[i as usize].hint_app_id as u16;
            let tmp_hint_type = inner.nic_hints_array[i as usize].hint_content as u16;
            // println!("Success Get NIC HINTS: core: {}, app_id: {}, hint type: {}", inner.queue_id, tmp_app_id, tmp_hint_type);
            
            hints_array.push((tmp_app_id, tmp_hint_type));
            // println!("hint array 0 is {} and len is {}", hints_array[0]., hints_array.len());
                //unsafe{
              //  mqnic_rearm_monitor(
                //    inner.device.ptr,
                 //   inner.queue_id,
                 //   inner.nic_hints_array[i as usize].hint_app_id as u16,
               // )
           // }
            // self.rearm_monitor(inner.queue_id, inner.nic_hints_array[i as usize].hint_app_id as u16);
        }
        // if nb_rx > 0{
        //     println!("*** {} receive {} packets\n",  inner.queue_id, nb_rx);
        // }
        assert!(nb_rx as usize <= RECEIVE_BATCH_SIZE);

        for &packet in &packets[..nb_rx as usize] {
            let mut ixy = Ixybuf{ptr: packet, data_offset: 0, data_length: 0};
            ixy.data_length = ixy.bufsize() as usize;

            out.push(ixy);
            
        }
        (hints_array, out)
    }

    fn local_link_addr(&self) -> MacAddress {
        self.inner.borrow().link_addr.clone()
    }

    fn local_ipv4_addr(&self) -> Ipv4Addr {
        self.inner.borrow().ipv4_addr.clone()
    }

    fn tcp_options(&self) -> tcp::Options<Self> {
        self.inner.borrow().tcp_options.clone()
    }

    fn udp_options(&self) -> udp::Options {
        self.inner.borrow().udp_options.clone()
    }

    fn arp_options(&self) -> arp::Options {
        self.inner.borrow().arp_options.clone()
    }

    fn advance_clock(&self, now: Instant) {
        self.inner.borrow_mut().timer.0.advance_clock(now);
    }

    fn wait(&self, duration: Duration) -> Self::WaitFuture {
        let self_ = self.inner.borrow_mut();
        let now = self_.timer.0.now();
        self_
            .timer
            .0
            .wait_until(self_.timer.clone(), now + duration)
    }

    fn wait_until(&self, when: Instant) -> Self::WaitFuture {
        let self_ = self.inner.borrow_mut();
        self_.timer.0.wait_until(self_.timer.clone(), when)
    }

    fn now(&self) -> Instant {
        self.inner.borrow().timer.0.now()
    }

    fn rng_gen<T>(&self) -> T
    where
        Standard: Distribution<T>,
    {
        let mut self_ = self.inner.borrow_mut();
        self_.rng.gen()
    }

    fn rng_shuffle<T>(&self, slice: &mut [T]) {
        let mut inner = self.inner.borrow_mut();
        slice.shuffle(&mut inner.rng);
    }

    fn spawn<F: Future<Output = ()> + 'static>(&self, future: F) -> SchedulerHandle {
        self.scheduler
            .insert(Operation::Background(future.boxed_local()))
    }

    fn scheduler(&self) -> &Scheduler<Operation<Self>> {
        &self.scheduler
    }
}
