#include "c_math.h"

int c_math_add_one(int value) {
    return value + 1;
}

int c_math_checked_div(int lhs, int rhs, int *result) {
    if (rhs == 0) {
        return 1;
    }
    *result = lhs / rhs;
    return 0;
}
