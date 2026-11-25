#!/usr/bin/env bash
# The emulator step of the Android emulator flow (decision 0080, task A11): installs the debug APK, makes a pairing offer
# the emulator can dial at 10.0.2.2, and runs the Maestro flow with it. The offer is masked and never printed.
set -euo pipefail

cd "$GITHUB_WORKSPACE"
export PATH="$ANDROID_HOME/platform-tools:$PATH"
readonly PORT=7347
readonly FLOW=mobile/.maestro/pairing-home-workspaces.yaml
logcat="$RUNNER_TEMP/logcat.txt"
trap 'adb logcat -d -s ReactNativeJS:V AndroidRuntime:E >"$logcat" 2>/dev/null || true' ERR

apk="$(find "$RUNNER_TEMP/apk" -name '*.apk' -print -quit)"
test -n "$apk"
adb wait-for-device
adb install -r "$apk"
# The debug build loads its JavaScript from the Metro server the job started on the runner.
adb reverse tcp:8081 tcp:8081

offer="$("$MASSORCA_BIN" --json pair offer --ttl 900 | jq -r '.result.offer')"
test -n "$offer"
offer="$(printf '%s' "$offer" | python3 scripts/ci/rewrite-offer-address.py --address "10.0.2.2:$PORT")"
test -n "$offer"
echo "::add-mask::$offer"

maestro test -e "MASSORCA_PAIRING_OFFER=$offer" "$FLOW"
