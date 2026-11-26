#!/usr/bin/env bash
# Starts the massorca service on the runner for the Android emulator flow (decision 0080, task A11). It listens on
# localhost:7347, which the emulator reaches at 10.0.2.2, and holds the folder project, its workspace and one terminal
# in that workspace, which the flow opens.
# Later steps read the paths and the CLI location from the job environment.
set -euo pipefail

readonly PORT=7347
readonly PROFILE=e2e
root="${RUNNER_TEMP:?RUNNER_TEMP is not set}"
home="$root/massorca-home"
bin="$GITHUB_WORKSPACE/target/debug/massorca"
project_dir="$root/e2e-folder"
log="$root/massorca-service.log"

mkdir -p "$home" "$project_dir"
chmod 700 "$home"
export MASSORCA_HOME="$home" MASSORCA_PROFILE="$PROFILE" MASSORCA_NO_AUTOSTART=1
cat >> "$GITHUB_ENV" <<EOF
MASSORCA_HOME=$home
MASSORCA_PROFILE=$PROFILE
MASSORCA_NO_AUTOSTART=1
MASSORCA_BIN=$bin
EOF

setsid "$bin" service run --listen "localhost:$PORT" --log-stderr </dev/null >"$log" 2>&1 &

listening=0
for _ in $(seq 1 90); do
  if "$bin" --json server status 2>/dev/null |
    jq -e '[.result.network.listeners[] | select(.state == "listening")] | length > 0' >/dev/null; then
    listening=1
    break
  fi
  sleep 2
done
if [ "$listening" != 1 ]; then
  tail -n 80 "$log" >&2
  echo "the service did not start listening on localhost:$PORT" >&2
  exit 1
fi

project_id="$("$bin" --json project add "$project_dir" --folder --name e2e-folder | jq -r '.result.id')"
test -n "$project_id"
workspace_id="$("$bin" --json workspace create "$project_id" e2e-folder | jq -r '.result.id')"
test -n "$workspace_id"
# One live terminal in bash, so its prompt ends in $, which the flow waits for. The Terminal view opens it at once.
terminal_id="$("$bin" --json terminal create "$workspace_id" --shell /bin/bash | jq -r '.result.id')"
test -n "$terminal_id"
printf 'MASSORCA_E2E_TERMINAL=%s\n' "$terminal_id" >> "$GITHUB_ENV"
echo "service listening on localhost:$PORT with the e2e-folder workspace and its terminal"
