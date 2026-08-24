#ifndef FOL_DIGEST_H
#define FOL_DIGEST_H

#include <stdint.h>
#include <stddef.h>

/* A buffer, the way C carries one: an address and a count, with nothing in the
 * type system saying they belong together. */
uint32_t digest_sum(const uint8_t *bytes, size_t count);

/* The same pair, written through rather than read. */
void digest_fill(uint8_t *bytes, size_t count, uint8_t value);

#endif
