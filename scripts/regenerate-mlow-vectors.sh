#!/usr/bin/env bash
# Regenerate the MLow decoder cross-check fixtures from the `smpl` C reference.
#
# Consumers never run this — the fixtures are committed and the tests read them directly. It
# exists so the vectors stop being a documented recipe with a missing binary: everything needed
# to reproduce them, including the two non-obvious setup steps, lives in
# `scripts/mlow-vectors/mlow_frames.c`.
#
# The committed fixtures were produced with this exact oracle:
#
#   https://github.com/edgardmessias/opus_mlow at 84b076e0809412df22e8a0d26f944610c4a3e40f
#
# Byte-for-byte reproduction is only claimed against that revision. A different checkout can
# change the output because the ORACLE changed, which is not the same thing as a decoder change,
# so the script prints the revision it actually built against and warns when it differs.
#
# Requires a built `smpl` reference (the WhatsApp MLow fork of libopus):
#
#   cd "$MLOW_REFERENCE" && ./autogen.sh && ./configure --disable-shared --disable-doc \
#     --disable-extra-programs && make -j"$(nproc)"
#
# Then:
#
#   MLOW_REFERENCE=/path/to/opus_mlow scripts/regenerate-mlow-vectors.sh
#
# The script refuses to run against a reference worktree with uncommitted changes, or against a
# library older than the checkout, because neither can be attributed to a recorded revision. Set
# MLOW_ALLOW_DIRTY_REFERENCE=1 when generating from a modified oracle is the actual intent.
#
# Regenerating changes committed fixtures. Re-run the decoder suite afterwards and treat any
# correlation change as a finding, not as a number to paper over:
#
#   cargo test -p wacore --features voip-mlow --lib

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
testdata="$repo_root/wacore/src/voip/mlow/testdata"
harness_src="$repo_root/scripts/mlow-vectors/mlow_frames.c"

ref="${MLOW_REFERENCE:-}"
if [[ -z "$ref" ]]; then
  echo "error: set MLOW_REFERENCE to the built smpl/opus reference checkout" >&2
  exit 1
fi
if [[ ! -f "$ref/.libs/libopus.a" ]]; then
  echo "error: $ref/.libs/libopus.a not found; build the reference first (see the header of this script)" >&2
  exit 1
fi

# Recorded so a regeneration says which oracle produced it; see the header.
expected_rev="84b076e0809412df22e8a0d26f944610c4a3e40f"
actual_rev="$(git -C "$ref" rev-parse HEAD 2>/dev/null || echo unknown)"
if [[ "$actual_rev" == "unknown" ]]; then
  echo "warning: $ref is not a git checkout; cannot confirm the oracle revision" >&2
else
  # The revision alone does not identify the build: uncommitted changes still compile into the
  # static library while rev-parse keeps reporting the pinned commit, so a modified oracle would
  # silently rewrite the fixtures and look like a legitimate update. Refuse unless told otherwise.
  if [[ -n "$(git -C "$ref" status --porcelain 2>/dev/null)" ]]; then
    if [[ "${MLOW_ALLOW_DIRTY_REFERENCE:-}" == "1" ]]; then
      echo "warning: oracle worktree is DIRTY; output does not correspond to $actual_rev" >&2
    else
      echo "error: oracle worktree has uncommitted changes, so the fixtures it produces would not" >&2
      echo "       correspond to any recorded revision. Commit or stash them, or re-run with" >&2
      echo "       MLOW_ALLOW_DIRTY_REFERENCE=1 if you mean to generate from a modified oracle." >&2
      exit 1
    fi
  fi
  if [[ "$actual_rev" != "$expected_rev" ]]; then
    echo "warning: oracle is $actual_rev, fixtures were generated with $expected_rev" >&2
    echo "         differences below may come from the reference, not from this repository" >&2
  fi
fi
# A clean worktree at the right commit still proves nothing about the ARCHIVE: switching revisions
# without rebuilding leaves a stale .a that links fine and attributes its output to a commit it was
# never built from. Compare against the last thing that changed the checkout.
lib="$ref/.libs/libopus.a"
# Compare against the SOURCES rather than any git metadata file: `git reset --hard` on the same
# branch moves the branch ref while leaving .git/HEAD untouched, so a HEAD mtime check would accept
# an archive built from a different revision. A source newer than the archive is the direct
# statement that the archive does not correspond to the tree it would be attributed to.
# Check the layout first. `find` on a missing directory exits nonzero, and with `&&` below that
# would skip the whole staleness check in silence -- reporting nothing on exactly the day the oracle
# moved, which is the day this guard exists for.
for dir in "$ref/smpl" "$ref/src" "$ref/celt"; do
  if [[ ! -d "$dir" ]]; then
    echo "error: $dir is missing, so the archive cannot be checked against its sources." >&2
    echo "       The oracle layout changed; update this script before regenerating." >&2
    exit 1
  fi
done
if newer="$(find "$ref/smpl" "$ref/src" "$ref/celt" \
             \( -name '*.c' -o -name '*.h' \) -newer "$lib" -print -quit)" \
   && [[ -n "$newer" ]]; then
  echo "error: $lib is older than $newer, so it was not built from the current tree." >&2
  echo "       Rebuild it (make -j\"\$(nproc)\" in \$MLOW_REFERENCE) and re-run." >&2
  exit 1
fi

echo "==> oracle: $ref @ $actual_rev"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "==> building the harness against $ref"
cc -O2 \
  -I "$ref/include" -I "$ref/src" -I "$ref/celt" -I "$ref" \
  -o "$work/mlow_frames" "$harness_src" "$ref/.libs/libopus.a" -lm

# One packet per line: "<hex payload> <hex s16le pcm>". Split into the frames fixture and the
# reference PCM the decoder test compares against.
emit() {
  local frame_ms="$1" packets="$2" frames_json="$3" pcm_raw="$4"
  echo "==> ${frame_ms}ms x ${packets} -> $(basename "$frames_json"), $(basename "$pcm_raw")"
  "$work/mlow_frames" "$testdata/synth_mic.raw" "$frame_ms" "$packets" > "$work/vectors.txt"
  python3 - "$work/vectors.txt" "$frames_json" "$pcm_raw" <<'PY'
import json, sys
src, frames_path, pcm_path = sys.argv[1:4]
frames, pcm = [], bytearray()
for line in open(src):
    payload, samples = line.split()
    frames.append(payload)
    pcm += bytes.fromhex(samples)
json.dump(frames, open(frames_path, "w"), indent=1)
open(pcm_path, "wb").write(pcm)
print(f"   {len(frames)} frames, {len(pcm)//2} samples, TOCs: {sorted({f[:2] for f in frames})}")
PY
}

emit 120 8 "$testdata/mlow_120ms_frames.json" "$testdata/ref_120ms_expected.raw"
# 60 ms with DTX off: the encoder emits VoA=00 frames (TOC 0x10) for the silent stretches of
# synth_mic.raw, which is the only in-repo coverage of the DTX-off decode path.
emit 60 110 "$testdata/mlow_dtx_off_frames.json" "$testdata/ref_dtx_off_expected.raw"

echo "==> done; re-run: cargo test -p wacore --features voip-mlow --lib"
