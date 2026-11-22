#!/usr/bin/env bash
# Self-test for check-glibc-floor.sh: version order, readelf parsing, then real programs when cc and readelf exist.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
checker="$here/check-glibc-floor.sh"
# shellcheck source=check-glibc-floor.sh
source "$checker"

failures=0

# report <description> <want status> <got status>
report() {
  if [ "$3" -eq "$2" ]; then
    echo "ok   $1"
  else
    echo "FAIL $1: exit $3, want $2" >&2
    failures=$((failures + 1))
  fi
}

# status_of <command...>: prints the exit status of the command, its output hidden
status_of() {
  local got=0
  "$@" >/dev/null 2>&1 || got=$?
  echo "$got"
}

# judged <floor> <fixture function>: prints the exit status of judge_listing on the fixture's readelf text
judged() {
  local got=0
  "$2" | judge_listing "$1" fixture >/dev/null 2>&1 || got=$?
  echo "$got"
}

# readelf text of a program that needs GLIBC_2.2.5, GLIBC_2.17 and GLIBC_2.34
needs_2_34() {
  cat <<'EOF'
Version needs section '.gnu.version_r' contains 2 entries:
 Addr: 0x0000000000000a48  Offset: 0x000a48  Link: 7 (.dynstr)
  000000: Version: 1  File: libc.so.6  Cnt: 3
  0x0010:   Name: GLIBC_2.2.5  Flags: none  Version: 2
  0x0020:   Name: GLIBC_2.17  Flags: none  Version: 3
  0x0030:   Name: GLIBC_2.34  Flags: none  Version: 4
EOF
}

# readelf text of a program that needs GLIBC_2.9 and GLIBC_2.10; 2.10 is the newer one
needs_2_10() {
  cat <<'EOF'
  000000: Version: 1  File: libc.so.6  Cnt: 2
  0x0010:   Name: GLIBC_2.9  Flags: none  Version: 2
  0x0020:   Name: GLIBC_2.10  Flags: none  Version: 3
EOF
}

# readelf text of a program whose only library is libgcc_s, which names GCC versions and no glibc version
needs_no_glibc() {
  cat <<'EOF'
  000000: Version: 1  File: libgcc_s.so.1  Cnt: 1
  0x0010:   Name: GCC_3.0  Flags: none  Version: 2
EOF
}

# readelf text naming a glibc version that is not a number (glibc's internal GLIBC_PRIVATE)
needs_private() {
  cat <<'EOF'
  000000: Version: 1  File: ld-linux-x86-64.so.2  Cnt: 1
  0x0010:   Name: GLIBC_PRIVATE  Flags: none  Version: 2
EOF
}

report "2.34 is newer than 2.31" 0 "$(status_of version_newer 2.34 2.31)"
report "2.31 is not newer than 2.31" 1 "$(status_of version_newer 2.31 2.31)"
report "2.17 is not newer than 2.31" 1 "$(status_of version_newer 2.17 2.31)"
report "2.10 is newer than 2.9 (numbers, not text)" 0 "$(status_of version_newer 2.10 2.9)"
report "3.0 is newer than 2.31" 0 "$(status_of version_newer 3.0 2.31)"

report "a program needing 2.34 fails the 2.31 floor" 1 "$(judged 2.31 needs_2_34)"
report "a program needing 2.34 passes the 2.34 floor" 0 "$(judged 2.34 needs_2_34)"
report "2.10 fails the 2.9 floor (numbers, not text)" 1 "$(judged 2.9 needs_2_10)"
report "2.10 passes the 2.10 floor" 0 "$(judged 2.10 needs_2_10)"
report "a program with no glibc version passes" 0 "$(judged 2.31 needs_no_glibc)"
report "a non-numbered GLIBC version fails" 1 "$(judged 2.31 needs_private)"

report "no arguments is a usage error" 2 "$(status_of bash "$checker")"
report "a missing program fails" 1 "$(status_of bash "$checker" 2.31 "$here/no-such-program")"

if command -v cc >/dev/null 2>&1 && command -v readelf >/dev/null 2>&1; then
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  printf 'int main(void) { return 0; }\n' > "$tmp/host.c"
  cc -o "$tmp/host-program" "$tmp/host.c"
  # A program built on a current runner needs glibc 2.34 or newer, so it must fail the 2.31 floor.
  report "a program built on this host fails the 2.31 floor" 1 "$(status_of bash "$checker" 2.31 "$tmp/host-program")"
  report "a program built on this host passes a 99.0 floor" 0 "$(status_of bash "$checker" 99.0 "$tmp/host-program")"
  report "a text file is refused as not ELF" 1 "$(status_of bash "$checker" 2.31 "$checker")"
else
  echo "skip real programs: needs cc and readelf"
fi

if [ "$failures" -gt 0 ]; then
  echo "$failures check(s) failed" >&2
  exit 1
fi
echo "glibc floor checks passed"
