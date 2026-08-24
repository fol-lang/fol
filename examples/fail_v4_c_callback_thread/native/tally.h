#ifndef FOL_TALLY_H
#define FOL_TALLY_H

int tally_range(int upto,
                int (*step)(void *context, int accumulator, int value),
                void *context);

#endif
