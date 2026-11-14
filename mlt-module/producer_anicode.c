#include <framework/mlt.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/stat.h>

// M0 spike: draws a test pattern (color bars with a white bar that moves
// one column per frame) so we can see that Shotcut loads the module and
// requests new frames. The bars shift each time the scene file is saved,
// to test whether Shotcut picks up file changes.

static const uint8_t BAR_COLORS[][3] = {
    {255, 255, 255}, {255, 255, 0}, {0, 255, 255}, {0, 255, 0},
    {255, 0, 255},   {255, 0, 0},   {0, 0, 255},
};
static const int BAR_COUNT = sizeof(BAR_COLORS) / sizeof(BAR_COLORS[0]);

// Changes whenever the file's modification time changes.
static int bar_shift_for_file(const char *path)
{
    struct stat info;
    if (path == NULL || stat(path, &info) != 0)
        return 0;
    return info.st_mtime % BAR_COUNT;
}

static void draw_test_pattern(uint8_t *rgba, int width, int height, mlt_position position,
                              int bar_shift)
{
    int moving_column = position % width;
    for (int y = 0; y < height; y++) {
        for (int x = 0; x < width; x++) {
            uint8_t *pixel = rgba + (y * width + x) * 4;
            const uint8_t *color = BAR_COLORS[(x * BAR_COUNT / width + bar_shift) % BAR_COUNT];
            int is_moving_bar = abs(x - moving_column) < 8;
            pixel[0] = is_moving_bar ? 255 : color[0] / 2;
            pixel[1] = is_moving_bar ? 255 : color[1] / 2;
            pixel[2] = is_moving_bar ? 255 : color[2] / 2;
            pixel[3] = 255;
        }
    }
}

static int get_image(mlt_frame frame, uint8_t **buffer, mlt_image_format *format, int *width,
                     int *height, int writable)
{
    mlt_producer producer = mlt_frame_pop_service(frame);
    mlt_profile profile = mlt_service_profile(MLT_PRODUCER_SERVICE(producer));
    if (*width <= 0)
        *width = profile->width;
    if (*height <= 0)
        *height = profile->height;

    int size = *width * *height * 4;
    uint8_t *rgba = mlt_pool_alloc(size);
    const char *scene_path = mlt_properties_get(MLT_PRODUCER_PROPERTIES(producer), "resource");
    draw_test_pattern(rgba, *width, *height, mlt_frame_original_position(frame),
                      bar_shift_for_file(scene_path));

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
    mlt_producer producer = mlt_producer_new(profile);
    if (producer == NULL)
        return NULL;
    producer->get_frame = get_frame;
    mlt_properties_set(MLT_PRODUCER_PROPERTIES(producer), "resource", arg);
    return producer;
}
