#ifndef TALLY_H
#define TALLY_H

/* A synchronous callback: invoked once per step during the call, and never
 * retained afterwards. The context is handed straight back to the callback. */
int tally_range(int upto,
                int (*step)(void *context, int accumulator, int value),
                void *context);

#endif
