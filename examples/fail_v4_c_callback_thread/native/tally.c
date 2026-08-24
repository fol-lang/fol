#include "tally.h"
#include <pthread.h>

/* A provider that hands the callback to another thread and waits for it.
 *
 * The closure is still alive: `tally_range` has not returned, so the stack
 * local it lives in is intact and the context pointer is valid. What is not
 * valid is the thread. FOL lends a closure for the duration of one call on
 * one thread, and a closure reached from a second thread is reached without
 * any of the synchronisation that would make it safe. */
struct lent {
    int (*step)(void *, int, int);
    void *context;
    int accumulator;
    int value;
};

static void *invoke_elsewhere(void *raw) {
    struct lent *lent = (struct lent *)raw;
    lent->accumulator = lent->step(lent->context, lent->accumulator, lent->value);
    return 0;
}

int tally_range(int upto,
                int (*step)(void *context, int accumulator, int value),
                void *context) {
    struct lent lent = { step, context, 0, upto };
    pthread_t worker;
    pthread_create(&worker, 0, invoke_elsewhere, &lent);
    pthread_join(worker, 0);
    return lent.accumulator;
}
