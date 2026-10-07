/* Static-only investigation adapter. No dynamic module loading is supported. */
#ifndef NC_STATIC_LTDL_H
#define NC_STATIC_LTDL_H
typedef void *lt_dlhandle;
int lt_dlinit(void);
int lt_dlexit(void);
lt_dlhandle lt_dlopen(const char *);
void *lt_dlsym(lt_dlhandle, const char *);
int lt_dlclose(lt_dlhandle);
const char *lt_dlerror(void);
#endif
