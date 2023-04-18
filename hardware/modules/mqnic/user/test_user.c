
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include<sys/ioctl.h>
#include <endian.h>
 

#include <linux/kernel.h>
#include <linux/pci.h>
#include <linux/netdevice.h>
#include <linux/net_tstamp.h>
#include <sys/mman.h>
#include <stdint.h>
#include <math.h>

#include <linux/i2c.h>
#define MQNIC_IOCTL_TYPE 0x88
#define MQNIC_IOCTL_TEST _IOR(MQNIC_IOCTL_TYPE, 0x01, int32_t*)
#define MQNIC_IOCTL_INFO _IOR(MQNIC_IOCTL_TYPE, 0xf0, struct mqnic_ioctl_info)
static inline uint32_t get_reg32(const uint8_t* addr, int reg) {
	__asm__ volatile ("" : : : "memory");
	return *((volatile uint32_t*) (addr + reg));
}


struct mqnic_ioctl_info {
    uint32_t fw_id;
    uint32_t fw_ver;
    uint32_t board_id;
    uint32_t board_ver;
    uint32_t num_rx_queues;
    uint32_t num_event_queues;
    uint32_t rx_queue_offset;
    uint32_t rx_cpl_queue_offset;
    uint32_t num_tx_queues;
    uint32_t tx_queue_offset;
    uint32_t tx_cpl_queue_offset;
    uint32_t max_desc_block_size;
    uint32_t port_offset;
    size_t regs_size;
};

uint32_t round_power(uint32_t x){
    int power = 1;
    while (x >>= 1) power <<= 1;
    return power;
}

uint32_t log2_floor (uint32_t x)
{
    uint32_t res = -1;
    while (x) { res++ ; x = x >> 1; }
    return res;
}

int main()
{
        
        int test = 299;
        test = round_power(test);
        printf("Rounded %d", test);
        test = log2_floor(299);

        printf("floor %d", test);
        unsigned int i = 1;
        char *c = (char*)&i;
        if (*c)    
        printf("Little endian");
        else
        printf("Big endian");

        int fd;
        int32_t value, number;
        struct mqnic_ioctl_info mqnic_struct;

        printf("\nOpening Driver\n");
        fd = open("/dev/mqnic0", O_RDWR);
        if(fd < 0) {
                printf("Cannot open device file...\n");
                return 0;
        }
 
        printf("Writing Value to Driver\n");
        ioctl(fd, MQNIC_IOCTL_INFO, (struct mqnic_ioctl_info*) &mqnic_struct);
        printf("num_tx_queues Value is %x\n", mqnic_struct.num_tx_queues); 

        unsigned long * addr = (unsigned long *)mmap(NULL, mqnic_struct.regs_size,
        PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0x0);
        if (addr == MAP_FAILED)
        {
        perror("Failed to mmap: ");
        close(fd);
        return -1;
        }
        printf("mmap OK ? addr: %lx\n", addr);

        int x = get_reg32(addr, 0x0020);
        printf("IF COUNT OK ?: %d\n", x);

        int err = munmap(addr, mqnic_struct.regs_size);
        if(err != 0){
                printf("UnMapping Failed\n");
                return 1;
        }
 
        printf("Closing Driver\n");
        
        close(fd);
}