#ifndef P_H
#define P_H
#include <stdint.h>
/* The qsort/lua_CFunction shape: a function pointer and no context at all. */
int32_t fold_range(int32_t upto, int32_t (*step)(int32_t value));
#endif
