/* demo.h -- the normative shape of a FOL-generated C header.
 *
 * This file is a checked-in reference, not generated output. Nothing produces
 * a header yet; V4 milestone M5 does, and what it emits must match this shape.
 * Freezing it here means the naming, include guard, typedefs, and status
 * values are decided once, in plan/V4_PLAN.md section 4.16, rather than being
 * invented by whoever writes the emitter.
 */

#ifndef FOL_DEMO_H
#define FOL_DEMO_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Every exported FOL routine returns this. Ordinary results travel through out
 * parameters, so an infallible-looking function still has a panic and
 * validation channel. */
typedef int32_t fol_status_t;

/* Only 0 and 1 are valid. Imports validate. */
typedef uint8_t fol_bool_t;

/* A Unicode scalar value. Imports validate. */
typedef uint32_t fol_char_t;

#define FOL_STATUS_OK                0
#define FOL_STATUS_REPORT            1
#define FOL_STATUS_INVALID_ARGUMENT (-1)
#define FOL_STATUS_PANIC            (-2)
#define FOL_STATUS_INTERNAL         (-3)

/* On any failure the success out values are left uninitialized. The caller
 * must not read or free them. On FOL_STATUS_REPORT, and only then, the typed
 * error out parameter is initialized. */

fol_status_t fol_demo_add(int64_t a, int64_t b, int64_t *out_result);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* FOL_DEMO_H */
