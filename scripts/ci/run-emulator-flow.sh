#!/usr/bin/env bash
# The emulator step of the Android emulator flow (decision 0080, task A11). It installs the release APK, whose JavaScript
# is bundled (no Metro server), records the device log, makes a pairing offer the emulator can dial at 10.0.2.2, and runs
# the Maestro flow with it. The offer is masked in the job log and replaced in every file the job keeps.
set -euo pipefail

cd "$GITHUB_WORKSPACE"
export PATH="$ANDROID_HOME/platform-tools:$PATH"
readonly PORT=7347
readonly FLOW=mobile/.maestro/pairing-home-workspaces.yaml
readonly LOGCAT="$RUNNER_TEMP/logcat-full.txt"
readonly CRASHLOG="$RUNNER_TEMP/logcat-crash.txt"
readonly DEBUG_DIR="$RUNNER_TEMP/maestro-debug"
offer=""
logcat_pid=""

# Replaces the pairing offer in every kept file that is text. Screenshots are images and are skipped.
redact() {
  [ -n "$offer" ] || return 0
  python3 -I -c '
import pathlib, sys
secret = sys.argv[1].encode()
for name in sys.argv[2:]:
    root = pathlib.Path(name)
    files = [root] if root.is_file() else sorted(root.rglob("*")) if root.is_dir() else []
    for path in files:
        if not path.is_file() or path.suffix.lower() in (".png", ".jpg", ".jpeg"):
            continue
        data = path.read_bytes()
        if secret in data:
            path.write_bytes(data.replace(secret, b"<pairing offer redacted>"))
' "$offer" "$LOGCAT" "$CRASHLOG" "$DEBUG_DIR" || true
}

cleanup() {
  if [ -n "$logcat_pid" ]; then kill "$logcat_pid" 2>/dev/null || true; fi
  adb logcat -d -b crash >"$CRASHLOG" 2>/dev/null || true
  redact
}
trap cleanup EXIT

apk="$(find "$RUNNER_TEMP/apk" -name '*.apk' -print -quit)"
test -n "$apk"
adb wait-for-device
# The app's and React Native's messages, native crashes and the app's process lifecycle, from before the install on.
adb logcat -v threadtime AndroidRuntime:E ReactNative:V ReactNativeJS:V ReactHostImpl:V ReactInstance:V \
  SoLoader:V DEBUG:V ActivityManager:I System.err:W '*:S' >"$LOGCAT" 2>&1 &
logcat_pid=$!
adb install -r "$apk"

offer="$("$MASSORCA_BIN" --json pair offer --ttl 900 | jq -r '.result.offer')"
test -n "$offer"
offer="$(printf '%s' "$offer" | python3 scripts/ci/rewrite-offer-address.py --address "10.0.2.2:$PORT")"
test -n "$offer"
echo "::add-mask::$offer"

maestro test --debug-output "$DEBUG_DIR" -e "MASSORCA_PAIRING_OFFER=$offer" "$FLOW"
