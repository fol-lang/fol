#include "digest.h"
#include <stdlib.h>

/* Reports four elements in a two-element allocation. Reading the buffer on
 * that report reads two bytes that were never allocated. */
uint8_t *digest_take(uint8_t start, size_t *out_len, size_t *out_capacity) {
    uint8_t *bytes = malloc(2);
    bytes[0] = start;
    bytes[1] = start;
    *out_len = 4;
    *out_capacity = 2;
    return bytes;
}

void digest_release(uint8_t *bytes, size_t count) {
    (void)count;
    free(bytes);
}
