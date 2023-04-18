use crate::runtime::IxyRuntime;
use crate::runtime::Ixydev;
use anyhow::{bail, format_err, Error};
use catnip::protocols::ethernet2::MacAddress;
use std::collections::HashMap;
use std::{ffi::CString, mem::MaybeUninit, net::Ipv4Addr, ptr, time::Duration};
use ixy_rs::{
    test_link_success,memory_allocate_mempool,pkt_buf_alloc,ixy_init,
};

pub fn initialze_ixy(
    tx_queue_count: u16,
    rx_queue_count: u16,
    pci_addr: CString,
) -> Ixydev{
    
    let dev = unsafe { ixy_init(pci_addr.as_ptr(), tx_queue_count, rx_queue_count, 0)};
    Ixydev{ptr: dev,}

}

pub fn create_runtime(
    // memory_manager: MemoryManager,
    local_link_addr: MacAddress,
    local_ipv4_addr: Ipv4Addr,
    arp_table: HashMap<Ipv4Addr, MacAddress>,
    disable_arp: bool,
    use_jumbo_frames: bool,
    mtu: u16,
    mss: usize,
    tcp_checksum_offload: bool,
    udp_checksum_offload: bool,
    dev: Ixydev,
    queue_id: u16,

) -> Result<IxyRuntime, Error> {

    Ok(IxyRuntime::new(
        local_link_addr,
        local_ipv4_addr,
        arp_table,
        disable_arp,
        mss,
        tcp_checksum_offload,
        udp_checksum_offload,
        dev,
        queue_id,
    ))
}
