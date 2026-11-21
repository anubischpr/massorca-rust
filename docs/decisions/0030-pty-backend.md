# 0030: PTY backend

Status: accepted, 2026-11-21.

## Context

The service runs shells in pseudo-terminals on Windows (ConPTY), Linux and
macOS (Unix ptys). The service is a background process with no console
(0003), so a terminal must never open a window. Crates in this workspace deny
`unsafe_code` except in a module that explains each use; the fewer of those
the better. Research checked crate sources and platform documentation on
2026-11-21.

## Decision

- **`portable-pty` 0.9.0** (wezterm) on every platform: `openpty` with the
  size, `spawn_command` with a `CommandBuilder` (program, args, cwd, a fully
  built environment after `env_clear`), `try_clone_reader`, `take_writer`,
  `resize`, and the child's `wait`, `process_id` and, on Windows,
  `as_raw_handle`. Our code calls only its safe API.
  - Windows: it calls `CreatePseudoConsole` (a side-loaded `conpty.dll` is
    preferred when present; we ship none) and spawns with
    `EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT` only. A
    pseudoconsole has no hosting window, also under a detached parent.
  - Unix: the child calls `setsid()`, takes the pty as controlling terminal
    and closes every inherited descriptor above 2.
- **Blocking IO on dedicated threads.** One reader thread per terminal reads
  the master and feeds the screen (0034); one waiter thread blocks in
  `wait`. tokio's `AsyncFd` would need `unsafe` for a borrowed pty fd and does
  not apply to ConPTY pipes. Two threads per terminal are cheap at the 256
  limit of 0033.
- **Exit comes from the process, not from end of output.** On Windows the
  pseudoconsole keeps the output pipe open after the child exits, so the
  waiter thread's `wait` decides `exited`. The pseudoconsole is then closed
  off the reader thread (before Windows 11 24H2, `ClosePseudoConsole` blocks
  until its output is drained), which ends the output, and the exit is
  recorded once the reader reaches that end (at most 5 s; 500 ms on Unix), so
  attachments receive all output before `exited`.
  On Linux a read returns buffered output first and then `EIO`, which counts
  as end of output, as does a zero-length read.
- **We resolve the program path ourselves** (`PATH`, and `PATHEXT` on
  Windows) and pass an absolute path: `portable-pty` hands an unresolved
  name to `CreateProcessW`, which looks in the working directory rather than
  `PATH`.
- **Kill is ours.** `portable-pty`'s `Child::kill` signals only the direct
  child (and 0.9.0 misreports the Windows result), so tree termination is
  0031.
- **Ctrl+C** is input: the byte `0x03` written to the terminal, which ConPTY
  turns into a Ctrl+C event and a Unix line discipline into SIGINT. No signal
  API is exposed in Phase 2.

## Alternatives

- **`conpty` 0.7.0 plus `pty-process` 0.5.4.** Two crates and two APIs;
  `conpty` exposes no process handle, so no job object, and was last released
  in 2024; `pty-process` is Unix only.
- **`alacritty_terminal`'s `tty` module.** Works, but pulls the whole emulator
  and its event loop to get a pty.
- **`winpty-rs`.** Links the legacy WinPTY library; output as `OsString`, not
  bytes.
- **Direct Win32 and libc calls.** Most control (job assignment at creation,
  `CREATE_SUSPENDED`), at the cost of a large unsafe surface that
  `portable-pty` already carries and has years of use behind.
- **A wezterm git revision** with the unreleased Windows kill fix. Not needed,
  because we never call its kill.

## Consequences

- `portable-pty` is edition 2018 on `winapi` 0.3 and has had no release since
  2025-02. If it stalls further, the `pty` module is the only place to
  replace.
- Tests that spawn real shells run on Windows and Linux in CI; a
  pseudoconsole needs no desktop session, so they open no window.
