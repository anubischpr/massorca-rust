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
| `agent kinds` | Lists the agent kinds: id, name, owner, package state (`installed`), whether a program resolves (`detected`: `yes`, `no` or `invalid`) and the resolved path. Problems follow the table. |
| `agent start <KIND> --workspace <WS> [--prompt TEXT] [--read-only] [--title TITLE] [-- ARGS...]` | Starts an agent in the workspace root, in a terminal of its own. Prints the session. |
| `agent list [--workspace WS]` | Lists agent sessions, newest first: execution, activity and title. |
| `agent status [SESSION] [--watch] [--until STATE] [--timeout SECS]` | Shows one session's execution and activity, or every session's. `--watch` prints every change; `--until` waits for `working`, `waiting`, `idle`, `done` or `exited`. |
| `agent stop <SESSION> [--force]` | Ends the session's run. The session stays listed as exited (`killed`). |
| `agent prompt <SESSION> <TEXT> [--force]` | Types TEXT into the session's terminal and submits it, once the agent is `idle` or `done`. Prints the bytes handed to the terminal. `--force` types it while the agent works or waits. |
| `agent resume <SESSION>` | Starts a new run of an exited session with the kind's resume arguments, in a new terminal. The session keeps its id and agent ref. |
| `agent forget <SESSION>` | Forgets a session whose run has ended, with its terminal and generated files. |
| `agent attach <SESSION>` | Interactive: `terminal attach` on the session's run. `Ctrl+]` detaches. |
| `agent hook <EVENT> [PAYLOAD]` | Called by an agent's hooks inside its run: reports the event to the service. Always exits 0. |
| `agent delegate [--from SESSION] --prompt TEXT [--kind K] [--name N] [--title T] [--read-only]` | Starts a child session of SESSION with TEXT as its first prompt, in a new workspace of its own unless `--read-only`. The child's title is T, or the first line of TEXT (at most 80 characters). Prints the child. |
| `agent tree <SESSION>` | Shows the session's delegation tree: its root, then the root's children. |
| `agent bring-back <CHILD> [--no-merge] [--no-prompt]` | Merges a finished child's branch into its parent's worktree with a merge commit, and tells the parent's run. |
| `agent credentials import <KIND> --from <DIR> [--replace]` | Copies the kind's declared login files from `DIR` into the profile's home for the kind. Prints file names and sizes only, and says to stop using the source login. |
| `agent credentials show <KIND>` | Shows whether the profile has a home for the kind, and which declared files are in it, with sizes. |
| `agent credentials remove <KIND>` | Deletes the profile's home for the kind; its runs use the agent's usual home again. |
| `agent accounts list <KIND>` | Lists the kind's accounts: id, label, state, identity, whether active, and the projects that chose each. |
| `agent accounts add <KIND> [--label L] [--no-attach]` | Signs a new account in: runs the kind's login in a service-owned terminal, prints the account and the terminal, then attaches to it unless `--no-attach`. Refused with `--json` unless `--no-attach`. Built-in sign-in is declared for `claude` only (`claude auth login`); a kind without a sign-in is refused with `agent.credentials_unsupported`, which says to sign in from the vendor's CLI, then import the login. |
| `agent accounts import <KIND> --from <DIR> [--label L]` | Copies the kind's declared login files from `DIR` into a new account, then checks it. Prints `imported`, `failed` (with the reason) or `not_found`. Names and sizes only. |
| `agent accounts activate <KIND> <ACCOUNT> [--project P]` | Makes the account the kind's active one, or the account project `P` uses for the kind. |
| `agent accounts remove <KIND> <ACCOUNT>` | Deletes the account after its sign-out, if the kind declares one. Refused while a live run uses it. |
| `agent accounts check <KIND> [<ACCOUNT>]` | Runs the kind's check on an account, or on the active one, and prints its state. |
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

### Agents

Agents run as programs in service-owned terminals (`docs/architecture/agents.md`).

- `agent kinds` lists the built-in kinds (owner `core`: `claude`, `codex`, `antigravity`,
  `gentle-shell`) and the profile's own declarations in `<profile>/agents/kinds/*.json`
  (owner `user`). The files are read again on every call. `INSTALLED` is the kind's package
  state (`enabled` for every kind until packages land); `DETECTED` is `yes` when one of its
  programs resolves: an absolute path that is a file, or a bare name on the service's `PATH`
  (on Windows also with the `PATHEXT` extensions). `PATH` shows the resolved program. A file
  that does not parse, breaks a rule, or reuses a built-in id is listed with its problems,
  never detected, and never started. `--json` prints the `agent.kinds` result, where
  `installed` is deprecated and equals `detected`.
- `agent start` runs the kind's program directly in a new terminal of the workspace (no
  shell in between), with the first prompt as the kind declares. A workspace holds one live
  session that is not `--read-only`; a second writing start fails with
  `agent.workspace_busy`. A kind that is not installed fails with `agent.not_installed`.
  Arguments after `--` go to the agent after its own. A kind whose `prompt.mode` is `none`
  receives `--prompt` at its first `idle` report, and a kind that reports no `idle` refuses
  such a prompt with `agent.prompt_invalid`; the held prompt is dropped if no `idle` comes
  within 120 s or the run ends first. The text of `--prompt` follows the same rules as
  `agent prompt`, control characters included.
- `agent prompt SESSION TEXT` types TEXT into the session's terminal, then the kind's submit
  key (`\r` unless the kind declares another). A kind that declares bracketed paste gets the
  text between paste markers; a plain kind gets each line break as Enter. The text is at most
  8 KiB and holds no control character except tab, line feed and carriage return: NUL, the
  escape character, DEL and the C1 controls are refused with `agent.prompt_invalid`, in both
  paste modes (decision 0058). An exited run fails with `agent.not_live`.
  The agent must report `idle` or `done` first, or the call fails with `agent.busy`; a kind
  with no status mechanism is always ready. `--force` types the text anyway, for example to
  answer a question. After a prompt is accepted, the next one is refused with `agent.busy` until
  the agent reports something (or `--force`), so two prompts never land on the same idle; a kind
  with no status mechanism is not affected, since nothing reports back for it. The receipt counts
  the bytes handed to the terminal; it does not say the agent read them, and the activity turns
  `working` only when the agent's hooks report it.
- `agent resume SESSION` starts a new run of an exited session in a new terminal of its
  workspace, with the kind's resume arguments and the agent's own ref, so the agent continues
  its conversation. The session keeps its id and its agent ref; its activity is absent until
  the new run reports. The arguments and environment of the first start are not kept. A live
  run fails with `agent.busy`; a kind with no resume form, or a session whose agent ref is not
  known yet, or whose last run has no recorded end, fails with `agent.resume_unavailable`; another
  writing session in the workspace
  fails with `agent.workspace_busy`.
- `SESSION` is the session id or a unique prefix of at least 8 characters, resolved by the
  service; a prefix shared by several sessions fails with `selector.ambiguous`.
- Execution is `live`, `unverifiable` or `exited` with how the run ended; activity is what
  the agent reports (`working`, `waiting`, `idle`, or `done` with its outcome) and where it
  came from, for example `done (success) from hook`, or `no status` before any report. A run
  that ends while `working` or `waiting` shows `done (interrupted) from process`. Stopping or
  restarting the service ends every run, and the sessions are listed as `exited` with reason
  `service stopped`.
- `agent status SESSION --until STATE` waits on the service's status stream until the session
  reaches `STATE` (`exited` means its run has ended), then prints the session and exits 0.
  With `--timeout SECS` it gives up after that long and fails with `agent.status_timeout`.
  `agent status [SESSION] --watch` prints the sessions, then one line per change, until it
  is interrupted; with `--json` each item is its own `{"ok":true,"result":...}` line.
- `agent status --watch` and `git watch` outlive their connection (decision 0040). When the
  connection drops or the service stops, they print a `link` line, `unverifiable` with the
  attempt and the pause, and connect again with backoff: 250 ms doubling to 30 s, with jitter,
  reset after 60 s connected. A reconnect never starts the service. Once back they print
  `live`, and the stream starts afresh: a new snapshot of the sessions, or `watching` again.
  Nothing seen during the gap is reported as ended. A refusal ends the watch non-zero with
  its reason, for example a revoked device (`auth.device_revoked`).
- `agent hook EVENT [PAYLOAD]` reads the session and token from `MASSORCA_AGENT_SESSION` and
  `MASSORCA_AGENT_TOKEN`, which the service sets in every run, and the payload from the
  argument or else from stdin when stdin is not a terminal (at most 64 KiB). It never starts
  the service. It always exits 0 within 5 s and prints nothing on success; a problem, such
  as a refused report (`agent.hook_refused`), goes to stderr only, so a status problem never
  breaks the agent. Its command line never fails either: a missing event is reported on
  stderr, words after the payload are ignored, and values may start with `-`.
- `agent attach` needs a terminal like `terminal attach`. It refuses a run that has exited
  and names the terminal whose last screen `terminal screen` can print.
- `agent accounts` (decision 0059) keeps several credential homes per kind, one of them
  active. A run uses the project's choice, else the active account, else the agent's usual
  home. `agent credentials` (0056) still works and acts on the active account; it is
  deprecated in favour of `agent accounts`. The first account of a kind made from an
  existing 0056 home is named `default`, state `migrated`, and is moved, not copied.
  A kind has accounts only when its declaration sets `accounts.login`; the four built-in
  kinds do not, until their real login commands are verified.
- `agent delegate --prompt TEXT` starts a child session of the `--from` session (decision
  0055). Inside an agent run `--from` defaults to the run's own session, and the run's
  `MASSORCA_AGENT_TOKEN` always goes with the call, whatever session `--from` names; a token
  that is not the parent run's fails with `agent.hook_refused`, so a child cannot delegate as
  its parent. The kind is the parent's unless `--kind` names another. A writing
  child gets a new workspace in the parent's project, named `--name` or `<parent
  workspace>-<n>` with the first free `n`, on a branch of that name based on the parent
  workspace's branch; it starts from that branch's last commit, so uncommitted changes in the
  parent are not carried over, and the result's `note` says so. A parent in a folder workspace
  fails with `agent.delegate_needs_git` unless `--read-only` is given. A `--read-only` child
  runs in the parent's workspace and does not hold its writing slot. A delegated session
  cannot delegate: `agent.depth_exceeded`.
- `agent tree SESSION` prints the root of the session's tree (the session, or its parent)
  and the root's children, oldest first, with their execution and activity. `--json` prints
  `{ root, children }` with each session's result text.
- `agent bring-back CHILD` needs a child whose run has exited, or whose last turn is `done`;
  otherwise it fails with `agent.busy`. Inside an agent run the run's token goes with the call
  and must be the child's parent's, so a child cannot bring itself back (`agent.hook_refused`). When the child has a workspace of its own, its branch
  is merged into the parent's worktree with `git merge --no-ff --no-edit`, unless
  `--no-merge`: the parent must not be `working` (`agent.parent_busy`), no other session may
  write in its workspace (`agent.workspace_busy`), and its worktree must be clean with no
  rebase or merge stopped in it (`agent.parent_dirty`). While the merge runs, a start in that
  workspace and any prompt to its sessions, even with `--force`, fail with `agent.busy`. A merge that stops on conflicts is aborted, the parent is
  left as it was, and the call fails with `agent.merge_conflict`, the conflicting paths in
  the error's `data.paths`. Unless `--no-prompt`, a live parent run then gets one prompt, by
  the `agent prompt` rules without `--force`, naming the child, the merge commit and the
  child's result on one line. A prompt the parent cannot take does not fail the call: the
  result says `prompt delivered: no` with a note. The child is marked brought back; a second
  bring-back fails with `agent.already_brought_back`. The child's workspace is kept, and
  nothing is ever pushed.
- `agent credentials import KIND --from DIR` copies only the files the kind declares
  (`credentials.files`) into `<profile>/agents/homes/<KIND>/`; from then on every run of the
  kind in the profile gets the kind's home variable pointing there (decision 0056). Every
  declared file is required and must be a regular file of at most 1 MiB: a missing name, a
  link, a directory or a larger file fails the import with `agent.credentials_invalid`, lists
  each name with its reason (`--json`: `error.data.files`), and copies nothing. A relative
  `DIR` is taken from the current directory. An existing home is replaced only with
  `--replace` (`agent.credentials_exist` otherwise); a failed import leaves it as it was. A
  kind that declares no files is refused with `agent.credentials_unsupported`. The copy does
  not move the login: refresh tokens are single use, so stop using the source login after
  importing. File contents are never printed or logged.
- `agent credentials show KIND` and `remove KIND` never follow a link: a home that is a link
  is refused with `agent.credentials_invalid` and must be removed by hand.
- `agent credentials import KIND --replace` and `remove KIND` are refused with `agent.busy`
  while a run of the kind is live; stop it first. An old home left aside by a replace that
  could not finish (`homes/.KIND.old-...`) refuses the next import with
  `agent.credentials_invalid` naming it, so the only copy of a login is never deleted.

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
