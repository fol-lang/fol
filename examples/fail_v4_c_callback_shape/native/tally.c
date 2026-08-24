#include "tally.h"

int tally_range(int upto,
                int (*step)(int accumulator, int value, void *context),
                void *context) {
    int accumulator = 0;
    for (int value = 1; value <= upto; value += 1) {
        accumulator = step(accumulator, value, context);
    }
    return accumulator;
}
