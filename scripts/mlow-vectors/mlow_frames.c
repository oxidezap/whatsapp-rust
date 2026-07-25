// Encode a raw PCM file as MLow/smpl packets and decode each one back through the same
// reference, so a single run produces both halves of a decoder cross-check vector.
//
// Output, one line per packet, on stdout:
//
//     <hex payload> <hex s16le pcm>
//
// Diagnostics (packet index, byte count, TOC, sample count) go to stderr, so stdout can be
// redirected straight into a fixture builder.
//
// Two things are easy to get wrong and cost an afternoon each:
//
//   * smpl_CreateCodec() must run before the first encode. Without it every encode fails with
//     SMPL_ENC_NO_GLOBAL_DATA (-112), which opus_encode flattens into a bare OPUS_INTERNAL_ERROR
//     (-3) with no hint that global tables were the problem.
//   * The output buffer handed to opus_encode is a hard bound, not a capacity. Under CBR the
//     encoder pads to exactly that size, so an oversized buffer fails the pad. Keep it near the
//     real packet size.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "opus.h"

// Declared here rather than pulled from a private header: the reference exposes it as a plain
// symbol and the harness only needs the one entry point.
extern int smpl_CreateCodec(void);

#define FS 16000
#define MAX_PACKET 400

static void usage(const char *argv0) {
    fprintf(stderr,
            "usage: %s <input.raw> <frame_ms> [packets] [bitrate]\n"
            "  input.raw  s16le mono @ 16 kHz\n"
            "  frame_ms   10, 20, 60 or 120\n"
            "  packets    how many to emit (default: until EOF)\n"
            "  bitrate    encoder bitrate in bps (default 20000)\n",
            argv0);
}

int main(int argc, char **argv) {
    if (argc < 3) {
        usage(argv[0]);
        return 2;
    }
    const char *path = argv[1];
    const int frame_ms = atoi(argv[2]);
    const int want = argc > 3 ? atoi(argv[3]) : -1;
    const int bitrate = argc > 4 ? atoi(argv[4]) : 20000;

    if (frame_ms != 10 && frame_ms != 20 && frame_ms != 60 && frame_ms != 120) {
        fprintf(stderr, "error: frame_ms must be 10, 20, 60 or 120 (got %d)\n", frame_ms);
        return 2;
    }
    const int samps = FS / 1000 * frame_ms;

    if (smpl_CreateCodec() != 0) {
        fprintf(stderr, "error: smpl_CreateCodec failed\n");
        return 1;
    }

    FILE *f = fopen(path, "rb");
    if (!f) {
        perror("open input");
        return 2;
    }

    int err = 0;
    OpusEncoder *enc = opus_encoder_create(FS, 1, OPUS_APPLICATION_VOIP, &err);
    if (err != OPUS_OK || !enc) {
        fprintf(stderr, "error: encoder create: %s\n", opus_strerror(err));
        return 1;
    }
    opus_encoder_ctl(enc, OPUS_SET_USING_SMPL(1));
    opus_encoder_ctl(enc, OPUS_SET_BITRATE(bitrate));
    opus_encoder_ctl(enc, OPUS_SET_FORCE_CHANNELS(1));

    OpusDecoder *dec = opus_decoder_create(FS, 1, &err);
    if (err != OPUS_OK || !dec) {
        fprintf(stderr, "error: decoder create: %s\n", opus_strerror(err));
        return 1;
    }
    opus_decoder_ctl(dec, OPUS_SET_USING_SMPL(1));

    opus_int16 *pcm = malloc((size_t)samps * sizeof(opus_int16));
    opus_int16 *out = malloc((size_t)samps * sizeof(opus_int16));
    unsigned char packet[MAX_PACKET];
    if (!pcm || !out) {
        fprintf(stderr, "error: out of memory\n");
        return 1;
    }

    int emitted = 0;
    for (int n = 0; want < 0 || n < want; n++) {
        if (fread(pcm, sizeof(opus_int16), (size_t)samps, f) != (size_t)samps) {
            break;
        }
        opus_int32 len = opus_encode(enc, pcm, samps, packet, sizeof(packet));
        if (len <= 0) {
            fprintf(stderr, "error: encode packet %d: %s\n", n, opus_strerror(len));
            return 1;
        }
        int ns = opus_decode(dec, packet, len, out, samps, 0);
        if (ns <= 0) {
            fprintf(stderr, "error: decode packet %d: %s\n", n, opus_strerror(ns));
            return 1;
        }
        for (opus_int32 i = 0; i < len; i++) {
            printf("%02x", packet[i]);
        }
        printf(" ");
        for (int i = 0; i < ns; i++) {
            unsigned short v = (unsigned short)out[i];
            printf("%02x%02x", v & 0xff, (v >> 8) & 0xff);
        }
        printf("\n");
        fprintf(stderr, "packet %d: %d bytes, TOC 0x%02x, %d samples\n", n, len, packet[0], ns);
        emitted++;
    }

    fprintf(stderr, "emitted %d packet(s) of %d ms\n", emitted, frame_ms);
    free(pcm);
    free(out);
    opus_encoder_destroy(enc);
    opus_decoder_destroy(dec);
    fclose(f);
    return emitted > 0 ? 0 : 1;
}
