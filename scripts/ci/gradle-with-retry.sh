#!/usr/bin/env bash
# Runs Gradle (./gradlew, or $GRADLE when set) with the given arguments, and once more if the first run failed only
# because a plugin or dependency could not be downloaded (the Gradle plugin portal and Maven hosts fail now and then).
# Any other failure stands.
set -uo pipefail

gradle_cmd="${GRADLE:-./gradlew}"

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

"$gradle_cmd" "$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -eq 0 ]; then
  exit 0
fi
if ! grep -q -E "was not found in any of the following sources|Could not resolve|Could not GET|Could not HEAD|Read timed out|Connection reset|Connect timed out" "$log"; then
  exit "$status"
fi
echo "::warning::Gradle could not download a plugin or dependency; retrying once in 30 s"
sleep 30
"$gradle_cmd" "$@"
