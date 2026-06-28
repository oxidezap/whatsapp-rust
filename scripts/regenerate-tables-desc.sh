#!/usr/bin/env bash
# Regenerate wacore/src/voip/mlow/tables.desc from tables.proto.
#
# Consumers never run this — they only need `cargo build` (with the `voip`
# feature), which reads the committed `.desc` and writes Rust source to
# `OUT_DIR`. Editors of the `.proto` run this once per edit and commit both
# files.
#
# Requires `protoc` on PATH.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
proto_dir="$repo_root/wacore/src/voip/mlow"

if ! command -v protoc >/dev/null 2>&1; then
  echo "error: protoc not on PATH; install protobuf-compiler" >&2
  exit 1
fi

protoc \
  --descriptor_set_out="$proto_dir/tables.desc" \
  --include_imports \
  -I"$proto_dir" \
  "$proto_dir/tables.proto"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

{
  printf 'proto %s\n' "$(hash_file "$proto_dir/tables.proto")"
  printf 'desc %s\n' "$(hash_file "$proto_dir/tables.desc")"
} > "$proto_dir/tables.desc.sha256"

echo "regenerated: $proto_dir/tables.desc"
echo "regenerated: $proto_dir/tables.desc.sha256"
echo "commit wacore/src/voip/mlow/tables.proto, tables.desc, and tables.desc.sha256"
