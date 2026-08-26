#include "fold.h"
int32_t fold_range(int32_t upto, int32_t (*step)(int32_t value)){ int32_t t=0; for(int32_t v=1; v<=upto; v++) t += step(v); return t; }
