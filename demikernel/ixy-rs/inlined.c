/*
 * Copyright (c) Microsoft Corporation.
 * Licensed under the MIT license.
 */
#include "stats.h"
#include "log.h"
#include "memory.h"
#include "msg.h"
#include "driver/device.h"
#include "driver/mqnic_type.h"

struct mempool* memory_allocate_mempool_(uint32_t num_entries, uint32_t entry_size){
    return memory_allocate_mempool(num_entries, entry_size);
}

struct pkt_buf* pkt_buf_alloc_(struct mempool* mempool){
    return pkt_buf_alloc(mempool);
}

void pkt_buf_free_(struct pkt_buf* buf){
    return pkt_buf_free(buf);
}

void test_link_success_(){
    test_link_success();
}

uint32_t ixy_rx_batch_(struct ixy_device* dev, uint16_t queue_id, struct pkt_buf* bufs[], uint32_t num_bufs){
    return mqnic_rx_batch(dev, queue_id, bufs, num_bufs);
}

uint32_t ixy_rx_batch_hints_(struct ixy_device* dev, uint16_t queue_id, struct pkt_buf* bufs[], uint32_t num_bufs, uint16_t if_hint,  struct nic_hints* hints, uint16_t* hint_count){
    return mqnic_rx_batch_hints(dev, queue_id, bufs, num_bufs, if_hint, hints, hint_count);
}

uint32_t ixy_tx_batch_(struct ixy_device* dev, uint16_t queue_id, struct pkt_buf* bufs[], uint32_t num_bufs){
    return ixy_tx_batch(dev, queue_id, bufs, num_bufs);
}

struct ixy_device* ixy_init_(const char* pci_addr, uint16_t rx_queues, uint16_t tx_queues, int interrupt_timeout){
    return ixy_init(pci_addr, rx_queues, tx_queues, interrupt_timeout);
};

void register_app_(struct ixy_device* ixy, uint16_t queue_id, uint16_t app_id, uint8_t priority){
    return register_app(ixy, queue_id, app_id, priority);
}

void deregister_app_(struct ixy_device* ixy, uint16_t queue_id, uint16_t app_id){
    return deregister_app(ixy, queue_id, app_id);
}


void config_app_mat_(struct ixy_device* ixy, uint16_t app_id, uint16_t port_num, uint8_t priority){
    return config_app_mat(ixy, app_id, port_num, priority);
}

void mqnic_rx_feedback_(struct ixy_device* ixy, uint16_t queue_id, uint16_t app_id, uint16_t update_count){
    return mqnic_rx_feedback(ixy, queue_id, app_id, update_count);
}

uint8_t process_work_(uint8_t* data, uint8_t enable_work, uint8_t if_preemptive, uint32_t p_interval){
    return process_work(data, enable_work, if_preemptive, p_interval);
}


void mqnic_port_reset_monitor_(struct ixy_device* ixy){
    return mqnic_port_reset_monitor(ixy);
}

void mqnic_port_set_monitor_(struct ixy_device* ixy, uint16_t app_id, uint8_t cong_eopch_log, uint8_t scale_down_eopch_log, uint8_t scale_down_thresh){
    mqnic_port_set_monitor(ixy, app_id, cong_eopch_log, scale_down_eopch_log, scale_down_thresh);
}


void mqnic_rearm_monitor_(struct ixy_device* ixy, uint16_t queue_id, uint16_t app_id){
    mqnic_rearm_monitor(ixy, queue_id, app_id);
}

void mqnic_rearm_scale_down_monitor_(struct ixy_device* ixy, uint16_t queue_id, uint16_t app_id){
    mqnic_rearm_scale_down_monitor(ixy, queue_id, app_id);
}