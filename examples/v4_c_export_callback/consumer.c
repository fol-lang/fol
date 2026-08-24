/* C supplying a routine that FOL calls during the call.
 *
 * The context is how C carries state into a callback. Here it counts the
 * invocations, which is what proves FOL really called back rather than
 * computing the answer itself. */
#include <stdio.h>
#include "v4_c_export_callback.h"

static int failures = 0;

static void check(const char *what, long long got, long long want) {
    if (got != want) {
        printf("FAIL %s: got %lld want %lld\n", what, got, want);
        failures += 1;
    }
}

struct tally { int calls; };

static int32_t add(void *context, int32_t accumulator, int32_t value) {
    struct tally *state = (struct tally *)context;
    state->calls += 1;
    return accumulator + value;
}

int main(void) {
    struct tally state = { 0 };
    int32_t total = 0;
    check("status", fol_fold(20, 22, add, &state, &total), FOL_STATUS_OK);
    /* 0 + 20, then 20 + 22. */
    check("total", total, 42);
    /* Two calls, and the context arrived intact both times. */
    check("callbacks", state.calls, 2);

    /* A null callback is refused rather than called through. */
    int32_t ignored = 0;
    check("null callback", fol_fold(1, 2, NULL, &state, &ignored),
          FOL_STATUS_INVALID_ARGUMENT);

    if (failures != 0) {
        printf("%d callback check(s) failed\n", failures);
        return 1;
    }
    printf("all callback checks passed\n");
    return 0;
}
