#ifndef WIDGET_H
#define WIDGET_H


/* An opaque handle: the consumer never sees the definition. */
struct widget;

struct widget *widget_new(int seed);
int widget_size(const struct widget *w);
void widget_free(struct widget *w);

#endif
