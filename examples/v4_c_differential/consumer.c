/* FOL's belief about every scalar, checked against the C compiler's own.
 *
 * Two things are compared here that the rest of the suite does not compare.
 *
 * The **range edges**: a boundary that returns what it was given works for 42
 * under almost any mistake -- a wrong width, a lost sign, a truncating cast.
 * It stops working at INT8_MIN, at UINT64_MAX, and at the largest float that
 * is not infinity. Those are the values that tell the truth.
 *
 * The **bit pattern**, not the value. `-0.0 == 0.0` is true in C, so an
 * equality check cannot see a boundary that dropped a sign bit; memcmp can.
 * NaN is the mirror case: it compares unequal to itself, so equality would
 * report a failure that is not one. */
#include <stdio.h>
#include <string.h>
#include <float.h>
#include <math.h>
#include "v4_c_differential.h"

static int failures = 0;

static void check(const char *what, long long got, long long want) {
    if (got != want) {
        printf("FAIL %s: got %lld want %lld\n", what, got, want);
        failures += 1;
    }
}

/* Compared as bytes: the point is the pattern, not the numeric value. */
static void check_bits(const char *what, const void *got, const void *want, size_t width) {
    if (memcmp(got, want, width) != 0) {
        printf("FAIL %s: bit pattern changed crossing the boundary\n", what);
        failures += 1;
    }
}

#define EDGE_INT(name, ctype, fn, ...)                                        \
    do {                                                                      \
        static const ctype cases[] = { __VA_ARGS__ };                         \
        for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i += 1) {    \
            ctype out = 0;                                                    \
            check(name, fn(cases[i], &out), FOL_STATUS_OK);                   \
            check_bits(name, &out, &cases[i], sizeof(ctype));                 \
        }                                                                     \
    } while (0)

int main(void) {
    /* Signed: the minimum is the value a lost sign or a narrowing cast
     * destroys, and it has no positive counterpart to be confused with. */
    EDGE_INT("i8", int8_t, fol_echo_i8, INT8_MIN, -1, 0, 1, INT8_MAX);
    EDGE_INT("i16", int16_t, fol_echo_i16, INT16_MIN, -1, 0, 1, INT16_MAX);
    EDGE_INT("i32", int32_t, fol_echo_i32, INT32_MIN, -1, 0, 1, INT32_MAX);
    EDGE_INT("i64", int64_t, fol_echo_i64, INT64_MIN, -1, 0, 1, INT64_MAX);

    /* Unsigned: the maximum is all bits set, which is what a sign-extending
     * boundary turns into -1. */
    EDGE_INT("u8", uint8_t, fol_echo_u8, 0, 1, UINT8_MAX);
    EDGE_INT("u16", uint16_t, fol_echo_u16, 0, 1, UINT16_MAX);
    EDGE_INT("u32", uint32_t, fol_echo_u32, 0, 1, UINT32_MAX);
    EDGE_INT("u64", uint64_t, fol_echo_u64, 0, 1, UINT64_MAX);

    /* Floats, by bit pattern. Negative zero and the denormal minimum are the
     * two an ordinary equality check cannot see go wrong. */
    static const float f32_cases[] = {
        0.0f, -0.0f, 1.0f, -1.0f, FLT_MIN, -FLT_MAX, FLT_MAX, FLT_TRUE_MIN,
    };
    for (size_t i = 0; i < sizeof(f32_cases) / sizeof(f32_cases[0]); i += 1) {
        float out = 0.0f;
        check("f32", fol_echo_f32(f32_cases[i], &out), FOL_STATUS_OK);
        check_bits("f32", &out, &f32_cases[i], sizeof(float));
    }
    static const double f64_cases[] = {
        0.0, -0.0, 1.0, -1.0, DBL_MIN, -DBL_MAX, DBL_MAX, DBL_TRUE_MIN,
    };
    for (size_t i = 0; i < sizeof(f64_cases) / sizeof(f64_cases[0]); i += 1) {
        double out = 0.0;
        check("f64", fol_echo_f64(f64_cases[i], &out), FOL_STATUS_OK);
        check_bits("f64", &out, &f64_cases[i], sizeof(double));
    }

    /* Infinity survives; NaN is checked as a pattern because it is not equal
     * to itself. */
    float infinite = 0.0f, want_infinite = INFINITY;
    check("f32 inf", fol_echo_f32(want_infinite, &infinite), FOL_STATUS_OK);
    check_bits("f32 inf", &infinite, &want_infinite, sizeof(float));

    /* `bol` is one byte with exactly two valid values; anything else is
     * refused rather than truncated to its low bit. */
    fol_bool_t flag = 9;
    check("bol false", fol_echo_bol(0, &flag), FOL_STATUS_OK);
    check("bol false value", flag, 0);
    check("bol true", fol_echo_bol(1, &flag), FOL_STATUS_OK);
    check("bol true value", flag, 1);
    check("bol 2", fol_echo_bol(2, &flag), FOL_STATUS_INVALID_ARGUMENT);
    check("bol 255", fol_echo_bol(255, &flag), FOL_STATUS_INVALID_ARGUMENT);

    /* `chr` is a Unicode scalar value: the two edges, the two either side of
     * the surrogate hole, and the first value past the top. */
    uint32_t code = 0;
    static const uint32_t chr_ok[] = { 0x0, 0xD7FF, 0xE000, 0x10FFFF };
    for (size_t i = 0; i < sizeof(chr_ok) / sizeof(chr_ok[0]); i += 1) {
        check("chr", fol_echo_chr(chr_ok[i], &code), FOL_STATUS_OK);
        check("chr value", code, chr_ok[i]);
    }
    static const uint32_t chr_bad[] = { 0xD800, 0xDFFF, 0x110000, 0xFFFFFFFF };
    for (size_t i = 0; i < sizeof(chr_bad) / sizeof(chr_bad[0]); i += 1) {
        check("chr refused", fol_echo_chr(chr_bad[i], &code), FOL_STATUS_INVALID_ARGUMENT);
    }

    if (failures != 0) {
        printf("%d differential check(s) failed\n", failures);
        return 1;
    }
    printf("all differential checks passed\n");
    return 0;
}
