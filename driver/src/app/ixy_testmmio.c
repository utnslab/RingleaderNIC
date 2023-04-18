#define _GNU_SOURCE
#include <stdio.h>
#include <unistd.h>
#include <sys/time.h>
#include <pthread.h>
#include <stdlib.h>

#include "driver/device.h"
#include "driver/mqnic_type.h"
#include "driver/mqnic.h"

const int BATCH_SIZE = 32;
const int FEEDBACK_BATCH_SIZE = 1;
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

void *poll_queue(void *context)
{
	struct poll_struct *poll_info = (struct poll_struct *)context;
	struct ixy_device *dev = poll_info->dev;
	uint32_t queue_id = poll_info->queue_id;

	printf("Launch Poll Thread, %d\n", queue_id);
	// pthread_exit(NULL);

	struct timeval startt, endt;

	long total_returned_count = 0;

	gettimeofday(&startt, NULL);

	while (true)
	{
		total_returned_count += 1;
		mqnic_test_rx_mmio(dev, queue_id);

		if (total_returned_count > 10000000)
		{
			gettimeofday(&endt, NULL);
			long msecs_time = ((endt.tv_sec - startt.tv_sec) * 1000000.0) + ((endt.tv_usec - startt.tv_usec));
			printf("Queue: %d, throughput: %f Mpps\n", queue_id, total_returned_count * 1.0 / msecs_time);
			total_returned_count = 0;
			gettimeofday(&startt, NULL);
		}
	}
}

int main(int argc, char *argv[])
{
	cpu_set_t cpuset;
	pthread_t thread[MQNIC_USER_QUEUE_NUMBER];
	struct poll_struct poll_info[MQNIC_USER_QUEUE_NUMBER];
	if (argc < 3 || argc > 4)
	{
		printf("Usage: %s <pci bus id> <output file> [n packets]\n", argv[0]);
		return 1;
	}
	printf("Currently only work for interface 0\n");

	struct ixy_device *dev = ixy_init(argv[1], MQNIC_USER_QUEUE_NUMBER, MQNIC_USER_QUEUE_NUMBER, 0);

	for (int i = 0; i < MQNIC_USER_QUEUE_NUMBER; i++)
	{
		CPU_ZERO(&cpuset);
		CPU_SET((i) % 16, &cpuset);

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
