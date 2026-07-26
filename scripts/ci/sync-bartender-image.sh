#!/usr/bin/env bash
set -euo pipefail

# GitHub resolves service-container images before checkout, so a workflow
# cannot read the canonical image file directly. Keep the required literals
# generated from one commit-scoped source and reject drift in CI.
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
source_file="$repo_root/.github/bartender-image.txt"
workflow_dir="$repo_root/.github/workflows"
mode="${1:---check}"
pin_pattern="^[[:space:]]*image:[[:space:]]+['\"]?ghcr\\.io/whiskeysockets-devtools/bartender[^[:space:]'\"]*['\"]?[[:space:]]*$"
workflow_files=(
    "$workflow_dir/codspeed.yml"
    "$workflow_dir/copilot-setup-steps.yml"
    "$workflow_dir/e2e.yml"
)

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

invalid_targets=false
for workflow_file in "${workflow_files[@]}"; do
    if [[ ! -f "$workflow_file" ]]; then
        echo "Missing Bartender workflow target: $workflow_file" >&2
        invalid_targets=true
        continue
    fi

    mapfile -t target_pins < <(grep -n -E "$pin_pattern" "$workflow_file")
    if [[ ${#target_pins[@]} -ne 1 ]]; then
        printf '%s: expected exactly one Bartender image reference; found %s\n' \
            "$workflow_file" "${#target_pins[@]}" >&2
        invalid_targets=true
    fi
done

if [[ "$invalid_targets" == true ]]; then
    exit 1
fi

if [[ "$mode" == "--write" ]]; then
    for workflow_file in "${workflow_files[@]}"; do
        sed -i -E \
            "s#^([[:space:]]*image:[[:space:]]+)([\"']?)ghcr\\.io/whiskeysockets-devtools/bartender[^[:space:]\"']*([\"']?)([[:space:]]*)\$#\\1\\2$image\\2\\4#" \
            "$workflow_file"
    done
fi

pin_count=0
drift_found=false
for workflow_file in "${workflow_files[@]}"; do
    while IFS=: read -r line_number workflow_line; do
        pin_count=$((pin_count + 1))
        actual_image="${workflow_line#*:}"
        actual_image="${actual_image#"${actual_image%%[![:space:]]*}"}"
        actual_image="${actual_image%"${actual_image##*[![:space:]]}"}"
        first_character="${actual_image:0:1}"
        last_character="${actual_image: -1}"
        if [[ "$first_character" == "$last_character" &&
            ("$first_character" == '"' || "$first_character" == "'") ]]; then
            actual_image="${actual_image:1:${#actual_image}-2}"
        fi
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
