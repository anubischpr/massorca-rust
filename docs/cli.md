# The `massorca` command

`massorca` runs the MassOrca service of a profile and controls it. It also reads and
changes the service's settings. Every command talks to the service through the same
protocol as the desktop app, so what works here works for every client.

## Global options

These apply to every command and may come before or after the command name.

| Option | Environment variable | Meaning |
|---|---|---|
| `--profile <NAME>` | `MASSORCA_PROFILE` | Profile to use. Default `default`. Each profile has its own service and data. |
| `--home <DIR>` | `MASSORCA_HOME` | Directory that holds the profiles. Default: the platform data directory. |
| `--host <NAME>` | `MASSORCA_HOST` | Paired host to address instead of the local service (decision 0046). `local`, or no flag, is the local service. A host is never started. |
| `--json` | | Print one JSON document on stdout instead of text. |
| `--no-autostart` | `MASSORCA_NO_AUTOSTART` | Never start the service on its own. |

A profile name is 1 to 32 characters: a lowercase letter or digit first, then lowercase
letters, digits, `-` or `_`.

`MASSORCA_LOG` sets the service log filter in `tracing` directive syntax. The default is `info`.

`--host` applies to every command that talks to a service. `service run`, `service status`,
`service stop`, `service restart`, `service unit`, `pair accept` and the `host` commands work on
this computer only: with `--host` they exit 2 with `cli.host_not_supported`. `service unit` reads
the `--home` flag but never `MASSORCA_HOME`, because its unit is for another machine.

## Commands

| Command | What it does |
|---|---|
| `service run [--log-stderr] [--listen <ADDR>]... [--allow-all-interfaces]` | Runs the service of the profile in this process until it is stopped. `--listen` adds a network listener (also `MASSORCA_LISTEN`, comma separated). `--log-stderr` also writes log lines to stderr. |
| `service unit [--listen <ADDR>]... [--exe <PATH>]` | Prints the systemd user unit that runs `service run` on a Linux server (decision 0045). The unit's `MASSORCA_HOME` is the global `--home`. Defaults: `tailscale`, `%h/.massorca/bin/massorca`, `%h/.massorca/home`. |
| `service status` | Shows whether the service runs. Never starts it. |
| `service info` | Shows the service's identity (profile, pid, version, protocol, capabilities). Starts the service when it is not running. |
| `service stop` | Stops the service. Succeeds when it is not running. |
| `service restart` | Stops the service, then starts it again. |
| `config get <KEY> [--owner OWNER]` | Shows one setting and whether it still has its default value. |
| `config set <KEY> <VALUE> [--owner OWNER]` | Changes one setting. The value is parsed as JSON when it is valid JSON, otherwise taken as text. |
| `config list` | Lists every setting. |
| `project add <PATH> [--name NAME] [--folder \| --git] [--workspaces-root DIR]` | Registers a git repository (its toplevel) or a folder. Without `--folder` or `--git`, a directory inside a repository is a git project. |
| `project list` | Lists every project, ordered by name. |
| `project show <PROJECT>` | Shows one project. `PROJECT` is its id, name or path. |
| `project rm <PROJECT>` | Unregisters a project that has no workspaces. Its files stay. |
| `workspace create <PROJECT> <NAME> [--branch B] [--base REF] [--existing-branch] [--path DIR] [--request-id ID]` | Creates a worktree on a new branch in a git project, or registers a folder in a folder project. Alias `ws`. |
| `workspace list [--project PROJECT]` | Lists every workspace, or those of one project. Worktrees git registers under a workspaces root with no workspace row are listed as `unregistered`; nothing is removed for them. |
| `workspace show <WORKSPACE>` | Shows one workspace. `WORKSPACE` is its id, name or path. |
| `workspace rm <WORKSPACE> [--force] [--discard-ignored] [--forget-orphan]` | Removes a workspace. `--force` discards uncommitted and untracked changes; `--discard-ignored` deletes ignored files; `--forget-orphan` forgets a directory git does not recognise and keeps it. |
| `terminal create <WORKSPACE> [--shell PROGRAM [--arg ARG]...] [--cwd DIR] [--cols N --rows N] [--env KEY=VALUE]... [--title TITLE]` | Starts the default shell, or `--shell` program, in a workspace. Prints the terminal. |
| `terminal list [--workspace WORKSPACE]` | Lists live and exited terminals, or those of one workspace. |
| `terminal show <TERMINAL>` | Shows one terminal: id, state, shell, cwd, size and, once exited, how it ended. |
| `terminal send <TERMINAL> <TEXT>... [--no-enter]` | Types the words joined by spaces, then Enter unless `--no-enter`. |
| `terminal screen <TERMINAL> [--scrollback]` | Prints the visible screen as text. `--scrollback` prints the history above it first. |
| `terminal attach <TERMINAL>` | Interactive: shows the terminal and types into it. `Ctrl+]` detaches. |
| `terminal resize <TERMINAL> --cols N --rows N` | Sets the terminal's size. |
| `terminal kill <TERMINAL> [--force]` | Ends the terminal's processes. The terminal stays listed as exited until closed. |
| `terminal close <TERMINAL>` | Ends the terminal's processes if needed and forgets the terminal. |
| `pair offer [--admin] [--ttl <SECS>]` | Makes a pairing offer for a new device of the service. It is valid 10 minutes by default (1 to 3600 seconds) and works once. Prints the offer alone on the first line. `--admin` adds `host-admin` and `device-admin`. |
| `pair accept <OFFER\|-> [--name <HOST>] [--device-name <NAME>]` | Pairs this computer with the server of an offer and stores it as a host. `-` reads the offer from stdin, which keeps it out of shell history. The host name defaults to the server's name in the offer; the device name to this computer's host name. |
| `device list` | Lists the paired devices with their scopes, pairing and last-seen times, and state. Needs `device-admin`. |
| `device revoke <DEVICE>` | Revokes a device by id, or by name when that is unique. Its connections close at once and it is refused afterwards. |
| `device whoami` | Shows the principal this command runs as, its scopes and its device. |
| `server status` | Shows the listeners, the server pin, the active devices and the pending offers. Without `--host`, also this computer's Tailscale state. |
| `host list` | Lists the hosts this client paired with. Makes no connection. |
| `host show <NAME>` | Shows one host: addresses, pin, device, scopes and pairing time. |
| `host remove <NAME> [--keep-device] [--uninstall --ssh <TARGET>]` | Forgets a host. Unless `--keep-device`, revokes this device on the server when the host answers. `--uninstall` also stops and removes the service over SSH; if that fails the host is kept, so the command can be run again. |
| `host status [<NAME>]` | Connects to one host, or to every host, within 10 seconds and reports `connected`, `unreachable`, `refused`, `identity_mismatch` or `error`. A single named host that does not answer exits 4. |
| `host discover` | Lists the online Tailscale peers and whether each answers on port 7347. Without a tailnet it says why and lists nothing. |
| `host add-ssh <SSH-TARGET> --binary <PATH> [--name <HOST>] [--listen <ADDR>]... [--ssh-arg <ARG>]... [--device-name <NAME>]` | Installs or upgrades massorca on a Linux x86_64 or aarch64 host over SSH, as a systemd user unit, then pairs with it. Progress goes to stderr. A host that already exists is upgraded and not paired again. `--ssh-arg` takes one option per flag: `--ssh-arg -p --ssh-arg 2222`. |
| `ext ...` | Reserved for extensions. Every form exits 1 with `extension.not_implemented`. |

`--owner` is `core` (the default) or `ext:<id>`. Example key: `workspaces.default_root`,
which must be an absolute path.

### Projects and workspaces

A relative path given to `project add`, `workspace create --path`, or `--workspaces-root`
is taken from the directory the command runs in. A selector that is a path (it contains a
separator, or starts with a dot, such as `.` or `../wt`) is made absolute the same way; ids and
names are sent as typed. Workspaces of a project go under its
workspaces root, which defaults to `workspaces.default_root` (a folder named after the
project) at the time the project is added.

`workspace rm` behaves as follows:

- A worktree is removed with its directory. Uncommitted or untracked changes refuse the removal
  with `workspace.dirty` and exit 1; `--force` discards them.
- Ignored files refuse the removal with `workspace.ignored_files`, even with `--force`. Git
  does not list them, and removing the worktree deletes them, so only `--discard-ignored`
  removes them.
- Its branch is deleted only when git proves the branch is merged. Otherwise the output
  says `kept branch <B>: not merged`.
- A folder workspace is forgotten and its directory stays (`kept directory <PATH>`).
- A directory that git does not recognise as its worktree refuses with `workspace.orphan`, even
  with `--force`. `--forget-orphan` forgets the row and keeps the directory; git keeps its record
  of the worktree until it prunes it.
- The main worktree of a repository is never removed, and a locked worktree is refused
  with `workspace.locked`.

### Terminals

A terminal belongs to one workspace and runs in the service, so it keeps running after
the command that started it exits. `terminal list`, `show`, `send` and `screen` each
connect and disconnect on their own.

- `--cwd` is relative to the workspace root, or absolute. Unlike the paths above, a
  relative `--cwd` is not taken from the directory the command runs in.
- `--shell` names the program the terminal runs. Text typed into a shell is `terminal send`.
- `terminal screen` waits up to 5 seconds for the terminal's screen. It replays the screen
  in a local emulator, so colours and cursor movement are not printed, only the text.
- `terminal attach` needs a terminal on stdin and stdout, else it exits 2 before starting
  anything. It puts the local terminal in raw mode, resizes the terminal to the local size,
  and restores the local terminal on every exit. It prints the terminal's exit and returns
  0 when the terminal ends. `--json` is refused, because the output is the terminal's.
- `terminal kill` and `terminal close` end the processes; stopping the service ends every
  terminal and nothing is restored on the next start.

`service run` prints one line in text mode, `service running for profile <name> (pid <n>)`.
With `--json` it prints the start document on stdout, and the service log goes to the
profile's log files only unless `--log-stderr` is given.

### Hosts and pairing

A server is `massorca service run --listen <ADDR>` on another machine. `--listen` takes
`tailscale[:PORT]` (this node's Tailscale address, the default port 7347), `localhost:PORT` or
`IP:PORT`. An address on every interface (`0.0.0.0`, `::`) needs `--allow-all-interfaces`.

The server's owner makes an offer, and the other computer accepts it:

```sh
# on the server
massorca pair offer --admin
# on the other computer, reading the offer from stdin
massorca pair accept - --name vps
```

The accepted host is stored in this profile. `--host vps` then addresses it, and `host list`,
`host status` and `host remove` manage it. A host that does not answer is reported as
`unreachable`; nothing is inferred about the work on it (decision 0044).

## Output

Text mode writes results to stdout. Errors go to stderr as one line:

```text
error: <message> (<code>)
```

With `--json`, stdout holds exactly one document.

A success:

```json
{"ok": true, "result": {"running": false, "info": null}}
```

A failure, also on stdout, with the same exit code as text mode:

```json
{"ok": false, "error": {"code": "config.invalid_value", "message": "...", "data": null}}
```

A command line mistake with `--json` prints `usage.invalid_arguments` in this envelope.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | The service answered with an error, or local data could not be used. The code is printed. |
| 2 | The command line is invalid, including an invalid profile name or log filter, an invalid `--listen` (`usage.invalid_listen`), `--host` on a command that works on this computer only (`cli.host_not_supported`), or `terminal attach` without a terminal on stdin and stdout. |
| 3 | `service run`: another service already owns this profile. |
| 4 | The service is not running and could not be reached or started. For a host: `host.unreachable`, `host.identity_mismatch`, or a refusal such as `auth.device_revoked`. |

## Logs and files

Each profile lives in `<home>/profiles/<name>/`. The service writes daily log files to
`logs/` as `service.<date>.log`, keeping the last seven days.

## Not yet available

Extension commands arrive in later milestones. Deferred from ADR 0008 (branch prefixes,
generated names, setup scripts, terminal and agent guards on removal, squash-merge detection) are
not available yet.
