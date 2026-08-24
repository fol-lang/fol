/* An entry crossing the C boundary with the tags FOL wrote down.
 *
 * The load-bearing check is the round trip: FOL produces a variant, C reads
 * its tag against the header's constant, and hands the same value back for FOL
 * to read as a variant again. A tag the two sides disagreed on would fail on
 * one side or the other -- which is exactly how the original mismatch was
 * caught, when FOL evaluated a variant as 7 and the header declared it 1. */
#include <stdio.h>
#include "v4_c_export_entry.h"

static int failures = 0;

static void check(const char *what, long long got, long long want) {
    if (got != want) {
        printf("FAIL %s: got %lld want %lld\n", what, got, want);
        failures += 1;
    }
}

/* One variant, both directions: FOL produces it, C names it, FOL reads it. */
static void round_trip(const char *what, int32_t code, fol_lookup_tag_t tag,
                       int32_t weight) {
    fol_lookup_t state;
    int32_t got = 0;
    check(what, fol_classify(code, &state), FOL_STATUS_OK);
    check(what, state.tag, tag);
    check(what, fol_weight(state, &got), FOL_STATUS_OK);
    check(what, got, weight);
}

int main(void) {
    /* The tags are not 0/1/2 and not in declaration order, so a positional
     * numbering would disagree here on every variant. */
    check("missing tag", FOL_LOOKUP_MISSING, 4);
    check("found tag", FOL_LOOKUP_FOUND, 1);
    check("denied tag", FOL_LOOKUP_DENIED, 9);

    round_trip("found", 1, FOL_LOOKUP_FOUND, 10);
    round_trip("denied", 2, FOL_LOOKUP_DENIED, 20);
    round_trip("missing", 0, FOL_LOOKUP_MISSING, 30);

    /* A tag naming no variant is not an entry value, and reading the struct
     * for one would be reading whatever bytes happen to be there. */
    fol_lookup_t bogus;
    int32_t ignored = 0;
    bogus.tag = (fol_lookup_tag_t)7;
    check("unknown tag refused", fol_weight(bogus, &ignored),
          FOL_STATUS_INVALID_ARGUMENT);

    if (failures != 0) {
        printf("%d check(s) failed\n", failures);
        return 1;
    }
    printf("all entry checks passed\n");
    return 0;
}
