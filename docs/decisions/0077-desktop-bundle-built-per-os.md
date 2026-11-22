# 0077 The desktop bundle is built per OS and checked in CI

Status: accepted, 2026-11-22.

## Context

Decision 0075 has the desktop look for `resources/massorca-linux-<arch>` next to its executable. Decision 0076 has CI
build those two programs as the artifact `massorca-linux-resources`. Nothing yet builds the desktop bundle that carries
them.

## Decision

- A `desktop-bundle` job runs on ubuntu-latest and windows-latest, the OS matrix CI already uses. macOS is not built,
  because CI does not build the desktop there.
- Each job builds `massorca-desktop` in release mode, copies it to `bundle/`, downloads `massorca-linux-resources` into
  `bundle/resources/` and uploads `bundle/` as `massorca-desktop-bundle-<OS>`.
- The job asserts that `bundle/` holds the desktop executable and exactly the two programs in `resources/`, and that
  their SHA-256 values equal the ones the Linux programs job wrote after its floor check.
- The glibc floor check runs in the Linux job only, because it reads ELF headers with `readelf`. The Windows job gets the
  same bytes, so the checksum comparison covers it.
- The jobs run when the release group changes: core, ui or `scripts/`.
- No release, signing or packaging. Like the programs in 0076, the bundle artifacts do not keep the executable bit.

## Alternatives

- **Debug build.** Faster, and cached with the ui job, but the bundle would not be what users run.
- **Install binutils on Windows for the floor check.** Adds a toolchain to check bytes that the Linux job already checks.
- **A separate checksum artifact.** Works, but job outputs carry the same two values without another artifact.

## Consequences

- Each desktop job compiles GPUI in release mode, so a cold cache is slow. Caches are kept per OS.
- The bundle is unsigned and unpackaged. Installers come later.
- Each bundle artifact repeats the two programs, so the artifacts are large.
