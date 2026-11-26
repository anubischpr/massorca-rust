#!/usr/bin/env bash
# Installs the pinned Maestro CLI for the Android emulator flow (decision 0080, task A11) and puts it on the PATH of the
# later steps. The archive must match the SHA-256 that the release publishes in checksums_sha256.txt.
set -euo pipefail

readonly MAESTRO_RELEASE="cli-2.12.1"
readonly MAESTRO_SHA256="54dbb0716425626604973cbb374c585ca30272375d9ce15177a1152765ddd9d7"

dir="${RUNNER_TEMP:?RUNNER_TEMP is not set}/maestro"
mkdir -p "$dir"
curl -fsSL --retry 5 --retry-delay 5 --retry-all-errors \
  -o "$dir/maestro.zip" \
  "https://github.com/mobile-dev-inc/maestro/releases/download/${MAESTRO_RELEASE}/maestro.zip"
echo "${MAESTRO_SHA256}  ${dir}/maestro.zip" | sha256sum -c -
unzip -q "$dir/maestro.zip" -d "$dir/install"
bin="$(find "$dir/install" -type f -path '*/bin/maestro' -print -quit)"
test -n "$bin"
chmod +x "$bin"
echo "$(dirname "$bin")" >> "$GITHUB_PATH"
"$bin" --version
