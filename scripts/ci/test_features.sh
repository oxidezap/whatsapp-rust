#!/usr/bin/env bash
# Prints the features of a package that the test suite runs with all at once,
# comma-separated for `cargo nextest --features`.
#
# Derived from cargo metadata so a feature added tomorrow is exercised without
# anyone editing CI. Only a feature that cannot share a build with the rest
# needs a line below, and each says why it cannot.
set -euo pipefail

package="${1:?usage: test_features.sh <package>}"

# danger-skip-*  disable security verification, so the suite would change
#                behavior rather than catch rot; the cert-chain negative test is
#                itself cfg'd out under them.
# dhat-heap      installs a global allocator, colliding with the counting one
#                the allocation guards install.
# js, getrandom  wasm32 backend selection, with no native build to join.
excluded='default|danger-skip-.*|dhat-heap|js|getrandom'

cargo metadata --no-deps --format-version 1 \
  | jq -r --arg package "$package" \
      '.packages[] | select(.name == $package) | .features | keys[]' \
  | grep -vxE "$excluded" \
  | paste -sd,
