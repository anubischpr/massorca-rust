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
| `--json` | | Print one JSON document on stdout instead of text. |
| `--no-autostart` | `MASSORCA_NO_AUTOSTART` | Never start the service on its own. |

A profile name is 1 to 32 characters: a lowercase letter or digit first, then lowercase
letters, digits, `-` or `_`.

`MASSORCA_LOG` sets the service log filter in `tracing` directive syntax. The default is `info`.

## Commands

| Command | What it does |
|---|---|
| `service run [--log-stderr]` | Runs the service of the profile in this process until it is stopped. `--log-stderr` also writes log lines to stderr. |
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
| `agent kinds` | Lists the agent kinds: id, name, owner and the resolved program, or `no` / `invalid`. Problems follow the table. |
| `agent start <KIND> --workspace <WS> [--prompt TEXT] [--read-only] [--title TITLE] [-- ARGS...]` | Starts an agent in the workspace root, in a terminal of its own. Prints the session. |
| `agent list [--workspace WS]` | Lists agent sessions, newest first: execution, activity and title. |
| `agent status [SESSION] [--watch] [--until STATE] [--timeout SECS]` | Shows one session's execution and activity, or every session's. `--watch` prints every change; `--until` waits for `working`, `waiting`, `idle`, `done` or `exited`. |
| `agent stop <SESSION> [--force]` | Ends the session's run. The session stays listed as exited (`killed`). |
| `agent prompt <SESSION> <TEXT> [--force]` | Types TEXT into the session's terminal and submits it, once the agent is `idle` or `done`. Prints the bytes handed to the terminal. `--force` types it while the agent works or waits. |
| `agent resume <SESSION>` | Starts a new run of an exited session with the kind's resume arguments, in a new terminal. The session keeps its id and agent ref. |
| `agent forget <SESSION>` | Forgets a session whose run has ended, with its terminal and generated files. |
| `agent attach <SESSION>` | Interactive: `terminal attach` on the session's run. `Ctrl+]` detaches. |
| `agent hook <EVENT> [PAYLOAD]` | Called by an agent's hooks inside its run: reports the event to the service. Always exits 0. |
| `agent delegate [--from SESSION] --prompt TEXT [--kind K] [--name N] [--read-only]` | Starts a child session of SESSION with TEXT as its first prompt, in a new workspace of its own unless `--read-only`. Prints the child. |
| `agent tree <SESSION>` | Shows the session's delegation tree: its root, then the root's children. |
| `agent bring-back <CHILD> [--no-merge] [--no-prompt]` | Merges a finished child's branch into its parent's worktree with a merge commit, and tells the parent's run. |
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
  (owner `user`). The files are read again on every call. A kind is installed when one of
  its programs resolves: an absolute path that is a file, or a bare name on the service's
  `PATH` (on Windows also with the `PATHEXT` extensions). A file that does not parse, breaks
  a rule, or reuses a built-in id is listed with its problems and never started.
  `--json` prints the `agent.kinds` result.
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
- `agent hook EVENT [PAYLOAD]` reads the session and token from `MASSORCA_AGENT_SESSION` and
  `MASSORCA_AGENT_TOKEN`, which the service sets in every run, and the payload from the
  argument or else from stdin when stdin is not a terminal (at most 64 KiB). It never starts
  the service. It always exits 0 within 5 s and prints nothing on success; a problem, such
  as a refused report (`agent.hook_refused`), goes to stderr only, so a status problem never
  breaks the agent. Its command line never fails either: a missing event is reported on
  stderr, words after the payload are ignored, and values may start with `-`.
- `agent attach` needs a terminal like `terminal attach`. It refuses a run that has exited
  and names the terminal whose last screen `terminal screen` can print.
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

`service run` prints one line in text mode, `service running for profile <name> (pid <n>)`.
With `--json` it prints the start document on stdout, and the service log goes to the
profile's log files only unless `--log-stderr` is given.

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
| 2 | The command line is invalid, including an invalid profile name or log filter, or `terminal attach` without a terminal on stdin and stdout. |
| 3 | `service run`: another service already owns this profile. |
| 4 | The service is not running and could not be reached or started. |

## Logs and files

Each profile lives in `<home>/profiles/<name>/`. The service writes daily log files to
`logs/` as `service.<date>.log`, keeping the last seven days.

## Not yet available

Extension commands arrive in later milestones. Deferred from ADR 0008 (branch prefixes,
generated names, setup scripts, terminal and agent guards on removal, squash-merge detection,
remote hosts) are not available yet.
