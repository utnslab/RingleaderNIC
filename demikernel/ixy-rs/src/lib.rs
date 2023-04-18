// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[feature(mlx5)]
mod bindings;
use std::os::raw::{c_char, c_int};

pub use bindings::*;


#[link(name = "inlined")]
extern "C" {
    fn memory_allocate_mempool_(        
        num_entries: u32,
        entry_size: u32,) -> *mut mempool;
    fn pkt_buf_alloc_(
        mempool: *mut mempool) -> *mut pkt_buf;
    fn pkt_buf_free_(buf: *mut pkt_buf);
    fn test_link_success_();

    fn ixy_rx_batch_(   
        dev: *mut ixy_device,
        queue_id: u16,
        bufs: *mut *mut pkt_buf,
        num_bufs: u32,) -> u32;

    fn ixy_tx_batch_(   
        dev: *mut ixy_device,
        queue_id: u16,
        bufs: *mut *mut pkt_buf,
        num_bufs: u32,) -> u32;
    
    fn ixy_init_(
        pci_addr: *const c_char,
        rx_queues: u16,
        tx_queues: u16,
        interrupt_timeout: u32,
    ) -> *mut ixy_device;

    // fn mqnic_fill_rx_buffers_(
    //     dev: *mut ixy_device,
    //     queue_id: u16,
    //     num: u32,
    // );

    // fn mqnic_refill_rx_buffers_(
    //     dev: *mut ixy_device,
    //     queue: u16,
    // );
    fn register_app_(
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
        priority: u8,
    );

    fn deregister_app_(
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
    );


    fn mqnic_rx_feedback_(
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
        update_count: u16,
    );

    fn config_app_mat_(
        dev: *mut ixy_device,
        app_id: u16,
        port_num: u16,
        priority: u8,
    );

    fn process_work_(
        data: *mut u8,
        enable_work: u8,
        if_preemptive: u8,
        p_interval: u32,
    )-> u8;

    fn ixy_rx_batch_hints_(   
        dev: *mut ixy_device,
        queue_id: u16,
        bufs: *mut *mut pkt_buf,
        num_bufs: u32,
        if_hint: u16,
        hints: *mut nic_hints,
        hint_count: *mut u16) -> u32;

    fn mqnic_port_reset_monitor_(
        dev: *mut ixy_device,
    );

    fn mqnic_port_set_monitor_(
        dev: *mut ixy_device,
        app_id: u16,
        cong_eopch_log: u8,
        scale_down_eopch_log: u8,
        scale_down_thresh: u8,
    );

    fn mqnic_rearm_monitor_(
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
    );

    fn mqnic_rearm_scale_down_monitor_(
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
    );
}
#[inline]
pub unsafe fn memory_allocate_mempool(
    num_entries: u32,
    entry_size: u32,) -> *mut mempool {
    memory_allocate_mempool_(num_entries, entry_size)
}

#[inline]
pub unsafe fn pkt_buf_alloc(mempool: *mut mempool) -> *mut pkt_buf {
    pkt_buf_alloc_(mempool)
}

#[inline]
pub unsafe fn pkt_buf_free(buf: *mut pkt_buf) {
    pkt_buf_free_(buf)
}

#[inline]
pub unsafe fn test_link_success() {
    test_link_success_()
}

#[inline]
pub unsafe fn ixy_rx_batch(   
    dev: *mut ixy_device,
    queue_id: u16,
    bufs: *mut *mut pkt_buf,
    num_bufs: u32,) -> u32{
        ixy_rx_batch_(dev, queue_id, bufs, num_bufs)

}

#[inline]
pub unsafe fn ixy_init(
    pci_addr: *const c_char,
    rx_queues: u16,
    tx_queues: u16,
    interrupt_timeout: u32,
) -> *mut ixy_device{
    ixy_init_(pci_addr, rx_queues, tx_queues, interrupt_timeout)
}

#[inline]
pub unsafe fn ixy_tx_batch(   
    dev: *mut ixy_device,
    queue_id: u16,
    bufs: *mut *mut pkt_buf,
    num_bufs: u32,) -> u32{
        ixy_tx_batch_(dev, queue_id, bufs, num_bufs)
    }


#[inline]
    pub unsafe fn register_app(   
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
        priority: u8,){
            register_app_(dev, queue_id, app_id, priority)
        }

#[inline]
    pub unsafe fn deregister_app(   
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,){
            deregister_app_(dev, queue_id, app_id)
        }

#[inline]
    pub unsafe fn mqnic_rx_feedback(   
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
        update_count: u16,){
            mqnic_rx_feedback_(dev, queue_id, app_id, update_count)
        }

#[inline]
    pub unsafe fn config_app_mat(   
        dev: *mut ixy_device,
        app_id: u16,
        port_num: u16,
        priority: u8,){
            config_app_mat_(dev, app_id, port_num, priority)
        }
    

#[inline]
    pub unsafe fn process_work(
        data: *mut u8,
        enable_work: u8,
        if_preemptive: u8,
        p_interval: u32,
    ) -> u8{
        process_work_(data,enable_work, if_preemptive, p_interval)
    }

#[inline]
    pub unsafe fn ixy_rx_batch_hints(   
        dev: *mut ixy_device,
        queue_id: u16,
        bufs: *mut *mut pkt_buf,
        num_bufs: u32,
        if_hint: u16,
        hints: *mut nic_hints,
        hint_count: *mut u16) -> u32{
            ixy_rx_batch_hints_(dev, queue_id, bufs, num_bufs, if_hint, hints, hint_count)
        }

#[inline]
    pub unsafe fn mqnic_port_reset_monitor(
            dev: *mut ixy_device,
        ){
            mqnic_port_reset_monitor_(dev)
        }

    #[inline]
    pub unsafe fn mqnic_port_set_monitor(
        dev: *mut ixy_device,
        app_id: u16,
        cong_eopch_log: u8,
        scale_down_eopch_log: u8,
        scale_down_thresh: u8,
    ){
        mqnic_port_set_monitor_(dev, app_id, cong_eopch_log, scale_down_eopch_log, scale_down_thresh)
    }

    #[inline]
    pub unsafe fn mqnic_rearm_monitor(
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
    ){
        mqnic_rearm_monitor_(dev, queue_id, app_id)
    }

    #[inline]
    pub unsafe fn mqnic_rearm_scale_down_monitor(
        dev: *mut ixy_device,
        queue_id: u16,
        app_id: u16,
    ){
        mqnic_rearm_monitor_(dev, queue_id, app_id)
    }