#!/usr/bin/env bash
# Regenerate waproto/src/whatsapp.desc from waproto/src/whatsapp.proto.
#
# Consumers of this crate never run this — they only need `cargo build`,
# which reads the committed `.desc` and writes Rust source to `OUT_DIR`.
# Editors of the `.proto` run this once per edit and commit both files.
#
# Requires `protoc` on PATH.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
proto="$repo_root/waproto/src/whatsapp.proto"
desc="$repo_root/waproto/src/whatsapp.desc"
hash="$repo_root/waproto/src/whatsapp.desc.sha256"
includes="$repo_root/waproto/src"

if ! command -v protoc >/dev/null 2>&1; then
  echo "error: protoc not on PATH; install protobuf-compiler" >&2
  exit 1
fi

protoc \
  --descriptor_set_out="$desc" \
  --include_imports \
  --include_source_info \
  -I"$includes" \
  "$proto"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

{
  printf 'proto %s\n' "$(hash_file "$proto")"
  printf 'desc %s\n' "$(hash_file "$desc")"
} > "$hash"

echo "regenerated: $desc"
echo "regenerated: $hash"
echo "commit waproto/src/whatsapp.proto, waproto/src/whatsapp.desc, and waproto/src/whatsapp.desc.sha256"
