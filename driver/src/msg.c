#include "msg.h"
#include <stdio.h>
#include <unistd.h>
#include <sys/time.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>

uint8_t process_work(uint8_t *data, uint8_t enable_work, uint8_t if_preemptive, uint32_t p_interval)
{
    struct timespec workstartn, workendn;
    clock_gettime(CLOCK_MONOTONIC, &workstartn);

    struct Request req;
    memcpy(&req, (char *)data, sizeof(struct Request));
    uint32_t worker_ns = req.runNs;
    uint32_t runnable_worker_ns = ((p_interval < worker_ns) && (if_preemptive == 1)) ? p_interval : worker_ns;

    if (runnable_worker_ns > 0 && enable_work)
    {
        req.runNs = worker_ns - runnable_worker_ns;
        memcpy((char *)data, &req, sizeof(struct Request));

        while (1)
        {
            clock_gettime(CLOCK_MONOTONIC, &workendn);
            long nano_time = ((workendn.tv_sec - workstartn.tv_sec) * 1.0e9) + ((workendn.tv_nsec - workstartn.tv_nsec));
            if (nano_time >= runnable_worker_ns)
            {
                // printf("Nano time %d", nano_time);
                break;
            }
        }
    }
    else
        runnable_worker_ns = worker_ns;

    return runnable_worker_ns == worker_ns;
}
