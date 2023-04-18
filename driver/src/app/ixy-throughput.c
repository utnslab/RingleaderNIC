#define _GNU_SOURCE
#include <stdio.h>
#include <unistd.h>
#include <sys/time.h>
#include <pthread.h>
#include <stdlib.h>

#include "driver/mqnic.h"
#include "driver/device.h"
#include "driver/mqnic_type.h"
#include "msg.h"

const int RX_BATCH_SIZE = 16;
const int TX_BATCH_SIZE = 1;
const int worker_us = 0;
const int FEEDBACK_BATCH_SIZE = 1;
const int RING_SIZE = 128;

#define ENABLE_WORKER 1

struct poll_struct
{
	struct ixy_device *dev;
	uint32_t queue_id;
};

// From https://wiki.wireshark.org/Development/LibpcapFileFormat
typedef struct pcap_hdr_s
{
	uint32_t magic_number;	/* magic number */
	uint16_t version_major; /* major version number */
	uint16_t version_minor; /* minor version number */
	int32_t thiszone;		/* GMT to local correction */
	uint32_t sigfigs;		/* accuracy of timestamps */
	uint32_t snaplen;		/* max length of captured packets, in octets */
	uint32_t network;		/* data link type */
} pcap_hdr_t;

typedef struct pcaprec_hdr_s
{
	uint32_t ts_sec;   /* timestamp seconds */
	uint32_t ts_usec;  /* timestamp microseconds */
	uint32_t incl_len; /* number of octets of packet saved in file */
	uint32_t orig_len; /* actual length of packet */
} pcaprec_hdr_t;

struct worker_ring_s
{
	struct ixy_device *dev;
	uint32_t queue_id;
	long total_count;
	long total_feedback_count;
	long total_feedback_batch_num;
	long total_byte_count;
	long total_rx_batch_num;
	long total_rx_returned_count;
	long total_tx_batch_num;
	long total_tx_returned_count;
	long total_worktime;
	long total_worktime_count;

	uint32_t ring_head; // rx write head
	uint32_t work_head;
	uint32_t ring_tail; // tx write tail

	uint32_t ring_empty_slots;

	uint32_t unprocessed_work_count;
	uint32_t unsent_work_count;
	uint32_t unsent_feedback_count;

	struct nic_hints hints[16];
};

void init_worker_ring(struct worker_ring_s *worker_ring, struct ixy_device *dev, uint32_t queue_id)
{
	worker_ring->dev = dev;
	worker_ring->queue_id = queue_id;

	worker_ring->total_feedback_count = 0;
	worker_ring->total_feedback_batch_num = 0;
	worker_ring->total_byte_count = 0;
	worker_ring->total_rx_batch_num = 0;
	worker_ring->total_rx_returned_count = 0;
	worker_ring->total_tx_batch_num = 0;
	worker_ring->total_tx_returned_count = 0;

	worker_ring->ring_head = 0;
	worker_ring->work_head = 0;
	worker_ring->ring_tail = 0;
	worker_ring->ring_empty_slots = RING_SIZE;

	worker_ring->unprocessed_work_count = 0;
	worker_ring->unsent_work_count = 0;
	worker_ring->unsent_feedback_count = 0;
}

int if_pull_rx(struct pkt_buf **bufs, struct worker_ring_s *worker_ring)
{
	if (RING_SIZE - worker_ring->ring_empty_slots >= TX_BATCH_SIZE + 4)
		return 0;
	if (worker_ring->ring_empty_slots < RX_BATCH_SIZE)
		return 0;
	uint32_t rounded_size = RING_SIZE - worker_ring->ring_head;
	uint32_t num_rx = 0;
	uint32_t rx_batch_size = RX_BATCH_SIZE > rounded_size ? rounded_size : RX_BATCH_SIZE;

	rx_batch_size = rx_batch_size > worker_ring->ring_empty_slots ? worker_ring->ring_empty_slots : rx_batch_size;

	uint16_t hint_count = 0;
	if (rx_batch_size > 0)
	{
		num_rx = mqnic_rx_batch_hints(worker_ring->dev, worker_ring->queue_id, bufs + worker_ring->ring_head, rx_batch_size, 1, worker_ring->hints, &hint_count);

		if (hint_count > 0)
		{
			for (int i = 0; i < hint_count; i++)
			{
				printf("receive scale up msg! %d, app: %d, content: 0x%x\n", worker_ring->ring_head, worker_ring->hints[i].hint_app_id, worker_ring->hints[i].hint_content);
				mqnic_rearm_monitor(worker_ring->dev, worker_ring->queue_id, worker_ring->hints[i].hint_app_id);
			}
		}

		if (num_rx > 0)
		{
			worker_ring->total_rx_returned_count += 1;
			worker_ring->total_rx_batch_num += num_rx;
			// printf("Receive! %d, %d\n", worker_ring->queue_id, num_rx);
			worker_ring->unprocessed_work_count += num_rx;
			worker_ring->ring_empty_slots -= num_rx;

			for (uint32_t i = 0; i < num_rx; i++)
			{
				worker_ring->total_count++;
				worker_ring->total_byte_count += bufs[worker_ring->ring_head]->size;

				// for(int w = 0; w <  bufs[worker_ring->ring_head]->size; w ++){
				// 	printf("%02x ",  bufs[worker_ring->ring_head]->data[w]);
				// }
				// printf("\n");

				worker_ring->ring_head = (worker_ring->ring_head + 1) % RING_SIZE;
			}
		}
	}
	return num_rx;
}

void if_do_work(struct pkt_buf **bufs, struct worker_ring_s *worker_ring)
{
	if (worker_ring->unprocessed_work_count > 0)
	{
		uint32_t worker_ns = process_work((uint8_t *)(bufs[worker_ring->work_head]->data) + 42, ENABLE_WORKER, 0, 0);

		uint8_t mac_src_offset, mac_dst_offset, ip_src_offset, ip_dst_offset, port_src_offset, port_dst_offset;
		uint8_t *ptr_data_in;
		uint8_t tmp_mac[6];
		uint8_t tmp_ip[4];
		uint8_t tmp_port[2];

		mac_src_offset = 6;
		mac_dst_offset = 0;
		ip_src_offset = 26;
		ip_dst_offset = 30;
		port_src_offset = 34;
		port_dst_offset = 36;

		ptr_data_in = bufs[worker_ring->work_head]->data;

		memcpy(tmp_mac, ptr_data_in + mac_src_offset, 6);
		memcpy(ptr_data_in + mac_src_offset, ptr_data_in + mac_dst_offset, 6);
		memcpy(ptr_data_in + mac_dst_offset, tmp_mac, 6);

		memcpy(tmp_ip, ptr_data_in + ip_src_offset, 4);
		memcpy(ptr_data_in + ip_src_offset, ptr_data_in + ip_dst_offset, 4);
		memcpy(ptr_data_in + ip_dst_offset, tmp_ip, 4);

		memcpy(tmp_port, ptr_data_in + port_src_offset, 2);
		tmp_port[1] = 0;
		memcpy(ptr_data_in + port_src_offset, ptr_data_in + port_dst_offset, 2);
		memcpy(ptr_data_in + port_dst_offset, tmp_port, 2);

		worker_ring->total_worktime += worker_ns;
		worker_ring->total_worktime_count += 1;
		worker_ring->unprocessed_work_count--;
		worker_ring->unsent_feedback_count += 1;
		worker_ring->unsent_work_count += 1;
		worker_ring->work_head = (worker_ring->work_head + 1) % RING_SIZE;
		// printf("workns: %ld ns, workcount: %d\n", worker_ns, worker_ring->total_worktime_count);
	}
}

void if_send_feedback(struct pkt_buf **bufs, struct worker_ring_s *worker_ring)
{
	if (worker_ring->unsent_feedback_count >= FEEDBACK_BATCH_SIZE)
	{
		worker_ring->total_feedback_batch_num += FEEDBACK_BATCH_SIZE;
		worker_ring->total_feedback_count += 1;
		mqnic_rx_feedback(worker_ring->dev, worker_ring->queue_id, 1, FEEDBACK_BATCH_SIZE);
		worker_ring->unsent_feedback_count -= FEEDBACK_BATCH_SIZE;
	}
}

void if_send_tx(struct pkt_buf **bufs, struct worker_ring_s *worker_ring)
{
	if (worker_ring->unsent_work_count < TX_BATCH_SIZE)
		return;
	uint32_t rounded_size = RING_SIZE - worker_ring->ring_tail;
	// calculate tx send size
	uint32_t tx_batch_size = TX_BATCH_SIZE > rounded_size ? rounded_size : TX_BATCH_SIZE;

	tx_batch_size = tx_batch_size > worker_ring->unsent_work_count ? worker_ring->unsent_work_count : tx_batch_size;

	int sent = 0;
	if (tx_batch_size > 0)
	{
		sent = ixy_tx_batch(worker_ring->dev, worker_ring->queue_id, bufs + worker_ring->ring_tail, tx_batch_size);
		for (int i = 0; i < sent; i++)
		{
			pkt_buf_free(bufs[worker_ring->ring_tail + i]);
		}
		worker_ring->total_tx_returned_count += 1;
		worker_ring->total_tx_batch_num += sent;
		worker_ring->ring_empty_slots += sent;
		worker_ring->unsent_work_count -= sent;
		worker_ring->ring_tail = (worker_ring->ring_tail + sent) % RING_SIZE;
	}
}

void *poll_queue(void *context)
{
	struct poll_struct *poll_info = (struct poll_struct *)context;
	struct ixy_device *dev = poll_info->dev;
	uint32_t queue_id = poll_info->queue_id;
	struct worker_ring_s worker_ring;
	init_worker_ring(&worker_ring, dev, queue_id);

	printf("Launch Poll Thread, %d\n", queue_id);

	struct timeval startt, endt;
	// these two number are used to measure the avg batch size
	struct pkt_buf *bufs[RING_SIZE];

	register_app(dev, queue_id, 1, 0);
	register_app(dev, queue_id, 2, 0);

	gettimeofday(&startt, NULL);

	while (true)
	{
		if_pull_rx(bufs, &worker_ring);
		if_do_work(bufs, &worker_ring);
		if_send_feedback(bufs, &worker_ring);
		if_send_tx(bufs, &worker_ring);

		if (worker_ring.total_byte_count > 10000000)
		{
			gettimeofday(&endt, NULL);
			long msecs_time = ((endt.tv_sec - startt.tv_sec) * 1000000.0) + ((endt.tv_usec - startt.tv_usec));
			printf("Queue: %d, MBytes: %f, throughput: %f MBps, PPS: %f Mpps, Avg feedback Batch %f, Avg rx Batch %f, Avg tx Batch %f, Avg worktime %f us\n", queue_id, worker_ring.total_byte_count * 1.0 / 1000000, worker_ring.total_byte_count * 1.0 / msecs_time, worker_ring.total_count * 1.0 / msecs_time, worker_ring.total_feedback_batch_num * 1.0 / worker_ring.total_feedback_count, worker_ring.total_rx_batch_num * 1.0 / worker_ring.total_rx_returned_count, worker_ring.total_tx_batch_num * 1.0 / worker_ring.total_tx_returned_count, worker_ring.total_worktime * 1.0 / (worker_ring.total_worktime_count * 1000.0));
			worker_ring.total_worktime = 0;
			worker_ring.total_worktime_count = 0;
			worker_ring.total_feedback_batch_num = 0;
			worker_ring.total_feedback_count = 0;
			worker_ring.total_rx_batch_num = 0;
			worker_ring.total_rx_returned_count = 0;
			worker_ring.total_tx_batch_num = 0;
			worker_ring.total_tx_returned_count = 0;
			worker_ring.total_count = 0;
			worker_ring.total_byte_count = 0;
			gettimeofday(&startt, NULL);
		}
	}
}

int main(int argc, char *argv[])
{
	cpu_set_t cpuset;
	pthread_t thread[MQNIC_USER_QUEUE_NUMBER];
	struct poll_struct poll_info[MQNIC_USER_QUEUE_NUMBER];
	if (argc != 2)
	{
		printf("Usage: %s <pci bus id> \n", argv[0]);
		return 1;
	}
	printf("Currently only work for interface 0\n");

	struct ixy_device *dev = ixy_init(argv[1], MQNIC_USER_QUEUE_NUMBER, MQNIC_USER_QUEUE_NUMBER, 0);

	config_app_mat(dev, 1, 5678, 1);
	config_app_mat(dev, 2, 1234, 2);

	mqnic_port_reset_monitor(dev);

	// we disable the core reallocation feature in this testing
	// mqnic_port_set_monitor(dev, 1, 14);
	// mqnic_port_set_monitor(dev, 2, 14);

	for (int i = 0; i < MQNIC_USER_QUEUE_NUMBER; i++)
	{
		CPU_ZERO(&cpuset);
		CPU_SET(i, &cpuset);

		poll_info[i].dev = dev;
		poll_info[i].queue_id = i;
		if (pthread_create(&thread[i], NULL, poll_queue, &poll_info[i]))
		{
			printf("Create Thread Error\n");
		}

		if (pthread_setaffinity_np(thread[i], sizeof(cpu_set_t), &cpuset))
		{
			printf("Set affinity Error\n");
		}
	}

	for (int i = 0; i < MQNIC_USER_QUEUE_NUMBER; i++)
	{
		pthread_join(thread[i], NULL);
	}
	return 0;
}
