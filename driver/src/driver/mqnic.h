#ifndef IXY_MQNIC_H
#define IXY_MQNIC_H

#include <stdbool.h>
#include "stats.h"
#include "memory.h"
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <linux/i2c.h>
#include <math.h>
#include "driver/mqnic_type.h"

#define MQNIC_IOCTL_INFO _IOR(MQNIC_IOCTL_TYPE, 0xf0, struct mqnic_ioctl_info)
#define GETMIN(x, y) ((x) > (y) ? (y) : (x))

struct mqnic_device
{
	struct ixy_device ixy;
	uint32_t fw_id;
	uint32_t fw_ver;
	uint32_t board_id;
	uint32_t board_ver;
	uint32_t rx_queue_offset;
	uint32_t rx_cpl_queue_offset;
	uint32_t tx_queue_offset;
	uint32_t tx_cpl_queue_offset;
	uint32_t port_offset;
	uint32_t num_event_queues;
	size_t regs_size;
	// register addr
	uint8_t *addr;
	void *rx_queues;
	void *tx_queues;
};

#define IXY_TO_MQNIC(ixy_device) container_of(ixy_device, struct mqnic_device, ixy)

struct ixy_device *mqnic_init(const char *pci_addr, uint16_t rx_queues, uint16_t tx_queues, int interrupt_timeout);
uint32_t mqnic_get_link_speed(const struct ixy_device *dev);
struct mac_address mqnic_get_mac_addr(const struct ixy_device *dev);
void mqnic_set_mac_addr(struct ixy_device *dev, struct mac_address mac);
void mqnic_set_promisc(struct ixy_device *dev, bool enabled);
void mqnic_read_stats(struct ixy_device *dev, struct device_stats *stats);
uint32_t mqnic_tx_batch(struct ixy_device *dev, uint16_t queue_id, struct pkt_buf *bufs[], uint32_t num_bufs);

uint32_t mqnic_rx_batch_hints(struct ixy_device *ixy, uint16_t queue_id, struct pkt_buf *bufs[], uint32_t num_bufs, uint16_t if_hint, struct nic_hints *hints, uint16_t *hint_count);
uint32_t mqnic_rx_batch(struct ixy_device *dev, uint16_t queue_id, struct pkt_buf *bufs[], uint32_t num_bufs);
void mqnic_fill_rx_buffers(struct ixy_device *ixy, uint16_t queue_id, int num);
void mqnic_test_rx_mmio(struct ixy_device *ixy, uint16_t queue_id);
void register_app(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id, uint8_t priority);
void mqnic_rx_feedback(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id, uint16_t update_count);
void config_app_mat(struct ixy_device *ixy, uint16_t app_id, uint16_t port_num, uint8_t priority);
void mqnic_port_reset_monitor(struct ixy_device *ixy);
void mqnic_port_set_monitor(struct ixy_device *ixy, uint16_t app_id, uint8_t cong_eopch_log, uint8_t scale_down_eopch_log, uint8_t scale_down_thresh);
void mqnic_rearm_monitor(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id);
void mqnic_rearm_scale_down_monitor(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id);
#endif // IXY_MQNIC_H
