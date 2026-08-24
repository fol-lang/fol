#include "digest.h"
#include <stdlib.h>

uint32_t digest_sum(const uint8_t *bytes, size_t count) {
    uint32_t total = 0;
    for (size_t index = 0; index < count; index += 1) {
        total += (uint32_t)bytes[index];
    }
    return total;
}

void digest_fill(uint8_t *bytes, size_t count, uint8_t value) {
    for (size_t index = 0; index < count; index += 1) {
        bytes[index] = value;
    }
}

static uint32_t live = 0;

uint8_t *digest_take(uint8_t start, size_t *out_len, size_t *out_capacity) {
    size_t capacity = 8;
    uint8_t *bytes = malloc(capacity);
    if (bytes == 0) {
        *out_len = 0;
        *out_capacity = 0;
        return 0;
    }
    for (size_t index = 0; index < 3; index += 1) {
        bytes[index] = (uint8_t)(start + index);
    }
    *out_len = 3;
    *out_capacity = capacity;
    live += 1;
    return bytes;
}

void digest_release(uint8_t *bytes, size_t count) {
    (void)count;
    live -= 1;
    free(bytes);
}

uint32_t digest_live(void) {
    return live;
}
