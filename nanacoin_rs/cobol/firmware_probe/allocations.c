/* Account for libc allocations requested by this task during the probe.
 * Requested bytes are cumulative traffic, not retained or peak live memory;
 * ESP-IDF's heap monitor separately measures the local free-heap low point.
 */
#include <stdint.h>
#include <stdlib.h>
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
static TaskHandle_t nc_probe_task;
static int nc_track;
static unsigned nc_alloc_calls, nc_free_calls;
static uint64_t nc_requested_bytes;
extern void *__real_malloc(size_t);
extern void *__real_calloc(size_t, size_t);
extern void *__real_realloc(void *, size_t);
extern void __real_free(void *);
static void nc_allocation(size_t bytes) {
    if (nc_track && xTaskGetCurrentTaskHandle() == nc_probe_task) {
        ++nc_alloc_calls;
        nc_requested_bytes += bytes;
    }
}
void *__wrap_malloc(size_t bytes) { nc_allocation(bytes); return __real_malloc(bytes); }
void *__wrap_calloc(size_t count, size_t bytes) {
    nc_allocation(count * bytes); return __real_calloc(count, bytes);
}
void *__wrap_realloc(void *ptr, size_t bytes) {
    nc_allocation(bytes); return __real_realloc(ptr, bytes);
}
void __wrap_free(void *ptr) {
    if (nc_track && ptr && xTaskGetCurrentTaskHandle() == nc_probe_task) ++nc_free_calls;
    __real_free(ptr);
}
