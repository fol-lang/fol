#include "text.h"
uint32_t text_len(const char *s){ uint32_t n=0; while(s[n]) n++; return n; }
uint32_t text_first(const char *s){ return (uint32_t)(unsigned char)s[0]; }
