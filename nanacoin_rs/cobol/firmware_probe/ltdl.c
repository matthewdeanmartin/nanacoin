/* libcob's static module registration remains intact. Loading always fails.
 * This adapter is for the compile/link investigation, not Windows DLL builds.
 */
#include "ltdl.h"
int lt_dlinit(void) { return 0; }
int lt_dlexit(void) { return 0; }
lt_dlhandle lt_dlopen(const char *path) { (void)path; return 0; }
void *lt_dlsym(lt_dlhandle module, const char *symbol) {
    (void)module; (void)symbol; return 0;
}
int lt_dlclose(lt_dlhandle module) { (void)module; return 0; }
const char *lt_dlerror(void) {
    return "dynamic modules unavailable in static-only probe";
}
