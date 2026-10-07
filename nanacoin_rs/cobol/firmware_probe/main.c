#include <stdio.h>
#include <libcob.h>
#include "esp_heap_caps.h"
#include "posix.c"
#include "allocations.c"
#ifdef NC_ABI_VECTORS
#include "vectors.h"
#endif
extern int nc_rust_probe(void);

void app_main(void) {
    // A single task initializes the runtime and makes the direct Rust/C call.
    // This image is only built, never flashed or executed by the probe tool.
    heap_caps_monitor_local_minimum_free_size_start();
    size_t before = heap_caps_get_free_size(MALLOC_CAP_8BIT);
    nc_probe_task = xTaskGetCurrentTaskHandle();
    nc_track = 1;
    cob_init(0, NULL);
    size_t after_init = heap_caps_get_free_size(MALLOC_CAP_8BIT);
    int result = nc_rust_probe();
    unsigned calls = result ? (unsigned)-result : 3;
    size_t after_warm = heap_caps_get_free_size(MALLOC_CAP_8BIT);
    size_t after_first = after_warm, after_second = after_warm;
#ifdef NC_ABI_VECTORS
    if (result == 0) result = nc_run_vectors(&calls);
    after_first = heap_caps_get_free_size(MALLOC_CAP_8BIT);
    if (result == 0) result = nc_run_vectors(&calls);
    after_second = heap_caps_get_free_size(MALLOC_CAP_8BIT);
#endif
    for (unsigned i = 0; i < 128 && result == 0; ++i) {
        result = nc_rust_probe();
        calls += result ? (unsigned)-result : 3;
    }
    nc_track = 0;
    size_t after_repeat = heap_caps_get_free_size(MALLOC_CAP_8BIT);
    size_t low = heap_caps_get_minimum_free_size(MALLOC_CAP_8BIT);
    unsigned stack_free = uxTaskGetStackHighWaterMark(NULL);
    heap_caps_monitor_local_minimum_free_size_stop();
    printf("NC_PROBE {\"result\":%d,\"calls\":%u,\"heap_before\":%u,"
           "\"heap_after_init\":%u,\"heap_after_warm\":%u,\"heap_after_repeat\":%u,"
           "\"heap_after_vectors_first\":%u,\"heap_after_vectors_second\":%u,"
           "\"heap_low\":%u,\"stack_free_bytes\":%u,\"allocation_calls\":%u,"
           "\"free_calls\":%u,\"requested_bytes\":%llu}\n",
           result, calls, (unsigned)before, (unsigned)after_init, (unsigned)after_warm,
           (unsigned)after_repeat, (unsigned)after_first, (unsigned)after_second, (unsigned)low, stack_free,
           nc_alloc_calls, nc_free_calls, (unsigned long long)nc_requested_bytes);
    puts("NC_PROBE_DONE");
}
