/* A C consumer for FOL's borrowed string views.
 *
 * The interesting cases are the ones where C hands FOL something FOL must not
 * trust: a null pointer with a non-zero length, a length no allocation could
 * have, bytes that are not UTF-8. Each must come back as a refusal, not a
 * crash and not a wrong answer.
 *
 * The rest of the file is about the borrow itself. Every buffer here is heap
 * allocated and freed immediately after the call that lends it, so that if FOL
 * ever retained a pointer, AddressSanitizer would report the use-after-free
 * rather than the program quietly reading freed bytes.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "v4_c_string_view.h"

static int failures = 0;

static void check(const char *label, int condition) {
    if (!condition) {
        printf("FAIL %s\n", label);
        failures += 1;
    }
}

/* Lend a heap copy of `text`, then free it before returning. */
static fol_status_t length_of_heap_copy(const char *text, size_t len, int64_t *out) {
    uint8_t *buffer = malloc(len == 0 ? 1 : len);
    memcpy(buffer, text, len);
    fol_str_view_t view = { buffer, len };
    fol_status_t status = fol_view_text_length(view, out);
    free(buffer);
    return status;
}

static void accepted_views(void) {
    int64_t length = -1;

    check("ascii is accepted",
          length_of_heap_copy("hello", 5, &length) == FOL_STATUS_OK);
    check("ascii length is its byte count", length == 5);

    /* One scalar spelled in four bytes. `.len` is FOL's byte length, so the
     * answer is 4; what matters here is that the count came from the four
     * bytes lent and not from a byte past them, which is what the sanitizer
     * is watching for. */
    length = -1;
    check("multi-byte utf-8 is accepted",
          length_of_heap_copy("\xf0\x9f\xa6\x80", 4, &length) == FOL_STATUS_OK);
    check("multi-byte utf-8 is counted in bytes", length == 4);

    /* An interior NUL is data, not a terminator: the length is what decides. */
    length = -1;
    check("an interior nul is data",
          length_of_heap_copy("a\0b", 3, &length) == FOL_STATUS_OK);
    check("an interior nul is counted", length == 3);
}

static void empty_views(void) {
    int64_t length = -1;
    fol_bool_t empty = 2;

    /* A null pointer is legal when, and only when, the length is zero. */
    fol_str_view_t null_empty = { NULL, 0 };
    check("a null empty view is accepted",
          fol_view_text_length(null_empty, &length) == FOL_STATUS_OK);
    check("a null empty view has length zero", length == 0);
    check("a null empty view is empty",
          fol_view_is_empty(null_empty, &empty) == FOL_STATUS_OK && empty == 1);

    /* And a non-null zero-length view is the same string. FOL must not read
     * the pointer at all here; a one-byte allocation makes any read of
     * `ptr[0]` past the requested length visible to the sanitizer. */
    length = -1;
    check("a non-null empty view is accepted",
          length_of_heap_copy("", 0, &length) == FOL_STATUS_OK);
    check("a non-null empty view has length zero", length == 0);
}

static void refused_views(void) {
    int64_t length = 99;

    fol_str_view_t null_with_length = { NULL, 4 };
    check("a null pointer with a length is refused",
          fol_view_text_length(null_with_length, &length) == FOL_STATUS_INVALID_ARGUMENT);

    /* No allocation is this large, so the only honest answer is a refusal --
     * and computing a slice of this length would itself be undefined. */
    fol_str_view_t absurd = { (const uint8_t *)"x", (size_t)-1 };
    check("an impossible length is refused",
          fol_view_text_length(absurd, &length) == FOL_STATUS_INVALID_ARGUMENT);

    /* A lone continuation byte cannot start a UTF-8 sequence. */
    check("invalid utf-8 is refused",
          length_of_heap_copy("\x80\x80", 2, &length) == FOL_STATUS_INVALID_ARGUMENT);

    /* A truncated sequence: the leading byte promises three more bytes than
     * the view contains, which is exactly the shape that tempts an over-read. */
    check("truncated utf-8 is refused",
          length_of_heap_copy("\xf0\x9f\xa6", 3, &length) == FOL_STATUS_INVALID_ARGUMENT);

    check("a refused call leaves the out value untouched", length == 99);
}

static void unaligned_views(void) {
    /* A str view points at bytes, so every address is a legal one. Lending an
     * odd address proves the boundary does not quietly widen the read. */
    uint8_t *block = malloc(8);
    memcpy(block + 1, "odd", 3);
    fol_str_view_t view = { block + 1, 3 };
    int64_t length = -1;
    check("an unaligned view is accepted",
          fol_view_text_length(view, &length) == FOL_STATUS_OK);
    check("an unaligned view reads its own bytes", length == 3);
    free(block);
}

static void null_out_pointer(void) {
    fol_str_view_t view = { (const uint8_t *)"hello", 5 };
    check("a null out pointer is refused",
          fol_view_text_length(view, NULL) == FOL_STATUS_INVALID_ARGUMENT);
}

int main(void) {
    accepted_views();
    empty_views();
    refused_views();
    unaligned_views();
    null_out_pointer();

    if (failures != 0) {
        printf("%d string view check(s) failed\n", failures);
        return 1;
    }
    printf("all string view checks passed\n");
    return 0;
}
