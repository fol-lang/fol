#ifndef TALLY_H
#define TALLY_H

/* The context is the LAST callback parameter, not the first. V4 imports one
 * canonical shape and refuses this one rather than guessing which argument the
 * provider means to hand back. */
int tally_range(int upto,
                int (*step)(int accumulator, int value, void *context),
                void *context);

#endif
