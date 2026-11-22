# 0075 The SSH add installs the Linux program that ships with the desktop

Status: accepted, 2026-11-22.

## Context

Adding an SSH host (decision 0044) uploads a Linux `massorca` program to the host and then pairs with the service it starts.
Decision 0074 left the choice of that program open: the design had no field for it, and it needed the owner. The owner decided:
the desktop installs the Linux program that matches its own version, and the builds for x86_64 and aarch64 Linux ship with the
app as bundled resources.

## Decision

- The Add host dialog, SSH path, tests the host before anything is installed. The test runs `uname -sm` over ssh, the same
  step bootstrap starts with (`probe_machine`). A host that is not Linux on x86_64 or aarch64 is refused with the sentence the
  host's answer gives.
- The program is the one for the machine the test named: `resources/massorca-linux-<arch>` in the folder next to the desktop
  executable, where `<arch>` is `x86_64` or `aarch64`. Packaging that folder is a separate release task.
- Until the packaged programs are there, the dialog says plainly that the app has no program for that machine, and offers
  "Choose program file" under Advanced options. That override stays, for developers. The main flow has no file picker.
- "Add host" stays disabled until the test passes for the target the fields make now (the owner's decision in
  `docs/design/screens/shell-setup-hosts.md`).

## Alternatives

- Download the program from a release when the host is added: rejected. It needs the internet, and a release that lags the
  app would install a program of another version.
- A file picker in the main flow: rejected. Users would have to find the right binary for the host's machine, and nothing
  would stop a version mismatch.
- Build the program on the host: rejected. The host would need a Rust toolchain.

## Reasons

- Versions always match: the program comes from the same build as the desktop, so the host runs the version the app speaks.
- It works without internet: nothing is fetched when a host is added.
- Nobody has to find a binary: the main flow needs no path from the user.

## Consequences

- The package must place `resources/massorca-linux-x86_64` and `resources/massorca-linux-aarch64` next to the executable. Until
  then, the desktop cannot add an SSH host without the developer override.
- The SSH add needs no network from the desktop beyond ssh to the host itself.
- Not built here: the Server path of the Add host dialog (a pairing code inside the dialog; accepting an offer is on the
  Pairing page) and the SSH key picker.
