#include <stdlib.h>
#include "widget.h"

struct widget { int seed; };

struct widget *widget_new(int seed) {
    struct widget *w = malloc(sizeof(struct widget));
    if (w) { w->seed = seed; }
    return w;
}

int widget_size(const struct widget *w) { return w ? w->seed * 2 : -1; }

void widget_free(struct widget *w) { free(w); }
