#include "digest.h"

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
