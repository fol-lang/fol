#ifndef FOL_DIGEST_H
#define FOL_DIGEST_H

#include <stdint.h>
#include <stddef.h>

uint8_t *digest_take(uint8_t start, size_t *out_len, size_t *out_capacity);
void digest_release(uint8_t *bytes, size_t count);

#endif
