#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <linux/limits.h>
#include <linux/vfio.h>
#include <sys/stat.h>

#include "log.h"
#include "mqnic.h"
#include "pci.h"
#include "memory.h"

#include "driver/device.h"

#include "libixy-vfio.h"
#include "interrupts.h"
#include "stats.h"
#include <sys/mman.h>

const char *mqnic_driver_name = "mqnic0";

// TODO: Check round up to 2

// RSS config
// #define NUM_RX_QUEUE_ENTRIES 512
// #define NUM_TX_QUEUE_ENTRIES 512
// #define NUM_CPL_QUEUE_ENTRIES 512

#define NUM_RX_QUEUE_ENTRIES 256
#define NUM_TX_QUEUE_ENTRIES 256
#define NUM_CPL_QUEUE_ENTRIES 256

#define IF_RXCQ_BYPASS_REG 10
#define IF_TXCQ_BYPASS_REG 10
#define IF_RXTX_BYPASS_REG 10

#define RXCQ_BYPASS_BATCH 0
#define RXCQ_TAIL_UPDATE_BATCH 32

const int MQNIC_PKT_BUF_ENTRY_SIZE = 2048;
const int MQNIC_MIN_MEMPOOL_ENTRIES = 4096;
const int DESC_BLOCK_SIZE = 1;

struct mqnic_rx_queue
{
	// mostly constant
	uint8_t *rxq_addr;
	uint8_t *cpl_addr;
	uint32_t size;
	uint32_t full_size;
	uint32_t size_mask;
	uint32_t hw_ptr_mask;
	uint32_t accumulated_cq_updates;

	// rx ring pointers
	uint32_t rxq_head_ptr;
	uint32_t rxq_tail_ptr;
	uint32_t rxq_clean_tail_ptr;

	// cpl ring pointers
	uint32_t cpl_head_ptr;
	uint32_t cpl_tail_ptr;
	uint32_t cpl_clean_tail_ptr;

	volatile struct mqnic_desc *rxq_descriptors;
	volatile struct mqnic_cpl *cpl_descriptors;
	struct mempool *mempool;
	// uint16_t num_entries;
	// virtual addresses to map descriptors back to their mbuf for freeing
	void *rxq_virtual_addresses[NUM_RX_QUEUE_ENTRIES];
	void *cpl_virtual_addresses[NUM_CPL_QUEUE_ENTRIES];

	// msg_hint array
	uint8_t app_hints[];
};

struct mqnic_tx_queue
{
	// mostly constant
	uint8_t *txq_addr;
	uint8_t *cpl_addr;
	uint32_t size;
	uint32_t stride;
	uint32_t full_size;
	uint32_t size_mask;
	uint32_t hw_ptr_mask;
	uint32_t desc_block_size;
	uint32_t log_desc_block_size;

	// tx ring pointers
	uint32_t txq_head_ptr;
	uint32_t txq_tail_ptr;
	uint32_t txq_clean_tail_ptr;

	// cpl ring pointers
	uint32_t cpl_head_ptr;
	uint32_t cpl_tail_ptr;
	uint32_t cpl_clean_tail_ptr;

	volatile struct mqnic_desc *txq_descriptors;
	volatile struct mqnic_cpl *cpl_descriptors;
	// struct mempool* mempool;
	// uint16_t num_entries;
	// virtual addresses to map descriptors back to their mbuf for freeing
	void *txq_virtual_addresses[NUM_TX_QUEUE_ENTRIES];
	void *cpl_virtual_addresses[NUM_CPL_QUEUE_ENTRIES];
};

static inline void mqnic_refill_rx_buffers(struct ixy_device *ixy, uint16_t queue_id);
static inline int mqnic_prepare_rx_desc(struct mqnic_rx_queue *queue, uint32_t index);
static void init_tx(struct mqnic_device *dev);
static void init_rx(struct mqnic_device *dev);
static void start_txq_cpl_queue(struct mqnic_device *dev, int queue_id);
static void start_rxq_cpl_queue(struct mqnic_device *dev, int queue_id);
static void mqnic_reset_and_init(struct mqnic_device *dev);
static inline uint32_t round_power(uint32_t x);
static inline uint32_t log2_floor(uint32_t x);
static inline void mqnic_rx_cq_read_head_ptr(struct mqnic_rx_queue *queue);
static inline void mqnic_rx_cq_write_tail_ptr(struct mqnic_rx_queue *queue);
static inline void mqnic_tx_cq_read_head_ptr(struct mqnic_tx_queue *queue);
static inline void mqnic_tx_cq_write_tail_ptr(struct mqnic_tx_queue *queue);
static inline void mqnic_rx_read_tail_ptr(struct mqnic_rx_queue *queue);
static inline void mqnic_tx_read_tail_ptr(struct mqnic_tx_queue *queue);
static inline void mqnic_port_set_rss_mask(struct mqnic_device *dev, uint32_t rss_mask, uint32_t user_ip, uint32_t rank_bound);

static inline uint32_t round_power(uint32_t x)
{
	int power = 1;
	while (x >>= 1)
		power <<= 1;
	return power;
}

static inline uint32_t log2_floor(uint32_t x)
{
	uint32_t res = -1;
	while (x)
	{
		res++;
		x = x >> 1;
	}
	return res;
}

static void start_txq_cpl_queue(struct mqnic_device *dev, int queue_id)
{
	debug("starting tx queue %d", queue_id);
	struct mqnic_tx_queue *queue = ((struct mqnic_tx_queue *)(dev->tx_queues)) + queue_id;

	// activate cpl queue
	// set interrupt index, TODO: need to disable interrupt
	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_INTERRUPT_INDEX_REG, dev->num_event_queues - 1);
	// set size and activate queue
	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size) | MQNIC_CPL_QUEUE_ACTIVE_MASK);

	// activate tx queue
	// set completion queue index
	//  need to consider kernel offset
	set_reg32(queue->txq_addr, MQNIC_QUEUE_CPL_QUEUE_INDEX_REG, queue_id + MQNIC_TX_KERNEL_QUEUE_NUMBER);
	// set size and activate queue
	set_reg32(queue->txq_addr, MQNIC_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size) | (queue->log_desc_block_size << 8) | MQNIC_QUEUE_ACTIVE_MASK);
}

static void start_rxq_cpl_queue(struct mqnic_device *dev, int queue_id)
{
	debug("starting rx queue %d", queue_id);
	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;

	// rxq and cpl
	int mempool_size = round_power(NUM_RX_QUEUE_ENTRIES * 2);
	queue->mempool = memory_allocate_mempool(mempool_size < MQNIC_MIN_MEMPOOL_ENTRIES ? MQNIC_MIN_MEMPOOL_ENTRIES : mempool_size, MQNIC_PKT_BUF_ENTRY_SIZE);

	debug("finish allocate rx mempool for %d", queue_id);

	if (queue->size & (queue->size - 1))
	{
		error("number of queue entries must be a power of 2");
	}

	// activate cpl queue
	// set interrupt index, TODO: need to disable interrupt
	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_INTERRUPT_INDEX_REG, dev->num_event_queues - 1);
	// set size and activate queue
	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size) | MQNIC_CPL_QUEUE_ACTIVE_MASK);

	// activate rx queue
	// set completion queue index
	//  need to consider kernel offset
	set_reg32(queue->rxq_addr, MQNIC_QUEUE_CPL_QUEUE_INDEX_REG, queue_id + MQNIC_RX_KERNEL_QUEUE_NUMBER);
	// set size and activate queue
	set_reg32(queue->rxq_addr, MQNIC_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size) | MQNIC_QUEUE_ACTIVE_MASK);

	mqnic_refill_rx_buffers((struct ixy_device *)dev, queue_id);

	debug("finish mqnic_refill_rx_buffers %d", queue_id);

	mqnic_rx_read_tail_ptr(queue);
	debug("mqnic_rx_read_tail_ptr %d\n", queue->rxq_tail_ptr);
}

static inline void mqnic_refill_rx_buffers(struct ixy_device *ixy, uint16_t queue_id)
{
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;

	uint32_t missing = queue->size - (queue->rxq_head_ptr - queue->rxq_clean_tail_ptr);

	if (missing < 8)
		return;

	for (; missing-- > 0;)
	{
		if (mqnic_prepare_rx_desc(queue, queue->rxq_head_ptr & queue->size_mask))
			break;
		queue->rxq_head_ptr++;
	}
	// printf("Update -- rx head ptr %d , tail %d\n", queue->rxq_head_ptr, queue->rxq_tail_ptr);

	// enqueue on NIC
	set_reg32(queue->rxq_addr, MQNIC_QUEUE_HEAD_PTR_REG, queue->rxq_head_ptr & queue->hw_ptr_mask);
}

void mqnic_fill_rx_buffers(struct ixy_device *ixy, uint16_t queue_id, int num)
{

	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;
	for (int i = 0; i < num; i++)
	{
		if (mqnic_prepare_rx_desc(queue, queue->rxq_head_ptr & queue->size_mask))
			break;
		queue->rxq_head_ptr++;
	}
	// printf("Update -- rx head ptr %d , tail %d\n", queue->rxq_head_ptr, queue->rxq_tail_ptr);
	set_reg32(queue->rxq_addr, MQNIC_QUEUE_HEAD_PTR_REG, queue->rxq_head_ptr & queue->hw_ptr_mask);
}

void mqnic_test_rx_mmio(struct ixy_device *ixy, uint16_t queue_id)
{

	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_tx_queue *queue = ((struct mqnic_tx_queue *)(dev->tx_queues)) + queue_id;

	set_reg32(queue->txq_addr, MQNIC_QUEUE_HEAD_PTR_REG, 0);
}

static inline int mqnic_prepare_rx_desc(struct mqnic_rx_queue *queue, uint32_t index)
{

	volatile struct mqnic_desc *rxd = queue->rxq_descriptors + index;
	struct pkt_buf *buf = pkt_buf_alloc(queue->mempool);
	if (!buf)
	{
		error("failed to allocate rx descriptor");
		return -1;
	}
	rxd->addr = buf->buf_addr_phy + offsetof(struct pkt_buf, data);
	rxd->len = queue->mempool->buf_size;
	// we need to return the virtual address in the rx function which the descriptor doesn't know by default
	queue->rxq_virtual_addresses[index] = buf;
	// debug("generate one descriptor %x, %d", rxd->addr, rxd->len);
	return 0;
}

static void init_tx(struct mqnic_device *dev)
{
	int stride = DESC_BLOCK_SIZE * MQNIC_DESC_SIZE;
	for (uint16_t i = 0; i < dev->ixy.num_tx_queues; i++)
	{
		info("initializing tx queue %d", i);

		// Get common parameter
		struct mqnic_tx_queue *queue = ((struct mqnic_tx_queue *)(dev->tx_queues)) + i;

		queue->size = round_power(NUM_TX_QUEUE_ENTRIES);
		queue->full_size = queue->size >> 1;
		queue->size_mask = queue->size - 1;
		queue->hw_ptr_mask = 0xffff;

		info("tx queue size %d", queue->size);
		info("tx queue size_mask %x", queue->size_mask);

		// Init cpl ring
		queue->cpl_addr = dev->addr + dev->tx_cpl_queue_offset + i * MQNIC_CPL_QUEUE_STRIDE + MQNIC_TX_KERNEL_QUEUE_NUMBER * MQNIC_CPL_QUEUE_STRIDE;
		queue->cpl_head_ptr = 0;
		queue->cpl_tail_ptr = 0;
		queue->cpl_clean_tail_ptr = 0;

		uint32_t cpl_ring_size_bytes = queue->size * MQNIC_CPL_SIZE;
		struct dma_memory cpl_ring_mem = memory_allocate_dma(cpl_ring_size_bytes, true);
		memset(cpl_ring_mem.virt, 0, cpl_ring_size_bytes);

		queue->cpl_descriptors = (struct mqnic_cpl *)cpl_ring_mem.virt;
		debug("tx cpl %d cpl_addr:  0x%012lX", i, queue->cpl_addr - dev->addr);
		debug("tx cpl %d phy addr:  0x%012lX", i, cpl_ring_mem.phy);
		debug("tx cpl %d virt addr: 0x%012lX", i, (uintptr_t)cpl_ring_mem.virt);

		// deactivate queue
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_ACTIVE_LOG_SIZE_REG, 0);

		// set base address
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_BASE_ADDR_REG + 0, cpl_ring_mem.phy & 0xFFFFFFFFull);
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_BASE_ADDR_REG + 4, cpl_ring_mem.phy >> 32);

		uint32_t eq_index = dev->num_event_queues - 1;

		// set interrupt index
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_INTERRUPT_INDEX_REG, eq_index);

		// skip arming the cq will avoid generating the interrupt
		// Well, here is a bug, it will not receive interrupt even when we armed the cq..
		// set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_INTERRUPT_INDEX_REG, eq_index | MQNIC_CPL_QUEUE_ARM_MASK);

		// set pointers
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_HEAD_PTR_REG, queue->cpl_head_ptr & queue->hw_ptr_mask);
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_TAIL_PTR_REG, queue->cpl_tail_ptr & queue->hw_ptr_mask);
		// set size
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size));

		// Init TX ring

		queue->stride = stride;
		queue->desc_block_size = queue->stride / MQNIC_DESC_SIZE;
		queue->log_desc_block_size = queue->desc_block_size < 2 ? 0 : log2_floor(queue->desc_block_size - 1) + 1;
		queue->desc_block_size = 1 << queue->log_desc_block_size;

		queue->txq_addr = dev->addr + dev->tx_queue_offset + i * MQNIC_QUEUE_STRIDE + MQNIC_TX_KERNEL_QUEUE_NUMBER * MQNIC_QUEUE_STRIDE;

		queue->txq_head_ptr = 0;
		queue->txq_tail_ptr = 0;
		queue->txq_clean_tail_ptr = 0;

		uint32_t tx_ring_size_bytes = queue->size * queue->stride;
		struct dma_memory tx_ring_mem = memory_allocate_dma(tx_ring_size_bytes, true);

		memset(tx_ring_mem.virt, -1, tx_ring_size_bytes);
		queue->txq_descriptors = (struct mqnic_desc *)tx_ring_mem.virt;

		info("tx ring %d phy addr:  0x%012lX", i, tx_ring_mem.phy);
		info("tx ring %d virt addr: 0x%012lX", i, (uintptr_t)tx_ring_mem.virt);

		// deactivate queue
		set_reg32(queue->txq_addr, MQNIC_QUEUE_ACTIVE_LOG_SIZE_REG, 0);
		info("Finish Setting the queue register");

		// set base address
		set_reg32(queue->txq_addr, MQNIC_QUEUE_BASE_ADDR_REG + 0, tx_ring_mem.phy & 0xFFFFFFFFull);
		set_reg32(queue->txq_addr, MQNIC_QUEUE_BASE_ADDR_REG + 4, tx_ring_mem.phy >> 32);
		// tmp set completion queue index, will assign when activate
		set_reg32(queue->txq_addr, MQNIC_QUEUE_CPL_QUEUE_INDEX_REG, 0);

		// set pointers
		set_reg32(queue->txq_addr, MQNIC_QUEUE_HEAD_PTR_REG, queue->txq_head_ptr & queue->hw_ptr_mask);
		set_reg32(queue->txq_addr, MQNIC_QUEUE_TAIL_PTR_REG, queue->txq_tail_ptr & queue->hw_ptr_mask);
		// set size
		set_reg32(queue->txq_addr, MQNIC_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size) | (queue->log_desc_block_size << 8));
	}
}

void activate_hw_sche(struct mqnic_device *dev)
{
	int k;
	uint8_t *port_hw_addr = dev->addr + dev->port_offset;

	uint32_t sche_offset = get_reg32(port_hw_addr, MQNIC_PORT_REG_SCHED_OFFSET);
	info("Scheduler offset: 0x%08x", sche_offset);

	// enable schedulers
	set_reg32(port_hw_addr, MQNIC_PORT_REG_SCHED_ENABLE, 0xffffffff);

	// enable queues
	for (k = MQNIC_TX_KERNEL_QUEUE_NUMBER; k < (MQNIC_TX_KERNEL_QUEUE_NUMBER + dev->ixy.num_tx_queues); k++)
	{
		set_reg32(port_hw_addr, sche_offset + k * 4, 3);
	}
}

static void init_rx(struct mqnic_device *dev)
{

	// per-queue config, same for all queues
	for (uint16_t i = 0; i < dev->ixy.num_rx_queues; i++)
	{

		info("initializing rx queue %d", i);
		// Get common parameter
		struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + i;

		queue->size = round_power(NUM_RX_QUEUE_ENTRIES);
		queue->size_mask = queue->size - 1;
		queue->hw_ptr_mask = 0xffff;

		info("rx queue size %d", queue->size);
		info("rx queue size_mask %x", queue->size_mask);

		// Init cpl ring
		queue->cpl_addr = dev->addr + dev->rx_cpl_queue_offset + i * MQNIC_CPL_QUEUE_STRIDE + MQNIC_RX_KERNEL_QUEUE_NUMBER * MQNIC_CPL_QUEUE_STRIDE;
		queue->cpl_head_ptr = 0;
		queue->cpl_tail_ptr = 0;
		queue->cpl_clean_tail_ptr = 0;
		queue->accumulated_cq_updates = 0;

		uint32_t cpl_ring_size_bytes = queue->size * MQNIC_CPL_SIZE;
		struct dma_memory cpl_ring_mem = memory_allocate_dma(cpl_ring_size_bytes, true);
		memset(cpl_ring_mem.virt, 0, cpl_ring_size_bytes);

		queue->cpl_descriptors = (struct mqnic_cpl *)cpl_ring_mem.virt;
		debug("rx cpl %d phy addr:  0x%012lX", i, cpl_ring_mem.phy);
		debug("rx cpl %d virt addr: 0x%012lX", i, (uintptr_t)cpl_ring_mem.virt);

		// deactivate queue
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_ACTIVE_LOG_SIZE_REG, 0);

		// set base address
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_BASE_ADDR_REG + 0, cpl_ring_mem.phy & 0xFFFFFFFFull);
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_BASE_ADDR_REG + 4, cpl_ring_mem.phy >> 32);

		uint32_t eq_index = dev->num_event_queues - 1;

		// set interrupt index
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_INTERRUPT_INDEX_REG, eq_index);

		// skip arming the cq will avoid generating the interrupt
		// Well, here is a bug, it will not receive interrupt even when we armed the cq..
		// set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_INTERRUPT_INDEX_REG, eq_index | MQNIC_CPL_QUEUE_ARM_MASK);

		// set pointers
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_HEAD_PTR_REG, queue->cpl_head_ptr & queue->hw_ptr_mask);
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_TAIL_PTR_REG, queue->cpl_tail_ptr & queue->hw_ptr_mask);
		// set size
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size));

		// Init RX ring
		queue->rxq_addr = dev->addr + dev->rx_queue_offset + i * MQNIC_QUEUE_STRIDE + MQNIC_RX_KERNEL_QUEUE_NUMBER * MQNIC_QUEUE_STRIDE;
		queue->rxq_head_ptr = 0;
		queue->rxq_tail_ptr = 0;
		queue->rxq_clean_tail_ptr = 0;

		info("rx queue rxq_addr %x", queue->rxq_addr);

		uint32_t rx_ring_size_bytes = queue->size * MQNIC_DESC_SIZE;
		struct dma_memory rx_ring_mem = memory_allocate_dma(rx_ring_size_bytes, true);
		memset(rx_ring_mem.virt, -1, rx_ring_size_bytes);

		queue->rxq_descriptors = (struct mqnic_desc *)rx_ring_mem.virt;
		info("rx ring %d phy addr:  0x%012lX", i, rx_ring_mem.phy);
		info("rx ring %d virt addr: 0x%012lX", i, (uintptr_t)rx_ring_mem.virt);

		// deactivate queue
		set_reg32(queue->rxq_addr, MQNIC_QUEUE_ACTIVE_LOG_SIZE_REG, 0);
		info("Finish Setting the queue register");

		// set base address
		set_reg32(queue->rxq_addr, MQNIC_QUEUE_BASE_ADDR_REG + 0, rx_ring_mem.phy & 0xFFFFFFFFull);
		set_reg32(queue->rxq_addr, MQNIC_QUEUE_BASE_ADDR_REG + 4, rx_ring_mem.phy >> 32);
		// tmp set completion queue index, will assign when activate
		set_reg32(queue->rxq_addr, MQNIC_QUEUE_CPL_QUEUE_INDEX_REG, 0);

		// set pointers
		set_reg32(queue->rxq_addr, MQNIC_QUEUE_HEAD_PTR_REG, queue->rxq_head_ptr & queue->hw_ptr_mask);
		set_reg32(queue->rxq_addr, MQNIC_QUEUE_TAIL_PTR_REG, queue->rxq_tail_ptr & queue->hw_ptr_mask);
		// set size
		set_reg32(queue->rxq_addr, MQNIC_QUEUE_ACTIVE_LOG_SIZE_REG, log2_floor(queue->size));

		// init dispatcher's per-core queue
		set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG, 17);
	}
}

void register_app(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id, uint8_t priority)
{

	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;
	printf("Register APP %d on core %d\n", app_id, queue_id);

	// set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG,  ((0 << 20)&0x00f00000) | ((1<< 16)&0x000f0000) | ((app_id<<4)&0x00000ff0) | ((priority<<12)&0x0000f000)| (3 & 0x0000000f) );

	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG, ((1 << 20) & 0x00f00000) | ((5 << 16) & 0x000f0000) | ((app_id << 4) & 0x00000ff0) | ((priority << 12) & 0x0000f000) | (3 & 0x0000000f));
}

void deregister_app(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id)
{

	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;
	printf("DeRegister APP %d on core %d\n", app_id, queue_id);

	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG, ((app_id << 4) & 0x00000ff0) | (4 & 0x0000000f));
}

void config_app_mat(struct ixy_device *ixy, uint16_t app_id, uint16_t port_num, uint8_t priority)
{
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_APP_CONFG,
			  ((port_num << 16) & 0xffff0000) | ((priority << 12) & 0x0000f000) | ((app_id << 4) & 0x00000ff0));
}

void mqnic_port_reset_monitor(struct ixy_device *ixy)
{
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);
	// reset monitor
	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_APP_CONFG,
			  (2 & 0x0000000f));
}

void mqnic_port_set_monitor(struct ixy_device *ixy, uint16_t app_id, uint8_t cong_eopch_log, uint8_t scale_down_eopch_log, uint8_t scale_down_thresh)
{
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);
	// // reset monitor
	// set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_APP_CONFG,
	//  ((app_id<<4)&0x00000ff0) |  (2 & 0x0000000f) );

	// config monitor
	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_APP_CONFG,
			  ((scale_down_thresh << 28) & 0xf0000000) | ((cong_eopch_log << 20) & 0x0ff00000) | ((scale_down_eopch_log << 12) & 0x000ff000) | ((app_id << 4) & 0x00000ff0) | (1 & 0x0000000f));

	// printf("Config monitor epoch log %d\n", cong_eopch_log);
}

void mqnic_rearm_monitor(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id)
{
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;

	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG, ((app_id << 4) & 0x00000ff0) | (6 & 0x0000000f));
	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG, ((app_id << 4) & 0x00000ff0) | (7 & 0x0000000f));
}

void mqnic_rearm_scale_down_monitor(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id)
{
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;

	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG, ((app_id << 4) & 0x00000ff0) | (7 & 0x0000000f));
}

static void mqnic_reset_and_init(struct mqnic_device *dev)
{
	// init rx queues
	// mqnic_test_mmio(dev);
	info("Start to Init TX Queues ..");
	init_tx(dev);
	info("Success Init TX Queues ..");

	info("Start to Init RX Queues ..");

	init_rx(dev);

	info("Success Init RX Queues");

	info("Start to Start TX Queues ..");
	// enables queues after initializing everything
	for (uint16_t i = 0; i < dev->ixy.num_tx_queues; i++)
	{
		start_txq_cpl_queue(dev, i);
	}
	info("Success Start TX Queues");

	info("Start to Start RX Queues ..");
	// enables queues after initializing everything
	for (uint16_t i = 0; i < dev->ixy.num_rx_queues; i++)
	{
		start_rxq_cpl_queue(dev, i);
	}
	info("Success Start RX Queues");

	activate_hw_sche(dev);
	info("Success Activaet HW Scheduler");

	mqnic_port_set_rss_mask(dev, dev->ixy.num_rx_queues - 1, 0xc0a8e902, PER_CORE_RANK_BOUND * 5);
	usleep(5000);
	info("Finish reset_and_init");
}

/**
 * Initializes and returns the IXGBE device.
 * @param pci_addr The PCI address of the device.
 * @param rx_queues The number of receiver queues.
 * @param tx_queues The number of transmitter queues.
 * @param interrupt_timeout The interrupt timeout in milliseconds
 * 	- if set to -1 the interrupt timeout is disabled
 * 	- if set to 0 the interrupt is disabled entirely)
 * @return The initialized IXGBE device.
 */
struct ixy_device *mqnic_init(const char *pci_addr, uint16_t rx_queues, uint16_t tx_queues, int interrupt_timeout)
{
	struct mqnic_ioctl_info mqnic_ioctl_msg;

	if (getuid())
	{
		warn("Not running as root, this will probably fail");
	}
	if (rx_queues > MAX_QUEUES)
	{
		error("cannot configure %d rx queues: limit is %d", rx_queues, MAX_QUEUES);
	}
	if (tx_queues > MAX_QUEUES)
	{
		error("cannot configure %d tx queues: limit is %d", tx_queues, MAX_QUEUES);
	}

	// Allocate memory for the device that will be returned
	struct mqnic_device *dev = (struct mqnic_device *)malloc(sizeof(struct mqnic_device));
	dev->ixy.pci_addr = strdup(pci_addr);

	char path[PATH_MAX];
	snprintf(path, PATH_MAX, "/sys/bus/pci/devices/%s/iommu_group", pci_addr);
	struct stat buffer;
	dev->ixy.vfio = stat(path, &buffer) == 0;

	// Check if we want the VFIO stuff
	// This is done by checking if the device is in an IOMMU group.

	if (dev->ixy.vfio)
	{
		info("Find the IOMMU for device");
		// initialize the IOMMU for this device
		dev->ixy.vfio_fd = vfio_init(pci_addr);
		if (dev->ixy.vfio_fd < 0)
		{
			error("could not initialize the IOMMU for device %s", pci_addr);
		}
		dev->addr = vfio_map_region(dev->ixy.vfio_fd, VFIO_PCI_BAR0_REGION_INDEX);
	}
	else
	{
		warn("Not find the IOMMU for device");
		dev->addr = pci_map_resource(pci_addr);
	}

	// initialize interrupts for this device
	// setup_interrupts(dev);
	mqnic_ioctl_msg.fw_id = get_reg32(dev->addr, MQNIC_REG_FW_ID);
	mqnic_ioctl_msg.fw_ver = get_reg32(dev->addr, MQNIC_REG_FW_VER);
	mqnic_ioctl_msg.board_id = get_reg32(dev->addr, MQNIC_REG_BOARD_ID);
	mqnic_ioctl_msg.board_ver = get_reg32(dev->addr, MQNIC_REG_BOARD_VER);
	// mqnic_ioctl_msg.rx_queue_offset = 0;
	// mqnic_ioctl_msg.rx_cpl_queue_offset = 0;
	// mqnic_ioctl_msg.tx_queue_offset = 0;
	// mqnic_ioctl_msg.tx_cpl_queue_offset = 0;
	// mqnic_ioctl_msg.port_offset =0;
	// mqnic_ioctl_msg.rx_queue_offset = get_reg32(dev->addr + MQNIC_REG_IF_CSR_OFFSET);
	uint32_t if_csr_offset = get_reg32(dev->addr, MQNIC_REG_IF_CSR_OFFSET);
	info("IF CSR offset: 0x%08x\n", get_reg32(dev->addr, MQNIC_REG_IF_CSR_OFFSET));
	uint8_t *csr_hw_addr = dev->addr + if_csr_offset;
	mqnic_ioctl_msg.rx_queue_offset = get_reg32(csr_hw_addr, MQNIC_IF_REG_RX_QUEUE_OFFSET);
	mqnic_ioctl_msg.rx_cpl_queue_offset = get_reg32(csr_hw_addr, MQNIC_IF_REG_RX_CPL_QUEUE_OFFSET);
	mqnic_ioctl_msg.tx_queue_offset = get_reg32(csr_hw_addr, MQNIC_IF_REG_TX_QUEUE_OFFSET);
	mqnic_ioctl_msg.tx_cpl_queue_offset = get_reg32(csr_hw_addr, MQNIC_IF_REG_TX_CPL_QUEUE_OFFSET);
	mqnic_ioctl_msg.port_offset = get_reg32(csr_hw_addr, MQNIC_IF_REG_PORT_OFFSET);

	mqnic_ioctl_msg.num_event_queues = get_reg32(csr_hw_addr, MQNIC_IF_REG_EVENT_QUEUE_COUNT);
	mqnic_ioctl_msg.regs_size = 0x1000;
	mqnic_ioctl_msg.num_rx_queues = get_reg32(csr_hw_addr, MQNIC_IF_REG_RX_QUEUE_COUNT);
	mqnic_ioctl_msg.num_tx_queues = get_reg32(csr_hw_addr, MQNIC_IF_REG_TX_QUEUE_COUNT);

	// communicate with kernel driver through IOCTL
	// int fd;
	// fd = open("/dev/mqnic0", O_RDWR);
	// if(fd < 0)
	// 	error("Cannot open device file...\n");

	info("Start to get MQNIC Configurations ..");
	// ioctl(fd, MQNIC_IOCTL_INFO, (struct mqnic_ioctl_info*) &mqnic_ioctl_msg);
	dev->fw_id = mqnic_ioctl_msg.fw_id;
	dev->fw_ver = mqnic_ioctl_msg.fw_ver;
	dev->board_id = mqnic_ioctl_msg.board_id;
	dev->board_ver = mqnic_ioctl_msg.board_ver;
	dev->rx_queue_offset = mqnic_ioctl_msg.rx_queue_offset;
	dev->rx_cpl_queue_offset = mqnic_ioctl_msg.rx_cpl_queue_offset;
	dev->tx_queue_offset = mqnic_ioctl_msg.tx_queue_offset;
	dev->tx_cpl_queue_offset = mqnic_ioctl_msg.tx_cpl_queue_offset;
	dev->port_offset = mqnic_ioctl_msg.port_offset;
	dev->num_event_queues = mqnic_ioctl_msg.num_event_queues;
	dev->regs_size = mqnic_ioctl_msg.regs_size;

	// dev->ixy.num_rx_queues = mqnic_ioctl_msg.num_rx_queues;
	// dev->ixy.num_tx_queues = mqnic_ioctl_msg.num_tx_queues;
	info("Get MQNIC Configurations Success");

	dev->ixy.driver_name = mqnic_driver_name;
	dev->ixy.num_rx_queues = rx_queues;
	dev->ixy.num_tx_queues = tx_queues;
	dev->ixy.rx_batch = mqnic_rx_batch;
	dev->ixy.tx_batch = mqnic_tx_batch;
	dev->ixy.read_stats = mqnic_read_stats;
	dev->ixy.set_promisc = mqnic_set_promisc;
	dev->ixy.get_link_speed = mqnic_get_link_speed;
	dev->ixy.get_mac_addr = mqnic_get_mac_addr;
	dev->ixy.set_mac_addr = mqnic_set_mac_addr;
	dev->ixy.interrupts.interrupts_enabled = interrupt_timeout != 0;
	// 0x028 (10ys) => 97600 INT/s
	dev->ixy.interrupts.itr_rate = 0x028;
	dev->ixy.interrupts.timeout_ms = interrupt_timeout;

	if (!dev->ixy.vfio && interrupt_timeout != 0)
	{
		warn("Interrupts requested but VFIO not available: Disabling Interrupts!");
		dev->ixy.interrupts.interrupts_enabled = false;
	}
	info("---------------");
	info("fw_id %d", dev->fw_id);
	info("num_event_queues 0x%x", dev->num_event_queues);
	info("rx_queue_offset 0x%x", dev->rx_queue_offset);
	info("rx_cpl_queue_offset 0x%x", dev->rx_cpl_queue_offset);
	info("tx_queue_offset 0x%x", dev->tx_queue_offset);
	info("tx_cpl_queue_offset 0x%x", dev->tx_cpl_queue_offset);
	info("port_offset 0x%x", dev->port_offset);
	info("regs_size 0x%x", dev->regs_size);
	info("rx kernel queue offset %d, len %d", 0, mqnic_ioctl_msg.num_rx_queues);
	info("rx user queue offset %d, len %d", MQNIC_RX_KERNEL_QUEUE_NUMBER, dev->ixy.num_rx_queues);
	// assert(MQNIC_RX_KERNEL_QUEUE_NUMBER == mqnic_ioctl_msg.num_rx_queues);
	// assert(2 * mqnic_ioctl_msg.num_rx_queues >= MQNIC_RX_KERNEL_QUEUE_NUMBER + dev->ixy.num_rx_queues);
	info("tx kernel queue offset %d, len %d", 0, mqnic_ioctl_msg.num_tx_queues);
	info("tx user queue offset %d, len %d", MQNIC_TX_KERNEL_QUEUE_NUMBER, dev->ixy.num_tx_queues);
	// assert(MQNIC_TX_KERNEL_QUEUE_NUMBER == mqnic_ioctl_msg.num_tx_queues);
	// assert(2 * mqnic_ioctl_msg.num_tx_queues >= MQNIC_TX_KERNEL_QUEUE_NUMBER + dev->ixy.num_tx_queues);
	info("---------------");
	// exit(0);

	// Map BAR0 region
	// if (dev->ixy.vfio) {
	// 	debug("mapping BAR0 region via VFIO...");
	// 	dev->addr = vfio_map_region(dev->ixy.vfio_fd, VFIO_PCI_BAR0_REGION_INDEX);
	// 	// initialize interrupts for this device
	// 	setup_interrupts(dev);
	// } else {
	// 	debug("mapping BAR0 region via pci file...");
	// 	dev->addr = pci_map_resource(pci_addr);
	// }

	// Map device register region to user space through mmap

	// dev->addr = (uint8_t*)mmap(NULL, dev->regs_size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0x0);

	// if (dev->addr == MAP_FAILED)
	// {
	// 	close(fd);
	// 	error("Failed to mmap device register");
	// }

	info("Success mmap MQNIC device register");

	dev->rx_queues = calloc(rx_queues, sizeof(struct mqnic_rx_queue));
	dev->tx_queues = calloc(tx_queues, sizeof(struct mqnic_tx_queue));

	mqnic_reset_and_init(dev);
	return &dev->ixy;
}

// TODO: support get_link_speed
uint32_t mqnic_get_link_speed(const struct ixy_device *ixy)
{
	return 0;
}

// TODO: support get_mac_addr
struct mac_address mqnic_get_mac_addr(const struct ixy_device *ixy)
{
	struct mac_address mac;
	mac.addr[0] = 0;
	mac.addr[1] = 0;
	mac.addr[2] = 0;
	mac.addr[3] = 0;
	mac.addr[4] = 0;
	mac.addr[5] = 0;

	return mac;
}

// TODO: support set_mac_addr
void mqnic_set_mac_addr(struct ixy_device *ixy, struct mac_address mac)
{
}

// TODO: support set_promisc
void mqnic_set_promisc(struct ixy_device *ixy, bool enabled)
{
}

// TODO: support read_stats
void mqnic_read_stats(struct ixy_device *ixy, struct device_stats *stats)
{
}

// advance index with wrap-around, this line is the reason why we require a power of two for the queue size
#define wrap_ring(index, ring_size) (uint16_t)((index + 1) & (ring_size - 1))

static inline void mqnic_rx_cq_read_head_ptr(struct mqnic_rx_queue *queue)
{
	uint32_t nic_head_ptr = get_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_HEAD_PTR_REG);
	queue->cpl_head_ptr += (nic_head_ptr - queue->cpl_head_ptr) & queue->hw_ptr_mask;
}

static inline void mqnic_rx_cq_write_tail_ptr(struct mqnic_rx_queue *queue)
{
	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_TAIL_PTR_REG, queue->cpl_tail_ptr & queue->hw_ptr_mask);
}
static inline void mqnic_tx_cq_read_head_ptr(struct mqnic_tx_queue *queue)
{
	uint32_t nic_head_ptr = get_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_HEAD_PTR_REG);
	queue->cpl_head_ptr += (nic_head_ptr - queue->cpl_head_ptr) & queue->hw_ptr_mask;
}

static inline void mqnic_tx_cq_write_tail_ptr(struct mqnic_tx_queue *queue)
{
	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_TAIL_PTR_REG, queue->cpl_tail_ptr & queue->hw_ptr_mask);
}
static inline void mqnic_rx_read_tail_ptr(struct mqnic_rx_queue *queue)
{
	uint32_t nic_tail_ptr = get_reg32(queue->rxq_addr, MQNIC_QUEUE_TAIL_PTR_REG);
	queue->rxq_tail_ptr += (nic_tail_ptr - queue->rxq_tail_ptr) & queue->hw_ptr_mask;
}

static inline void mqnic_tx_read_tail_ptr(struct mqnic_tx_queue *queue)
{
	uint32_t nic_tail_ptr = get_reg32(queue->txq_addr, MQNIC_QUEUE_TAIL_PTR_REG);
	queue->txq_tail_ptr += (nic_tail_ptr - queue->txq_tail_ptr) & queue->hw_ptr_mask;
}

static inline bool mqnic_is_tx_ring_full(struct mqnic_tx_queue *queue)
{
	return queue->txq_head_ptr - queue->txq_clean_tail_ptr >= queue->full_size;
}

void mqnic_rx_feedback(struct ixy_device *ixy, uint16_t queue_id, uint16_t app_id, uint16_t update_count)
{
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;

	set_reg32(queue->cpl_addr, MQNIC_CPL_QUEUE_CPU_MSG_REG, ((update_count << 16) & 0xffff0000) | ((app_id << 4) & 0x00000ff0) | (5 & 0x0000000f));
}

// try to receive a batch of packets and hints if available, non-blocking
uint32_t mqnic_rx_batch_hints(struct ixy_device *ixy, uint16_t queue_id, struct pkt_buf *bufs[], uint32_t num_bufs, uint16_t if_hint, struct nic_hints *hints, uint16_t *hint_count)
{
	uint32_t cq_tail_ptr, cq_index, cq_next_index, rxq_index, ring_clean_tail_ptr;
	volatile struct mqnic_cpl *cpl;
	volatile struct mqnic_cpl *nxt_cpl;
	uint32_t buf_index = 0, next_batch = 0;
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);
	bool if_next;

	uint16_t tmp_hint_count = 0;

	struct mqnic_rx_queue *queue = ((struct mqnic_rx_queue *)(dev->rx_queues)) + queue_id;

	mqnic_refill_rx_buffers(ixy, queue_id);

	cq_tail_ptr = queue->cpl_tail_ptr;
	cq_index = cq_tail_ptr & queue->size_mask;

#ifdef IF_RXCQ_BYPASS_REG
	cq_next_index = (cq_index + RXCQ_BYPASS_BATCH) & queue->size_mask;
	nxt_cpl = (struct mqnic_cpl *)(queue->cpl_descriptors + cq_next_index);
	if_next = nxt_cpl->len != 0;
	next_batch = RXCQ_BYPASS_BATCH;
#else
	mqnic_rx_cq_read_head_ptr(queue);
	if_next = queue->cpl_head_ptr != cq_tail_ptr;
#endif

	while (if_next && buf_index < num_bufs)
	{
		// printf("Receive one packet, cpl head ptr %d , tail %d\n", queue->cpl_head_ptr, cq_tail_ptr);
		// printf("rx head ptr %d , tail %d\n", queue->rxq_head_ptr, queue->rxq_tail_ptr);
		cpl = (struct mqnic_cpl *)(queue->cpl_descriptors + cq_index);
		if (if_hint && (cpl->rx_hash != 0))
		{
			hints[tmp_hint_count].hint_app_id = (cpl->rx_hash & 0x00000ff0) >> 4;
			hints[tmp_hint_count].hint_content = (cpl->rx_hash & 0xffff0000) >> 16;

			tmp_hint_count += 1;
		}
		rxq_index = cpl->index & queue->size_mask;

		// get the data buf from the rx queue
		struct pkt_buf *buf = (struct pkt_buf *)queue->rxq_virtual_addresses[rxq_index];

		// for(int w = 0; w < cpl->len; w ++){
		// 	printf("%02x ",buf ->data[w]);
		// }
		// printf("\n");

		// update the buf size according to the completion
		buf->size = GETMIN(cpl->len, queue->mempool->buf_size);
		buf->size = cpl->len;
		bufs[buf_index] = buf;

		// empty the address to show we can free rx queue.
		queue->rxq_virtual_addresses[rxq_index] = NULL;
		cpl->len = 0;

		buf_index++;
		cq_tail_ptr++;
		cq_index = cq_tail_ptr & queue->size_mask;

#ifdef IF_RXCQ_BYPASS_REG
		if (next_batch != 0)
		{
			if_next = true;
			next_batch--;
		}
		else
		{
			cq_next_index = (cq_index + RXCQ_BYPASS_BATCH) & queue->size_mask;
			nxt_cpl = (struct mqnic_cpl *)(queue->cpl_descriptors + cq_next_index);
			if_next = nxt_cpl->len != 0;
			next_batch = RXCQ_BYPASS_BATCH;
		}
#else
		if_next = queue->cpl_head_ptr != cq_tail_ptr;
#endif
	}

	// update CQ tail
	if (buf_index != 0)
	{
		queue->accumulated_cq_updates += buf_index;
		queue->cpl_tail_ptr = cq_tail_ptr;
		if (queue->accumulated_cq_updates > RXCQ_TAIL_UPDATE_BATCH)
		{
			mqnic_rx_cq_write_tail_ptr(queue);
			queue->accumulated_cq_updates = 0;
		}
	}

// process rx ring
// read tail pointer from NIC
#ifdef IF_RXTX_BYPASS_REG
	queue->rxq_tail_ptr = queue->rxq_tail_ptr + buf_index;
#else
	mqnic_rx_read_tail_ptr(queue);
#endif

	ring_clean_tail_ptr = queue->rxq_clean_tail_ptr;
	rxq_index = ring_clean_tail_ptr & queue->size_mask;

	while (ring_clean_tail_ptr != queue->rxq_tail_ptr)
	{
		if (queue->rxq_virtual_addresses[rxq_index])
		{
			break;
		}

		ring_clean_tail_ptr++;
		rxq_index = ring_clean_tail_ptr & queue->size_mask;
	}

	queue->rxq_clean_tail_ptr = ring_clean_tail_ptr;

	if (if_hint)
	{
		*hint_count = tmp_hint_count;
	}

	// if(buf_index > 0){
	// 	printf("----- batch %d cpl head ptr %d , tail %d\n", buf_index, queue->cpl_head_ptr, cq_tail_ptr);
	// 	printf("rx head ptr %d , tail %d\n", queue->rxq_head_ptr, queue->rxq_tail_ptr);
	// }

	return buf_index; // number of packets stored in bufs; buf_index points to the next index
}

uint32_t mqnic_rx_batch(struct ixy_device *ixy, uint16_t queue_id, struct pkt_buf *bufs[], uint32_t num_bufs)
{
	return mqnic_rx_batch_hints(ixy, queue_id, bufs, num_bufs, 0, NULL, NULL);
}

static inline uint32_t mqnic_process_tx_cq(struct mqnic_tx_queue *queue, int budget)
{
	uint32_t cq_tail_ptr, cq_index, txq_index, ring_clean_tail_ptr;
	struct mqnic_cpl *cpl;
	uint32_t packets = 0;
	uint32_t bytes = 0;
	int done = 0;
	bool if_next;

	cq_tail_ptr = queue->cpl_tail_ptr;
	cq_index = cq_tail_ptr & queue->size_mask;

#ifdef IF_TXCQ_BYPASS_REG
	cpl = (struct mqnic_cpl *)(queue->cpl_descriptors + cq_index);
	if_next = cpl->len != 0;
#else
	mqnic_tx_cq_read_head_ptr(queue);
	if_next = queue->cpl_head_ptr != cq_tail_ptr;
#endif

	while (if_next && done < budget)
	{
		cpl = (struct mqnic_cpl *)(queue->cpl_descriptors + cq_index);
		txq_index = cpl->index & queue->size_mask;

		// empty the address to show we can free tx queue.

		pkt_buf_free(queue->txq_virtual_addresses[txq_index]);
		queue->txq_virtual_addresses[txq_index] = NULL;
		// printf("Recive CPl: %d\n", txq_index);

		packets++;
		bytes += cpl->len;

		cpl->len = 0;

		done++;
		cq_tail_ptr++;
		cq_index = cq_tail_ptr & queue->size_mask;

#ifdef IF_TXCQ_BYPASS_REG
		cpl = (struct mqnic_cpl *)(queue->cpl_descriptors + cq_index);
		if_next = cpl->len != 0;
#else
		if_next = queue->cpl_head_ptr != cq_tail_ptr;
#endif
	}

	// update CQ tail
	queue->cpl_tail_ptr = cq_tail_ptr;
	mqnic_tx_cq_write_tail_ptr(queue);

// process ring
// read tail pointer from NIC
#ifdef IF_RXTX_BYPASS_REG
	queue->txq_tail_ptr = queue->txq_tail_ptr + done;
#else
	mqnic_tx_read_tail_ptr(queue);
#endif

	ring_clean_tail_ptr = queue->txq_clean_tail_ptr;
	txq_index = ring_clean_tail_ptr & queue->size_mask;

	while (ring_clean_tail_ptr != queue->txq_tail_ptr)
	{
		if (queue->txq_virtual_addresses[txq_index])
		{
#ifdef IF_RXTX_BYPASS_REG
			error("Error TX queue %d, %d", txq_index, done);
			exit(0);
#endif
			break;
		}

		ring_clean_tail_ptr++;
		txq_index = ring_clean_tail_ptr & queue->size_mask;
	}

	queue->txq_clean_tail_ptr = ring_clean_tail_ptr;

	return done;
}

// we control the tail, hardware the head
// huge performance gains possible here by sending packets in batches - writing to TDT for every packet is not efficient
// returns the number of packets transmitted, will not block when the queue is full
uint32_t mqnic_tx_batch(struct ixy_device *ixy, uint16_t queue_id, struct pkt_buf *bufs[], uint32_t num_bufs)
{
	int txq_index;
	struct mqnic_desc *tx_desc;
	struct mqnic_device *dev = IXY_TO_MQNIC(ixy);
	struct mqnic_tx_queue *queue = ((struct mqnic_tx_queue *)(dev->tx_queues)) + queue_id;
	// step 1: clean up descriptors that were sent out by the hardware and return them to the mempool
	mqnic_process_tx_cq(queue, 64);

	// step 2: send out as many of our packets as possible
	uint32_t sent;
	txq_index = queue->txq_head_ptr & queue->size_mask;
	for (sent = 0; sent < num_bufs; sent++)
	{
		if (mqnic_is_tx_ring_full(queue))
		{
			// printf("Warning: mqnic_is_tx_ring_full\n");
			break;
		}
		// printf("Sent Buf, Head Ptr %d\n",  txq_index);
		tx_desc = (struct mqnic_desc *)(queue->txq_descriptors + txq_index);
		tx_desc->tx_csum_cmd = 0;

		struct pkt_buf *buf = bufs[sent];
		queue->txq_virtual_addresses[txq_index] = (void *)buf;

		tx_desc[0].len = buf->size;
		tx_desc[0].addr = buf->buf_addr_phy + offsetof(struct pkt_buf, data);
		queue->txq_head_ptr++;
		// if(txq_index == 0)
		// {printf("Sent Buf, Head Ptr %d, %d\n",  txq_index, tx_desc[0].len);
		// for(int w = 0; w < tx_desc[0].len; w ++){
		// 	printf("%02x ", buf->data[w]);
		// }
		// printf("\n");}
		buf->ref_count++;
		txq_index = queue->txq_head_ptr & queue->size_mask;
	}

	set_reg32(queue->txq_addr, MQNIC_QUEUE_HEAD_PTR_REG, queue->txq_head_ptr & queue->hw_ptr_mask);
	return sent;
}

static inline void mqnic_port_set_rss_mask(struct mqnic_device *dev, uint32_t rss_mask, uint32_t user_ip, uint32_t rank_bound)
{
	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_USER_OFFSET, MQNIC_RX_KERNEL_QUEUE_NUMBER);
	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_USER_RSS_MASK, rss_mask);
	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_USER_IP, user_ip);

	// rss config
	// set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_DISPATCH_POLICY, 0);
	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_DISPATCH_POLICY, 1);
	set_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_USER_QUEUE_BOUND, rank_bound);
	uint32_t bound = get_reg32(dev->addr + dev->port_offset, MQNIC_PORT_REG_USER_QUEUE_BOUND);
	printf("Config per-core rank bound: %d\n", bound);
}
