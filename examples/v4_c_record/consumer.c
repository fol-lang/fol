/* A C consumer of FOL's exported record surface.
 *
 * The header's `_Static_assert`s do the layout half of the job: this file
 * fails to compile if the C compiler's own size, alignment, or offsets differ
 * from what FOL computed. What is left is behaviour -- that a struct passed
 * by value arrives with its fields intact, and one returned comes back the
 * same way. */
#include <stdio.h>
#include <stddef.h>
#include "v4_c_record.h"

static int failures = 0;

static void check(const char *what, long long got, long long want) {
    if (got != want) {
        printf("FAIL %s: got %lld want %lld\n", what, got, want);
        failures++;
    }
}

int main(void) {
    /* Built here in C and read in FOL: the struct crosses inbound. */
    fol_point_t point;
    point.zulu = 11;
    point.alpha = 31;
    point.mike = 7;

    int32_t x = 0;
    check("point_x status", fol_rec_point_x(point, &x), FOL_STATUS_OK);
    check("point_x value", x, 11);

    int32_t sum = 0;
    check("point_sum status", fol_rec_point_sum(point, &sum), FOL_STATUS_OK);
    check("point_sum value", sum, 42);

    /* Built in FOL and read here: the struct crosses outbound. */
    fol_point_t made;
    check("make_point status", fol_rec_make_point(4, 5, &made), FOL_STATUS_OK);
    check("make_point.zulu", made.zulu, 4);
    check("make_point.alpha", made.alpha, 5);
    check("make_point.mike", made.mike, 7);

    if (failures == 0) {
        printf("v4_c_record: all record checks passed\n");
    }
    return failures == 0 ? 0 : 1;
}
