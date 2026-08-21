#!/usr/bin/env bash
# Prints the crates the feature matrix walks, as a JSON array for a GitHub
# Actions matrix.
#
# Derived from cargo metadata rather than listed in the workflow, for the same
# reason test_features.sh derives features: a hand-written list is a second
# source of truth that nothing checks, and the failure it produces is a crate
# quietly dropping out of the only job that exercises its features one at a
# time. Adding one to the workspace is enough.
#
# The rule is "publishable", not "has features": a crate whose `[features]` table
# is empty still gets checked once here with default features off, which is a
# combination the other jobs never build, and one that grows a feature tomorrow
# needs no edit either way.
set -euo pipefail

cargo metadata --no-deps --format-version 1 \
  | jq -c '[.packages[] | select(.publish != []) | .name] | sort'
