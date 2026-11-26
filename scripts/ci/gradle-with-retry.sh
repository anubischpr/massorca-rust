#!/usr/bin/env bash
# Runs Gradle (./gradlew, or $GRADLE when set) with the given arguments, and up to twice more if a run failed only
# because a plugin or dependency could not be downloaded (the Gradle plugin portal and Maven hosts fail now and then;
# Robolectric's own android-all download while the tests run answers 429 when Maven Central throttles).
# Any other failure stands.
set -uo pipefail

gradle_cmd="${GRADLE:-./gradlew}"
download_failure="was not found in any of the following sources|Could not resolve|Could not GET|Could not HEAD|Read timed out|Connection reset|Connect timed out|status code (429|5[0-9][0-9])|HTTP response code: (429|5[0-9][0-9])"

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

"$gradle_cmd" "$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -eq 0 ]; then
  exit 0
fi
if ! grep -q -E "$download_failure" "$log"; then
  exit "$status"
fi
# Up to two more attempts, waiting longer each time: a throttled host needs time, not a quick second try.
for wait in 30 90; do
  echo "::warning::Gradle could not download a plugin or dependency; retrying in $wait s"
  sleep "$wait"
  "$gradle_cmd" "$@" 2>&1 | tee "$log"
  status=${PIPESTATUS[0]}
  if [ "$status" -eq 0 ]; then
    exit 0
  fi
  if ! grep -q -E "$download_failure" "$log"; then
    exit "$status"
  fi
done
exit "$status"
