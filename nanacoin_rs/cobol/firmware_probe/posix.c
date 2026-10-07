/* Investigation profile: no login database or POSIX process signals.
 * Return ENOSYS rather than pretending those services exist. Scheduling uses
 * ESP-IDF, and actual filesystem calls are provided by ESP-IDF VFS.
 */
#include <errno.h>
#include <signal.h>
char *getlogin(void) { errno = ENOSYS; return 0; }
void (*signal(int sig, void (*handler)(int)))(int) {
    (void)sig; (void)handler; errno = ENOSYS; return SIG_ERR;
}
