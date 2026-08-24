/* A C consumer of a FOL-owned resource.
 *
 * `fol_session_t` is declared and never defined, so this file cannot read
 * through the pointer or copy what is behind it. It can hold the address,
 * hand it back, and release it once -- which is the whole contract. */
#include <stdio.h>
#include <stdlib.h>
#include "v4_c_export_handle.h"

static int failures = 0;

static void check(const char *what, long long got, long long want) {
    if (got != want) {
        printf("FAIL %s: got %lld want %lld\n", what, got, want);
        failures += 1;
    }
}

int main(void) {
    fol_session_t *session = NULL;
    check("open status", fol_session_open(21, &session), FOL_STATUS_OK);
    if (session == NULL) {
        printf("FAIL open produced no handle\n");
        return 1;
    }

    /* Borrowing does not consume: the handle is still ours afterwards. */
    int32_t size = 0;
    check("size status", fol_session_size(session, &size), FOL_STATUS_OK);
    check("size value", size, 42);

    int32_t again = 0;
    check("second borrow status", fol_session_size(session, &again), FOL_STATUS_OK);
    check("second borrow value", again, 42);

    /* Releasing consumes it exactly once. */
    int32_t seed = 0;
    check("close status", fol_session_close(session, &seed), FOL_STATUS_OK);
    check("close value", seed, 21);

    /* A null handle is a caller error the wrapper refuses rather than
     * dereferences. */
    int32_t ignored = 0;
    check("null borrow", fol_session_size(NULL, &ignored),
          FOL_STATUS_INVALID_ARGUMENT);

    if (failures != 0) {
        printf("%d handle check(s) failed\n", failures);
        return 1;
    }
    printf("all handle checks passed\n");
    return 0;
}
