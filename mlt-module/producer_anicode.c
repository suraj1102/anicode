#include <framework/mlt.h>
#include <math.h>

#include "anicode.h"

// The `anicode` producer: renders a Lua scene with the Rust engine. The scene
// reloads when its file changes, so edits show on the next frame requested.

static anicode_engine *engine_of(mlt_producer producer)
{
    return mlt_properties_get_data(MLT_PRODUCER_PROPERTIES(producer), "_anicode", NULL);
}

static void log_error(mlt_producer producer, anicode_engine *engine)
{
    mlt_log_error(MLT_PRODUCER_SERVICE(producer), "%s\n", anicode_last_error(engine));
}

static int get_image(mlt_frame frame, uint8_t **buffer, mlt_image_format *format, int *width,
                     int *height, int writable)
{
    mlt_producer producer = mlt_frame_pop_service(frame);
    mlt_service service = MLT_PRODUCER_SERVICE(producer);
    mlt_profile profile = mlt_service_profile(service);
    if (*width <= 0)
        *width = profile->width;
    if (*height <= 0)
        *height = profile->height;

    int size = *width * *height * 4;
    uint8_t *rgba = mlt_pool_alloc(size);
    // The frame position counts from the clip's in-point, not from the scene start.
    mlt_position position = mlt_frame_original_position(frame) + mlt_producer_get_in(producer);
    double time = position / mlt_profile_fps(profile);

    // MLT may ask for frames from several threads; the engine is not thread safe.
    mlt_service_lock(service);
    anicode_engine *engine = engine_of(producer);
    if (anicode_reload_if_changed(engine) < 0)
        log_error(producer, engine);
    if (anicode_render(engine, time, *width, *height, rgba, *width * 4) < 0)
        log_error(producer, engine);
    mlt_service_unlock(service);

    mlt_frame_set_image(frame, rgba, size, mlt_pool_release);
    *buffer = rgba;
    *format = mlt_image_rgba;
    return 0;
}

static int get_frame(mlt_producer producer, mlt_frame_ptr frame, int index)
{
    *frame = mlt_frame_init(MLT_PRODUCER_SERVICE(producer));
    mlt_properties frame_properties = MLT_FRAME_PROPERTIES(*frame);
    mlt_properties_set_int(frame_properties, "progressive", 1);
    mlt_properties_set_double(frame_properties, "aspect_ratio",
                              mlt_profile_sar(mlt_service_profile(MLT_PRODUCER_SERVICE(producer))));

    mlt_frame_set_position(*frame, mlt_producer_position(producer));
    mlt_frame_push_service(*frame, producer);
    mlt_frame_push_get_image(*frame, get_image);
    mlt_producer_prepare_next(producer);
    return 0;
}

mlt_producer producer_anicode_init(mlt_profile profile, mlt_service_type type, const char *id, char *arg)
{
    if (arg == NULL)
        return NULL;
    anicode_engine *engine = anicode_open(arg);
    if (engine == NULL)
        return NULL;
    mlt_producer producer = mlt_producer_new(profile);
    if (producer == NULL) {
        anicode_close(engine);
        return NULL;
    }
    producer->get_frame = get_frame;

    mlt_properties properties = MLT_PRODUCER_PROPERTIES(producer);
    mlt_properties_set(properties, "resource", arg);
    mlt_properties_set_data(properties, "_anicode", engine, 0, (mlt_destructor) anicode_close, NULL);
    if (anicode_last_error(engine)[0] != '\0')
        log_error(producer, engine);

    // The last frame shows the end state, so the clip is one frame longer than the scene.
    double duration = anicode_duration(engine);
    if (duration > 0) {
        int length = (int) lround(duration * mlt_profile_fps(profile)) + 1;
        mlt_properties_set_position(properties, "length", length);
        mlt_properties_set_position(properties, "out", length - 1);
    }
    return producer;
}
