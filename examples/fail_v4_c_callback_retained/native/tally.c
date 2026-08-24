#include "tally.h"

/* A provider that keeps what it was lent. */
static int (*retained_step)(void *, int, int);
static void *retained_context;

int tally_range(int upto,
                int (*step)(void *context, int accumulator, int value),
                void *context) {
    int accumulator = 0;
    for (int value = 1; value <= upto; value += 1) {
        accumulator = step(context, accumulator, value);
    }
    /* The misuse: the callback was valid for this call and is kept anyway. */
    retained_step = step;
    retained_context = context;
    return accumulator;
}

int tally_replay(int value) {
    /* Called after `tally_range` returned. The FOL closure it reaches for is
     * gone; without a check this reads whatever reused that stack. */
    return retained_step(retained_context, 0, value);
}
