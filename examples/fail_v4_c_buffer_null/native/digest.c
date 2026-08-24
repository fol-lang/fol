#include "digest.h"
#include <stdlib.h>

/* Returns no buffer and claims three elements. There is no memory the count
 * could be describing, so there is nothing honest to hand back. */
uint8_t *digest_take(uint8_t start, size_t *out_len, size_t *out_capacity) {
    (void)start;
    *out_len = 3;
    *out_capacity = 3;
    return 0;
}

void digest_release(uint8_t *bytes, size_t count) {
    (void)count;
    free(bytes);
}
