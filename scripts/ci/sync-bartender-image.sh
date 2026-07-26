#!/usr/bin/env bash
set -euo pipefail

# GitHub resolves service-container images before checkout, so a workflow
# cannot read the canonical image file directly. Keep the required literals
# generated from one commit-scoped source and reject drift in CI.
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
source_file="$repo_root/.github/bartender-image.txt"
workflow_dir="$repo_root/.github/workflows"
mode="${1:---check}"
pin_pattern='^[[:space:]]*image:[[:space:]]+ghcr\.io/whiskeysockets-devtools/bartender[^[:space:]]*'

if [[ $# -gt 1 || ("$mode" != "--check" && "$mode" != "--write") ]]; then
    echo "Usage: $0 [--check|--write]" >&2
    exit 2
fi

if [[ ! -f "$source_file" ]]; then
    echo "Missing canonical Bartender image file: $source_file" >&2
    exit 1
fi

image="$(<"$source_file")"
if [[ ! "$image" =~ ^ghcr\.io/whiskeysockets-devtools/bartender@sha256:[0-9a-f]{64}$ ]]; then
    echo "Invalid canonical Bartender image: $image" >&2
    exit 1
fi

workflow_files=()
while IFS= read -r -d '' workflow_file; do
    if grep -q -E "$pin_pattern" "$workflow_file"; then
        workflow_files+=("$workflow_file")
    fi
done < <(
    find "$workflow_dir" -type f \( -name '*.yml' -o -name '*.yaml' \) -print0 |
        sort -z
)

if [[ ${#workflow_files[@]} -eq 0 ]]; then
    echo "No Bartender workflow image references found" >&2
    exit 1
fi

if [[ "$mode" == "--write" ]]; then
    for workflow_file in "${workflow_files[@]}"; do
        sed -i -E \
            "s#^([[:space:]]*image:[[:space:]]+)ghcr\\.io/whiskeysockets-devtools/bartender[^[:space:]]*#\\1$image#" \
            "$workflow_file"
    done
fi

pin_count=0
drift_found=false
for workflow_file in "${workflow_files[@]}"; do
    while IFS=: read -r line_number workflow_line; do
        pin_count=$((pin_count + 1))
        actual_image="$(awk '{print $2}' <<<"$workflow_line")"
        if [[ "$actual_image" != "$image" ]]; then
            printf '%s:%s: Bartender image is %s; expected %s\n' \
                "$workflow_file" "$line_number" "$actual_image" "$image" >&2
            drift_found=true
        fi
    done < <(grep -n -E "$pin_pattern" "$workflow_file")
done

if [[ "$drift_found" == true ]]; then
    echo "Run scripts/ci/sync-bartender-image.sh --write to synchronize the workflows." >&2
    exit 1
fi

echo "Bartender image pins are synchronized ($pin_count references)."
