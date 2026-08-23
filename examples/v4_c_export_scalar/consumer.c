/* The M5 real consumer.
 *
 * Uses only the installed prefix: the generated header and the installed
 * library. No Rust or FOL internals, no hand-written declarations. */
#include <stdio.h>
#include "v4_c_export_scalar.h"

#define CHECK(expr) do { if (!(expr)) { printf("FAIL: %s\n", #expr); return 1; } } while (0)

int main(void) {
    /* Every signed and unsigned width. */
    int8_t i8 = 0;
    CHECK(fol_slice_add_i8(20, 22, &i8) == FOL_STATUS_OK && i8 == 42);
    int16_t i16 = 0;
    CHECK(fol_slice_add_i16(300, 44, &i16) == FOL_STATUS_OK && i16 == 344);
    int32_t i32 = 0;
    CHECK(fol_slice_add_i32(70000, 1, &i32) == FOL_STATUS_OK && i32 == 70001);
    int64_t i64 = 0;
    CHECK(fol_slice_add_i64(5000000000LL, 1, &i64) == FOL_STATUS_OK && i64 == 5000000001LL);

    uint8_t u8 = 0;
    CHECK(fol_slice_add_u8(200, 55, &u8) == FOL_STATUS_OK && u8 == 255);
    uint16_t u16 = 0;
    CHECK(fol_slice_add_u16(60000, 1, &u16) == FOL_STATUS_OK && u16 == 60001);
    uint32_t u32 = 0;
    CHECK(fol_slice_add_u32(4000000000U, 1, &u32) == FOL_STATUS_OK && u32 == 4000000001U);
    uint64_t u64 = 0;
    CHECK(fol_slice_add_u64(10000000000ULL, 1, &u64) == FOL_STATUS_OK && u64 == 10000000001ULL);

    /* Both float widths. */
    float f32 = 0.0f;
    CHECK(fol_slice_scale_f32(2.5f, 4.0f, &f32) == FOL_STATUS_OK && f32 == 10.0f);
    double f64 = 0.0;
    CHECK(fol_slice_scale_f64(2.5, 4.0, &f64) == FOL_STATUS_OK && f64 == 10.0);

    /* ABI boolean: 0 and 1 are the only valid inputs. */
    fol_bool_t flag = 9;
    CHECK(fol_slice_negate(0, &flag) == FOL_STATUS_OK && flag == 1);
    CHECK(fol_slice_negate(1, &flag) == FOL_STATUS_OK && flag == 0);
    CHECK(fol_slice_negate(2, &flag) == FOL_STATUS_INVALID_ARGUMENT);
    CHECK(fol_slice_negate(255, &flag) == FOL_STATUS_INVALID_ARGUMENT);

    /* UTF-32 character: a surrogate or out-of-range code point is refused. */
    int64_t code = 0;
    CHECK(fol_slice_code_point(0x41, &code) == FOL_STATUS_OK && code == 0x41);
    CHECK(fol_slice_code_point(0x1F600, &code) == FOL_STATUS_OK && code == 0x1F600);
    CHECK(fol_slice_code_point(0xD800, &code) == FOL_STATUS_INVALID_ARGUMENT);
    CHECK(fol_slice_code_point(0x110000, &code) == FOL_STATUS_INVALID_ARGUMENT);

    /* A no-value result still returns a status. */
    CHECK(fol_slice_touch(7) == FOL_STATUS_OK);

    /* A null required out pointer is refused. */
    CHECK(fol_slice_add_i64(1, 2, NULL) == FOL_STATUS_INVALID_ARGUMENT);

    /* A recoverable report returns 1 and initializes ONLY the error out.
       The success out is seeded with a sentinel the callee must not touch. */
    int64_t quotient = -12345;
    int64_t error = 0;
    CHECK(fol_slice_checked_div(84, 2, &quotient, &error) == FOL_STATUS_OK);
    CHECK(quotient == 42);

    quotient = -12345;
    CHECK(fol_slice_checked_div(1, 0, &quotient, &error) == FOL_STATUS_REPORT);
    CHECK(error == 7);
    CHECK(quotient == -12345); /* untouched on the report path */

    /* A panic is contained and reported, never unwound into C. */
    int64_t unused = 0;
    CHECK(fol_slice_always_panics(1, &unused) == FOL_STATUS_PANIC);

    /* Reaching here proves the panic did not unwind through this frame. */
    CHECK(fol_slice_add_i64(1, 1, &i64) == FOL_STATUS_OK && i64 == 2);

    printf("all scalar exports ok\n");
    return 0;
}
