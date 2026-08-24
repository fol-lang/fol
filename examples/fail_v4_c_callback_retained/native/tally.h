#ifndef TALLY_H
#define TALLY_H

/* A synchronous callback, declared the same way the honest provider declares
 * it. Nothing in C says whether the provider keeps it. */
int tally_range(int upto,
                int (*step)(void *context, int accumulator, int value),
                void *context);

/* Invokes the callback `tally_range` was lent, after that call returned. */
int tally_replay(int value);

#endif
