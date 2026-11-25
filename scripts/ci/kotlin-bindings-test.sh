#!/usr/bin/env bash
# The generated Kotlin bindings on the JVM against a real server (decision 0080, task A6): builds the Rust library and
# the massorca program, generates the Kotlin API in library mode, starts a service on loopback, and runs the Gradle
# tests of mobile/android/bindings-test. Needs `uniffi-bindgen` (the version in Cargo.lock) and Gradle on PATH.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

cargo build --locked -p massorca-cli -p massorca-client-ffi
target="$(cargo metadata --locked --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
lib_dir="$target/debug"
test -f "$lib_dir/libmassorca_client_ffi.so"

project="$root/mobile/android/bindings-test"
rm -rf "$project/build/uniffi"
uniffi-bindgen generate --library "$lib_dir/libmassorca_client_ffi.so" --language kotlin \
  --no-format --out-dir "$project/build/uniffi"

# A Kotlin exception already has `message`, and uniffi declares an error field of that name beside it, which Kotlin
# refuses. uniffi's own renaming does not reach fields of types in a submodule, so the generated file is edited: every
# backquoted `message` in it is such a field.
sed -i 's/`message`/`reason`/g' "$project/build/uniffi/uniffi/massorca_client_ffi/massorca_client_ffi.kt"

# A private home: the service refuses one that other users could reach.
work="$(mktemp -d "${RUNNER_TEMP:-/tmp}/massorca-kt.XXXXXX")"
service_pid=""
cleanup() {
  if [ -n "$service_pid" ]; then
    kill "$service_pid" 2>/dev/null || true
    wait "$service_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT
mkdir -m 700 "$work/home"

export MASSORCA_HOME="$work/home"
export MASSORCA_PROFILE=server
export MASSORCA_NO_AUTOSTART=1
export MASSORCA_BIN="$lib_dir/massorca"
export MASSORCA_FFI_LIB_DIR="$lib_dir"

port=47391
"$MASSORCA_BIN" service run --log-stderr --listen "127.0.0.1:$port" >"$work/service.log" 2>&1 &
service_pid=$!

ready=false
for _ in $(seq 1 100); do
  if "$MASSORCA_BIN" --json server status 2>/dev/null | grep -q '"listening"'; then
    ready=true
    break
  fi
  kill -0 "$service_pid" 2>/dev/null || break
  sleep 0.2
done
if [ "$ready" != true ]; then
  echo "the service did not start listening; its log:" >&2
  cat "$work/service.log" >&2
  exit 1
fi

gradle --no-daemon --console=plain -p "$project" test
