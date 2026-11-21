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
| 2 | The command line is invalid, including an invalid profile name or log filter. |
| 3 | `service run`: another service already owns this profile. |
| 4 | The service is not running and could not be reached or started. |

## Logs and files

Each profile lives in `<home>/profiles/<name>/`. The service writes daily log files to
`logs/` as `service.<date>.log`, keeping the last seven days.

## Not yet available

Extension commands arrive in later milestones. Deferred from ADR 0008 (branch prefixes,
generated names, setup scripts, terminal and agent guards on removal, squash-merge detection,
remote hosts) are not available yet.
