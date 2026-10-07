/* ESP-IDF has no login database or POSIX process signals. File and timer
 * services come from ESP-IDF; Rust owns bank identities and scheduling.
 * Explicitly refuse unsupported services rather than installing dummy success.
 */
#include <errno.h>
#include <signal.h>
#include <sys/types.h>
char *getlogin(void) { errno = ENOSYS; return 0; }
void (*signal(int sig, void (*handler)(int)))(int) {
    (void)sig; (void)handler; errno = ENOSYS; return SIG_ERR;
}
/* GnuCOBOL includes CBL_GC_FORK in its system routine table. FreeRTOS has
 * threads, not Unix processes; newlib's fork wrapper must fail explicitly.
 */
pid_t _fork(void) { errno = ENOSYS; return -1; }
