# 0164: Vendor CLI fixes: Gentle Shell uninstall, and the Manual launch value of Claude Code

Status: accepted, 2026-11-25.

## Context

Two vendor CLIs changed under the declarations MassOrca shipped, both checked against the vendors' docs and `--help` on
2026-11-25.

- Gentle Shell. Its package installs with `npm install -g gentle-pi`, which the vendor documents on every OS. Its
  uninstall ran `npm uninstall -g gentle-pi`, which leaves the CLI's own data behind. The vendor's `gentle-shell
  self-uninstall --yes` removes that data and the package, and refuses an npm-link install. Its bundled installer puts a
  `gentle-shell.cmd` launcher on the user `Path` in the registry, not in npm's folder (decision 0092).
- Claude Code. Decision 0083 passes `--permission-mode default` for Manual. Claude Code 2.1.330's `--help` lists
  `manual` among the `--permission-mode` choices and not `default`; its docs name `default` with `manual` as an alias.

## Decision

1. **Gentle Shell uninstall.** `cli.uninstall` runs `gentle-shell self-uninstall --yes` on each OS. The install stays
   `npm install -g gentle-pi`. The uninstall still runs only for a CLI the lockfile records as installed by the package
   (decision 0062).
2. **Own binary in an uninstall.** An uninstall step may start the CLI's own `binary` as well as an allowlisted program
   (`is_uninstall_program`). The rule applies to uninstall steps only: an install starts an allowlisted program, as before.
   Only a built-in may declare vendor commands (0062, V1), so only the first-party packages can use this.
3. **Detection of the bundled launcher.** The `verify` step stays `gentle-shell --version` and checks the exit code. The
   extension's program search uses the registry-merged `PATH` (decision 0092), and a `.cmd` launcher in a folder only the
   user `Path` lists resolves under Windows' `PATHEXT`. A unit test covers this with an injected registry value
   (`a_launcher_on_the_registry_user_path_resolves_through_the_merged_search`).
4. **Launch value of a permission mode.** A mode may declare `launch_values`: the vendor's spellings of its launch value,
   in preference order. Its `args` then hold `{value}` once, directly after the flag it belongs to. At launch the first
   spelling that the CLI's `--help` lists for that flag is passed. When the help lists none of them, or cannot be read,
   the first spelling is passed. The help is the cached `--help` probe that the instruction channel already runs (one run
   per program path, modification time and probe arguments), so no second probe is run.
5. **Claude Code's Manual.** The mode keeps the id `default`, so sessions stored before this change keep their mode. Its
   declaration is `args: ["--permission-mode", "{value}"]` with `launch_values: ["default", "manual"]`. The CLI that lists
   `manual` only gets `manual`. A CLI that lists `default`, or neither, gets `default`.
6. **Validation.** A kind is refused when a `launch_values` spelling is invalid or repeated, or when its `args` do not hold
   `{value}` exactly once right after a flag. An `args` with `{value}` and no `launch_values` is refused too. Without
   `launch_values` (or with an empty list) the `args` hold no placeholder, as before.
7. **Agent launch uses the same search path.** Session start, resume and the kind listing resolve an agent's program with
   the host's search path (`vendor_search_env`, decision 0092), not the process `PATH` alone. `AgentsConfig` carries it as
   `VendorSearch`: the service builds it with `VendorSearch::host()`, and tests inject a fixed one with
   `VendorSearch::fixed`. This supersedes the deferral of agent launch in decision 0092.

## Alternatives

- Add `gentle-shell` to the vendor allowlist. Rejected: every built-in install and uninstall step could then start it.
  The uninstall rule above names the CLI's own binary, which the `verify` step already runs.
- A probe per spelling, each with its own `contains` check. Rejected: it runs the help twice and needs an "A and not B"
  rule. Choosing the first listed spelling covers the same cases with one run.
- Change the mode's id to `manual`. Rejected: stored sessions and `agent.set_mode` callers name `default`.

## Consequences

- A launch value that the help does not show is passed as the first spelling, so an unknown or unreadable help keeps
  the behaviour that decision 0083 documented.
- The choice is whole-word: a listed spelling counts when it appears in the flag's own option block (its line and the
  lines it wraps onto), not in a neighbouring option, and `manual` does not match `manually`.
- Agent launch resolves programs on the host's search path, so a launcher that only the registry's user `Path` lists is
  detected in the kind listing and starts. The run's own environment is unchanged: its `PATH` is still the service's, so
  a launcher that calls a tool only the registry `Path` lists can still fail when it runs.
- An uninstall can start a CLI's own binary. This is limited to the uninstall steps of built-in packages, and the binary
  must be the one the same package's `verify` step runs.
