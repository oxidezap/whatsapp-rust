#!/usr/bin/env bash
# Runs `cargo nextest run` and files its JUnit report under a name of its own.
#
# nextest writes one report per profile, so successive runs in a job overwrite
# it; this moves each result to $TEST_TIMINGS_DIR/<label>.xml. Two things the
# obvious inline version gets wrong. A failing test still produces a report, and
# that is exactly when its timings are worth keeping, so the status is returned
# for the caller to decide rather than ending the step. A failing *build*
# produces none, so a report left in the restored target dir is cleared first and
# the move is conditional, or a stale one would be published as this run's.
set -uo pipefail

label="${1:?usage: nextest_timed.sh <label> <cargo nextest args...>}"
shift
dir="${TEST_TIMINGS_DIR:?TEST_TIMINGS_DIR must be set}"
profile="${NEXTEST_PROFILE:?NEXTEST_PROFILE must be set}"
report="target/nextest/$profile/junit.xml"

mkdir -p "$dir"
rm -f "$report"

status=0
cargo nextest run "$@" || status=$?

if [ -f "$report" ]; then
  mv "$report" "$dir/$label.xml"
fi
exit "$status"
