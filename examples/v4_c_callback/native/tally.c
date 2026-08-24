#include "tally.h"

int tally_range(int upto,
                int (*step)(void *context, int accumulator, int value),
                void *context) {
    int accumulator = 0;
    for (int value = 1; value <= upto; value += 1) {
        accumulator = step(context, accumulator, value);
    }
    return accumulator;
}
