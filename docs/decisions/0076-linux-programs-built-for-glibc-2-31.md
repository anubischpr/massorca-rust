# 0076 The Linux programs are built against a glibc 2.31 floor

Status: accepted, 2026-11-22.

## Context

Decision 0075 ships `resources/massorca-linux-x86_64` and `resources/massorca-linux-aarch64` next to the desktop
executable. The desktop copies the program for a host's machine to that host and starts it there, and we do not control
the hosts. So the program must load on the oldest Linux we support. Decision 0014 already sets that floor: the glibc of
Ubuntu 20.04, which is glibc 2.31.

A program built on a current Linux needs a newer glibc. The M2 milestone notes that a build made on Ubuntu 24.04 needs
glibc 2.39 (`docs/milestones/m2-server-on-vps.md`), which an Ubuntu 20.04 host cannot run.

## Decision

- CI builds `massorca` (package `massorca-cli`) for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, with
  cargo-zigbuild 0.24.1 and zig 0.17.1, targeting glibc 2.31 (`--target <triple>.2.31`). zig links against glibc 2.31
  stubs, so the link can pick symbol versions only up to 2.31. The check below confirms it.
- `scripts/check-glibc-floor.sh 2.31` reads the GLIBC symbol versions each program names (`readelf --version-info`) and
  fails when one is newer than 2.31. The job runs its self-test first; that test builds a program on the runner, which
  must fail the floor.
- The job checks that each file is a 64-bit little-endian program for its machine, runs the x86_64 program with
  `--version`, and uploads the two files as the artifact `massorca-linux-resources`. The artifact's root holds the
  programs, so the package step places that folder as `resources/` next to the desktop executable.
- The job uses no secrets and publishes nothing.

## Alternatives

- **Static musl.** One file with no glibc needs, so it runs on any distribution. Not chosen now: musl changes the C library
  the program calls (memory and name lookups), and the native crates (ring, bundled SQLite) would need musl builds that
  nobody has verified here. Switch to it if a host with glibc older than 2.31 must be supported.
- **Build on an Ubuntu 20.04 runner.** Gives glibc 2.31 directly, but ties the floor to a runner image that GitHub retires
  on its own schedule. zig pins the version inside the job.
- **A floor of Ubuntu 22.04 (glibc 2.35).** Easier to build, but refuses hosts that the 20.04 floor covers.

## Reasons

- The program stays dynamically linked to the host's glibc, so name lookup and locale behaviour match the host's own tools.
  Static glibc cannot load the name-service modules that those lookups use.
- The floor is enforced on every build: a dependency that needs a newer glibc symbol fails CI, not a user's host.

## Consequences

- Hosts with glibc older than 2.31 (for example RHEL 8 or Debian 10, both glibc 2.28) cannot run the program. The add-host
  test in decision 0075 checks only `uname -sm`, so such a host passes the test and then fails when the program starts.
- The check covers glibc symbol versions only. It does not check other shared libraries, such as OpenSSL. A dependency
  that links one needs its own check.
- The artifact does not keep the executable bit. That does not matter: the desktop reads the file's bytes and sets the mode
  on the host (`chmod 700` in `crates/core-remote/src/bootstrap.rs` on `desktop/hosts-screens`).
