# 0113: CLI on PATH survives updates

Status: accepted, 2026-11-25.

## Context

Decision 0088 (D6) points the command at the binary that ran `cli install`. An update that moves that binary leaves the
command on a path that no longer exists: `massorca` fails until `cli install` runs again, and `cli status` can only say
`different_binary`. 0088 names the fix, a stable installed location, but no installer places one yet. The command has to
follow the binary that the update left behind.

## Decision

- D1 Stale. A command that `cli install` made, whose binary is gone, is `stale` in `cli status`, with the missing path as
  `points_to`. A command that runs another binary that is still there stays `different_binary`: two working installs are
  a choice, so nothing changes one silently.
- D2 Repair at service start. The desktop starts its bundled `massorca` as the service with a hidden `--repair-command`
  flag (`crates/desktop/src/service/launch.rs`). That service points a stale command at itself, through the same
  placement `cli install` uses, so the command follows the bundle after an update. A service that `cli`, a person or a
  test starts does not repair, and `cli status` and the other commands change nothing.
- D3 Only a gone command is repaired. A start never re-points a command that works, even one that runs another binary,
  so a development build that starts a service cannot take over an install.
- D4 Best effort. The repair changes only the command file or link that massorca made. A failure is logged as a warning
  and the service starts anyway.
- D5 Unchanged. `cli install` still points the command at the binary it runs and refuses files it did not make.
  `cli uninstall` removes a stale command and its folder entry the way it removes any other command.
- D6 Not covered. A binary that an update leaves at its old path still runs, and the command keeps it until
  `cli install` points it at the new one. `different_binary` shows that state.

## Alternatives

- A stable copy in a fixed folder, which 0088 names as the follow-up. It needs the installers to place and update the
  copy, which this change does not own.
- A launcher that reads a pointer file the app writes. It needs the same start-time write plus a script per platform.
  The repair reuses the placement that `cli install` already has.
- Re-pointing whenever the command names another binary. Every development build would take over the install.

## Consequences

- An update that moves the binary is repaired at the next service start. Until then `massorca` fails, and `cli status`
  reports `stale` with the fix.
- `cli status` has one new state, `stale`.
- The flag is hidden in help, as the other flags the client sets are (`crates/cli/src/args.rs`). Tests never pass it,
  so no test can repair the developer's command.
