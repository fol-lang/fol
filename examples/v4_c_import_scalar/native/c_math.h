#ifndef V4_C_MATH_H
#define V4_C_MATH_H

/* An infallible call: every input has an answer. */
int c_math_add_one(int value);

/* A fallible call, C-style: an integer status plus a typed out-parameter.
   Returns 0 on success and 1 when the divisor is zero. */
int c_math_checked_div(int lhs, int rhs, int *result);

#endif
