#include "ass.h"
#include <errno.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define WIDTH 1280
#define HEIGHT 360

typedef struct {
    ASS_Library *library;
    ASS_Renderer *renderer;
    ASS_Track *track;
    const char *name;
    FILE *log;
    int serious_errors;
} Context;

static void message(int level, const char *fmt, va_list args, void *data) {
    Context *ctx = data;
    fprintf(ctx->log, "[%d] ", level);
    vfprintf(ctx->log, fmt, args);
    fputc('\n', ctx->log);
    if (level <= 1) ctx->serious_errors++;
}

static void die(const char *msg) { fprintf(stderr, "%s\n", msg); exit(1); }

static char *read_bytes(const char *path, long *size) {
    FILE *f = fopen(path, "rb");
    if (!f) die(path);
    if (fseek(f, 0, SEEK_END) != 0) die("seek failed");
    *size = ftell(f);
    if (*size <= 0 || *size > 1000000000) die("invalid file size");
    rewind(f);
    char *data = malloc((size_t)*size);
    if (!data || fread(data, 1, (size_t)*size, f) != (size_t)*size) die("read failed");
    fclose(f);
    return data;
}

static void init(Context *ctx, const char *dir, const char *file, const char *ttc, const char *otf) {
    char path[4096];
    snprintf(path, sizeof(path), "%s/%s.libass.log", dir, ctx->name);
    ctx->log = fopen(path, "wb");
    if (!ctx->log) die("cannot create log");
    ctx->library = ass_library_init();
    if (!ctx->library) die("ass_library_init failed");
    ass_set_message_cb(ctx->library, message, ctx);
    ass_set_extract_fonts(ctx->library, 1);
    if (ttc && otf) {
        long size;
        char *data = read_bytes(ttc, &size);
        ass_add_font(ctx->library, "original.ttc", data, (int)size);
        free(data);
        data = read_bytes(otf, &size);
        ass_add_font(ctx->library, "original.otf", data, (int)size);
        free(data);
    }
    snprintf(path, sizeof(path), "%s/%s", dir, file);
    ctx->track = ass_read_file(ctx->library, path, "UTF-8");
    if (!ctx->track) die("ass_read_file failed");
    ctx->renderer = ass_renderer_init(ctx->library);
    if (!ctx->renderer) die("ass_renderer_init failed");
    ass_set_frame_size(ctx->renderer, WIDTH, HEIGHT);
    ass_set_storage_size(ctx->renderer, WIDTH, HEIGHT);
    ass_set_pixel_aspect(ctx->renderer, 1.0);
    ass_set_fonts(ctx->renderer, NULL, NULL, ASS_FONTPROVIDER_NONE, NULL, 0);
}

static unsigned char *render(Context *ctx, long long time, const char *dir, size_t *visible) {
    unsigned char *pixels = calloc(WIDTH * HEIGHT * 3, 1);
    if (!pixels) die("out of memory");
    int change;
    ASS_Image *head = ass_render_frame(ctx->renderer, ctx->track, time, &change);
    for (ASS_Image *im = head; im; im = im->next) {
        int rgb[3] = {(im->color >> 24) & 255, (im->color >> 16) & 255, (im->color >> 8) & 255};
        int opacity = 255 - (im->color & 255);
        for (int y = 0; y < im->h; y++) for (int x = 0; x < im->w; x++) {
            int px = im->dst_x + x, py = im->dst_y + y;
            if (px < 0 || px >= WIDTH || py < 0 || py >= HEIGHT) die("image outside frame");
            int alpha = (im->bitmap[y * im->stride + x] * opacity + 127) / 255;
            size_t offset = ((size_t)py * WIDTH + px) * 3;
            for (int c = 0; c < 3; c++) pixels[offset + c] = (rgb[c] * alpha + pixels[offset + c] * (255 - alpha) + 127) / 255;
        }
    }
    *visible = 0;
    for (size_t i = 0; i < WIDTH * HEIGHT; i++) if (pixels[i*3] || pixels[i*3+1] || pixels[i*3+2]) (*visible)++;
    char path[4096];
    snprintf(path, sizeof(path), "%s/%s-%lld.ppm", dir, ctx->name, time);
    FILE *out = fopen(path, "wb");
    if (!out) die("cannot write frame");
    fprintf(out, "P6\n%d %d\n255\n", WIDTH, HEIGHT);
    if (fwrite(pixels, 3, WIDTH * HEIGHT, out) != WIDTH * HEIGHT) die("frame write failed");
    fclose(out);
    return pixels;
}

static size_t difference(const unsigned char *a, const unsigned char *b) {
    size_t result = 0;
    for (size_t i = 0; i < WIDTH * HEIGHT; i++) if (memcmp(a + 3*i, b + 3*i, 3) != 0) result++;
    return result;
}

int main(int argc, char **argv) {
    if (argc != 4) die("usage: verify_libass ARTIFACT_DIR ORIGINAL_TTC ORIGINAL_OTF");
    Context ctx[5] = {{.name="baseline"}, {.name="embedded"}, {.name="no-fonts"}, {.name="ttc-only"}, {.name="otf-only"}};
    const char *files[] = {"input.ass", "input.assfonts.ass", "input.ass", "ttc-only.ass", "otf-only.ass"};
    for (int i = 0; i < 5; i++) init(&ctx[i], argv[1], files[i], i == 0 ? argv[2] : NULL, i == 0 ? argv[3] : NULL);
    int ok = 1;
    printf("{\"libass_version\":%d,\"system_font_provider\":\"NONE\",\"frames\":[", ass_library_version());
    for (int frame = 0; frame < 4; frame++) {
        long long time = frame * 1000 + 500;
        size_t visible[5], diff[5] = {0};
        unsigned char *pixels[5];
        for (int i = 0; i < 5; i++) pixels[i] = render(&ctx[i], time, argv[1], &visible[i]);
        for (int i = 1; i < 5; i++) diff[i] = difference(pixels[0], pixels[i]);
        if (!visible[0] || diff[1] || visible[2]) ok = 0;
        if (frame == 0 && !diff[4]) ok = 0;
        if (frame == 1 && !diff[3]) ok = 0;
        printf("%s{\"time_ms\":%lld,\"baseline_visible_pixels\":%zu,\"embedded_visible_pixels\":%zu,\"embedded_different_pixels\":%zu,\"no_fonts_visible_pixels\":%zu,\"ttc_only_different_pixels\":%zu,\"otf_only_different_pixels\":%zu}", frame ? "," : "", time, visible[0], visible[1], diff[1], visible[2], diff[3], diff[4]);
        for (int i = 0; i < 5; i++) free(pixels[i]);
    }
    if (ctx[0].serious_errors || ctx[1].serious_errors) ok = 0;
    printf("],\"baseline_errors\":%d,\"embedded_errors\":%d,\"passed\":%s}\n", ctx[0].serious_errors, ctx[1].serious_errors, ok ? "true" : "false");
    for (int i = 0; i < 5; i++) { ass_free_track(ctx[i].track); ass_renderer_done(ctx[i].renderer); ass_library_done(ctx[i].library); fclose(ctx[i].log); }
    return ok ? 0 : 1;
}
