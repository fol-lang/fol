#ifndef FOL_DIGEST_H
#define FOL_DIGEST_H

#include <stdint.h>
#include <stddef.h>

/* A buffer, the way C carries one: an address and a count, with nothing in the
 * type system saying they belong together. */
uint32_t digest_sum(const uint8_t *bytes, size_t count);

/* The same pair, written through rather than read. */
void digest_fill(uint8_t *bytes, size_t count, uint8_t value);

/* A buffer the provider allocates and only the provider can free. FOL never
 * adopts this memory: it is validated, copied out of, and released. */
uint8_t *digest_take(uint8_t start, size_t *out_len, size_t *out_capacity);
void digest_release(uint8_t *bytes, size_t count);

/* Allocations this provider has made and not yet freed. FOL cannot see C's
 * heap, so the provider is asked. */
uint32_t digest_live(void);

#endif
