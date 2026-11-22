#!/usr/bin/env bash
# Fails when a Linux program needs a GLIBC symbol version newer than the floor.
# Usage: scripts/check-glibc-floor.sh <floor, e.g. 2.31> <program>...
set -euo pipefail

# Succeeds when dotted version $1 is newer than $2, comparing numbers part by part (2.10 is newer than 2.9).
version_newer() {
  [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n 1)" = "$1" ]
}

# Judges `readelf --version-info` text on stdin against the floor $1; $2 names the program in messages.
judge_listing() {
  local floor="$1" program="$2" versions invalid highest
  versions="$(sed -n 's/.*Name: GLIBC_\([^ ]*\).*/\1/p' | sort -uV)"
  invalid="$(printf '%s\n' "$versions" | grep -v -E '^([0-9]+(\.[0-9]+)*)?$' || true)"
  if [ -n "$invalid" ]; then
    echo "FAIL $program names a GLIBC version that is not numbered: $invalid" >&2
    return 1
  fi
  highest="$(printf '%s\n' "$versions" | tail -n 1)"
  if [ -n "$highest" ] && version_newer "$highest" "$floor"; then
    echo "FAIL $program needs GLIBC_$highest, newer than the floor GLIBC_$floor" >&2
    return 1
  fi
  echo "ok   $program needs GLIBC_${highest:-none} at most (floor GLIBC_$floor)"
}

# Checks one program; readelf must first read it as an ELF program.
check_program() {
  local floor="$1" program="$2" listing
  if ! command -v readelf >/dev/null 2>&1; then
    echo "readelf (binutils) is required" >&2
    return 2
  fi
  if ! listing="$(readelf --version-info "$program" 2>&1)"; then
    echo "FAIL $program: readelf cannot read it as an ELF program" >&2
    return 1
  fi
  printf '%s\n' "$listing" | judge_listing "$floor" "$program"
}

main() {
  local floor="${1:-}" status=0 program
  if [ "$#" -lt 2 ] || [[ ! "$floor" =~ ^[0-9]+(\.[0-9]+)*$ ]]; then
    echo "usage: $0 <floor, e.g. 2.31> <program>..." >&2
    return 2
  fi
  shift
  for program in "$@"; do
    check_program "$floor" "$program" || status=1
  done
  return "$status"
}

# Sourced by the self-test, which calls the functions above; run directly, it checks its arguments.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  main "$@"
fi
