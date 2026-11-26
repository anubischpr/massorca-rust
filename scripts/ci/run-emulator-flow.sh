#!/usr/bin/env bash
# The emulator step of the Android emulator flow (decision 0080, task A11). It installs the release APK, whose JavaScript
# is bundled (no Metro server), records the device log, makes a pairing offer the emulator can dial at 10.0.2.2, and runs
# the Maestro flow with it. The offer is masked in the job log and replaced in every file the job keeps. A failed run
# also keeps the terminal's screen as text: the server's view of the shell the flow typed into.
set -euo pipefail

cd "$GITHUB_WORKSPACE"
export PATH="$ANDROID_HOME/platform-tools:$PATH"
readonly PORT=7347
readonly FLOW=mobile/.maestro/pairing-home-workspaces.yaml
readonly LOGCAT="$RUNNER_TEMP/logcat-full.txt"
readonly CRASHLOG="$RUNNER_TEMP/logcat-crash.txt"
readonly SCREENLOG="$RUNNER_TEMP/terminal-screen.txt"
readonly SCREENTEXT="$RUNNER_TEMP/phone-screen-text.txt"
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
' "$offer" "$LOGCAT" "$CRASHLOG" "$SCREENLOG" "$SCREENTEXT" "$DEBUG_DIR" || true
}

# The phone's text and labels as its accessibility tree has them, the tree the flow's assertions read. Text that starts like
# the pairing offer or runs long is left out, so the offer is never printed or kept.
phone_text() {
  adb shell uiautomator dump /sdcard/phone-window.xml >/dev/null 2>&1 || return 0
  adb pull /sdcard/phone-window.xml "$RUNNER_TEMP/phone-window.xml" >/dev/null 2>&1 || return 0
  adb shell rm -f /sdcard/phone-window.xml >/dev/null 2>&1 || true
  python3 -I - "$RUNNER_TEMP/phone-window.xml" >"$SCREENTEXT" <<'PY' || true
import html, re, sys

xml = open(sys.argv[1], encoding="utf-8", errors="replace").read()
for node in re.findall(r"<node [^>]*>", xml):
    attrs = {key: html.unescape(value) for key, value in re.findall(r'([\w-]+)="([^"]*)"', node)}
    if attrs.get("index") == "0":
        print("screen bounds=%s" % attrs.get("bounds"))
    if "WebView" in attrs.get("class", ""):
        print("webview bounds=%s enabled=%s" % (attrs.get("bounds"), attrs.get("enabled")))
    if attrs.get("resource-id"):
        print("id %s enabled=%s bounds=%s" % (attrs["resource-id"], attrs.get("enabled"), attrs.get("bounds")))
    for key in ("text", "content-desc"):
        value = attrs.get(key, "")
        if value.strip() and not value.startswith("mop1.") and len(value) <= 200:
            print("%s: %s" % (key, value))
PY
  rm -f "$RUNNER_TEMP/phone-window.xml"
}

cleanup() {
  local status=$?
  if [ -n "$logcat_pid" ]; then kill "$logcat_pid" 2>/dev/null || true; fi
  adb logcat -d -b crash >"$CRASHLOG" 2>/dev/null || true
  # Only a failed run keeps the screens: the server's text of the terminal, and the phone's text as its tree has it.
  if [ "$status" -ne 0 ]; then
    if [ -n "${MASSORCA_E2E_TERMINAL:-}" ]; then
      "$MASSORCA_BIN" --json terminal screen "$MASSORCA_E2E_TERMINAL" 2>/dev/null |
        jq -r '.result.lines[]' >"$SCREENLOG" 2>/dev/null || true
    fi
    phone_text
    echo "Phone text at the failure (filtered; the pairing offer is left out):"
    head -n 120 "$SCREENTEXT" 2>/dev/null || true
  fi
  redact
}
trap cleanup EXIT

apk="$(find "$RUNNER_TEMP/apk" -name '*.apk' -print -quit)"
test -n "$apk"
adb wait-for-device
# The app's and React Native's messages, the WebView's console, native crashes and the app's process lifecycle,
# from before the install on.
adb logcat -v threadtime AndroidRuntime:E ReactNative:V ReactNativeJS:V ReactHostImpl:V ReactInstance:V \
  SoLoader:V DEBUG:V ActivityManager:I System.err:W chromium:I '*:S' >"$LOGCAT" 2>&1 &
logcat_pid=$!
adb install -r "$apk"

offer="$("$MASSORCA_BIN" --json pair offer --ttl 900 --address "10.0.2.2:$PORT" | jq -r '.result.offer')"
test -n "$offer"
echo "::add-mask::$offer"

maestro test --debug-output "$DEBUG_DIR" -e "MASSORCA_PAIRING_OFFER=$offer" "$FLOW"
