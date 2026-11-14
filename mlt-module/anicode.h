#ifndef ANICODE_H
#define ANICODE_H

#include <stdint.h>

/* The Rust engine (crates/anicode-ffi). A handle must not be used from two
 * threads at once. */
typedef struct AnicodeEngine anicode_engine;

/* Returns NULL only for an invalid path string. On a script error the handle
 * is still returned and anicode_last_error says why. */
anicode_engine *anicode_open(const char *scene_path);

/* 1 = reloaded, 0 = unchanged, -1 = script error (the previous scene stays). */
int anicode_reload_if_changed(anicode_engine *engine);

/* Scene length in seconds, or 0 if no scene has loaded. */
double anicode_duration(anicode_engine *engine);

/* Draws the frame at `time` seconds as straight-alpha RGBA. 0 = ok, -1 = error. */
int anicode_render(anicode_engine *engine, double time, int width, int height, uint8_t *rgba,
                   int stride);

/* "file:line: message", or "" when there is no error. */
const char *anicode_last_error(anicode_engine *engine);

void anicode_close(anicode_engine *engine);

#endif
