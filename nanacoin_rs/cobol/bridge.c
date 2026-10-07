/* Fixed-width C ABI between Rust and GnuCOBOL's generated C.
 * Rust holds one mutex across every entry, including runtime initialization.
 * The DLL stays loaded for the process lifetime; no cob_tidy while in use.
 */
#include <stdint.h>
#include <libcob.h>

#ifdef _WIN32
#define NC_EXPORT __declspec(dllexport)
#else
#define NC_EXPORT
#endif

extern int NCBANK(unsigned char *, unsigned char *, unsigned char *);

NC_EXPORT int32_t nc_bank_abi(void) { return 7; }
NC_EXPORT int32_t nc_bank_slots(void) { return 64; }
NC_EXPORT uint64_t nc_bank_capabilities(void) { return UINT64_C(0xFFFFFFFFFFFFFFFE); }

NC_EXPORT int32_t nc_bank_v7(int32_t operation, int64_t *slots)
{
    static int initialized = 0;
    int32_t result = -1;
    if (!slots) return 1;
    if (!initialized) {
        cob_init(0, NULL);
        initialized = 1;
    }
    NCBANK((unsigned char *)&operation, (unsigned char *)slots,
           (unsigned char *)&result);
    return result;
}
