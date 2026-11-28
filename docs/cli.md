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
Every log line is redacted before it is written: values under secret-looking keys (`password=`, `token=`,
`secret=`, `api_key`, `Authorization: Bearer ...` and similar), URL passwords, and tokens with a known secret
prefix (`sk-`, `ghp_`, a pairing offer) are written as `[redacted]`.

`--host` applies to every command that talks to a service. `service run`, `service status`,
`service stop`, `service restart`, `service unit`, `service log path`, `open`, `pair accept` and the `host` commands except
`host memory` and `host ports` work on this computer only: with `--host` they exit 2 with `cli.host_not_supported`.
`service unit` reads the `--home` flag but never `MASSORCA_HOME`, because its unit is for another
machine.

## Commands

| Command | What it does |
|---|---|
| `service run [--log-stderr] [--listen <ADDR>]... [--allow-all-interfaces]` | Runs the service of the profile in this process until it is stopped. `--listen` adds a network listener (also `MASSORCA_LISTEN`, comma separated). `--log-stderr` also writes log lines to stderr. Prints a readiness record once it accepts connections (see below). |
| `service unit [--listen <ADDR>]... [--exe <PATH>]` | Prints the systemd user unit that runs `service run` on a Linux server (decision 0045). The unit's `MASSORCA_HOME` is the global `--home`. Defaults: `tailscale`, `%h/.massorca/bin/massorca`, `%h/.massorca/home`. |
| `service log path` | Prints the profile's logs folder, then the newest service and desktop log file names (`none` when there is none). `--json`: `{dir, service, desktop}`, each file name or `null`. Never starts the service. Logs are daily `service.<date>.log` and `desktop.<date>.log` files, seven kept each, never sent anywhere. |
| `service status` | Shows whether the service runs. Never starts it. |
| `service info` | Shows the service's identity (profile, pid, version, protocol, capabilities) and its host's git: `present <version>`, `missing`, `too_old <version> (requires 2.25.0)` or `unknown (<reason>)`. Starts the service when it is not running. A service older than the git field shows `not reported`; update that service. With `--json`, the same state is `git` (`state`, `version`, `required` or `reason`). |
| `service tools` | Shows the developer tools on the service's host, one line each: `git`, `gh`, `glab` and the shells (`pwsh`, `powershell` on Windows; `bash`, `zsh`, `sh` elsewhere). Each line is `<id>: <state> [version] [(path)]`, with state `present`, `missing`, `too_old`, `unknown` or `other`. A forge CLI's version is read with `--version`; a shell is found but never started, so its version is not read. WSL is never probed. Detection runs on the service's host, so `--host` lists that host's tools. The answer is kept for the service run; `--refresh` detects again. With `--json`, the list is `tools`, each with `id`, `kind` (`vcs`, `forge`, `shell`), `state`, `version` and `path` (`null` when not reported). |
| `service stop` | Stops the service. Succeeds when it is not running. |
| `service restart` | Stops the service, then starts it again. |
| `open` | Starts the MassOrca desktop app on the profile and returns at once, without waiting for its window. The app is `massorca-desktop` beside `massorca`, or the executable `MASSORCA_DESKTOP_EXECUTABLE` names. It gets the profile, and `--no-autostart` when the CLI has it. With `--json`, prints the app's `pid`, `executable` and `profile`. Refused with `--host`. |
| `cli install` | Puts `massorca` on your PATH for this user only: a link in `~/.local/bin` on Unix, or a command file in a folder added to your Windows user PATH. Running it again changes nothing. Refused with `--host`. See [The command on PATH](#the-command-on-path). |
| `cli uninstall` | Removes the link or command file that `cli install` made, and its PATH entry once its folder is empty. Leaves any file massorca did not make. Refused with `--host`. |
| `cli status` | Shows whether `massorca` is on your PATH and which binary it runs. Changes nothing. Refused with `--host`. |
| `config get <KEY> [--owner OWNER]` | Shows one setting and whether it still has its default value. |
| `config set <KEY> <VALUE> [--owner OWNER]` | Changes one setting. The value is parsed as JSON when it is valid JSON, otherwise taken as text. |
| `config list` | Lists every setting. |
| `project add <PATH> [--name NAME] [--folder \| --git] [--workspaces-root DIR] [--icon ICON]` | Registers a git repository (its toplevel) or a folder. Without `--folder` or `--git`, a directory inside a repository is a git project. When neither is given and the directory is not inside a repository, a warning on stderr says it was added as a plain folder (text mode only). `--icon ICON` sets the icon: `emoji:<emoji>`, `lucide:<name>` or `image:<PNG file>`, and the CLI sends the file as an upload. Without it, the icon is detected from the repository's files when there is one: the first PNG or WebP at a conventional location, else the icon an `index.html` or a JSX route root (`app/root.tsx`) declares, else the `icon` of `package.json` (a PNG or WebP path inside the repository, or a data URI). Every file read stays inside the repository (decisions 0091 and 0180). |
| `project list` | Lists every project, ordered by name, with its icon in an `ICON` column and the owner of its remote in an `OWNER` column (`-` when there is none). |
| `project show <PROJECT>` | Shows one project, with its icon when it has one and its remote owner as `owner:` when it has one. The owner is the first path segment group of the primary network remote (`origin` when it is a network remote, else the first network remote, as `git remotes` marks it), such as `acme` for `git@github.com:acme/widgets.git` or `group/sub` for a nested GitLab group; it is absent for a folder project or when no remote is a network remote. It is read from git on every call, so a changed remote shows at once. `PROJECT` is its id, name or path. |
| `project update <PROJECT> (--icon ICON \| --clear-icon)` | Sets the project's icon with `--icon`, which takes the forms of `project add --icon`, or removes it with `--clear-icon`, so the project shows the default icon. A Lucide icon takes a colour as `lucide:<name>:<colour>`, where `<colour>` is `usage-1` to `usage-6` or `neutral`; without one it is neutral. Prints the icon. |
| `project scripts <PROJECT> [--workspace WORKSPACE]` | Shows the scripts as `project trust-scripts` without `--hash` does, and changes nothing. |
| `project trust-scripts <PROJECT> [--workspace WORKSPACE] [--hash HASH]...` | Shows the `setup` and `archive` scripts of the project's `orca.yaml`, or of `WORKSPACE`'s copy (its `setup` only: removal reads `archive` from the project's), each with its SHA-256 and whether it is `trusted`, `not trusted` or `changed since it was trusted`, and the command that trusts it. `--hash` trusts the shown script with that hash; repeat it for both. A hash no shown script has exits 2 with `usage.unknown_script_hash`, and one that no longer matches the file is refused with `script.changed`. Trusting needs host-admin. A script runs only while its exact content is trusted (decision 0095). |
| `project inspect <PATH>` | Shows what `project add` without `--folder` or `--git` would register: a git repository at its toplevel, or a plain folder, and the id of a project already registered there. Registers nothing. |
| `project init-git <PROJECT>` | Turns a folder project into a git project: `git init` and an empty first commit (`Initial commit`) in its folder, with no file committed. The first commit uses your git identity, or a fixed `MassOrca` identity when none is configured. The project then gets the workspaces root a new repository of its name gets. Its folder workspaces stay as they are. Refused with `project.already_git` when the folder has a `.git` entry or is inside a repository, or the project is already git; with `git.failed` when git refuses the init or the commit, and then nothing is left behind. Prints the project (decision 0115). |
| `project rm <PROJECT>` | Unregisters a project that has no workspaces. Its files stay. |
| `workspace create <PROJECT> [NAME] [--in-place] [--branch B] [--base REF] [--existing-branch] [--refresh-base] [--path DIR] [--request-id ID] [--no-setup] [--unique] [--name-generated]` | Creates a worktree on a new branch in a git project, or registers a folder in a folder project. Alias `ws`. `--name-generated` marks `NAME` as derived rather than typed, so `workspace rename --from-generated` may replace it; it is ignored with `--branch` or `--existing-branch` and is meant for scripts and tests (capability `workspace-rename/1`). When the new worktree's `orca.yaml` has `scripts.setup`, it runs in a service terminal titled `Setup` and the output prints `setup running in terminal <ID>` and `watch it: massorca terminal attach <ID>`; `--json` carries it as `setup`. `--no-setup` skips it (decision 0086). A script whose content is not trusted does not run: the output prints `setup skipped: <reason>` and its hash, and `--json` carries `setup.outcome` `untrusted` (decision 0095). A taken name, branch (local or remote-tracking, any case) or folder is refused unless `--unique`, which uses the first free of `NAME`, `NAME-2` .. `NAME-100` (cut to stay within 64 characters) and prints the name used; it does not combine with `--branch`, `--existing-branch` or `--path` (capability `workspace-unique-name/1`). `--in-place` registers the project's own checkout as its workspace (decision 0112, capability `workspace-in-place/1`): no worktree or branch is made, `NAME` defaults to the project's name, the branch is read from the checkout on each call, no setup script runs, and a repeat call returns the same workspace. It does not combine with `--branch`, `--base`, `--existing-branch`, `--refresh-base`, `--path`, `--unique` or `--name-generated`. |
| `workspace list [--project PROJECT]` | Lists every workspace, or those of one project, with git's `locked` and `prunable` state. Each other linked worktree of a project that no workspace records, under its workspaces root or elsewhere, is listed as `unregistered` with the same state; nothing is removed for them. |
| `workspace show <WORKSPACE>` | Shows one workspace. `WORKSPACE` is its id, name or path.
| `workspace current [--path DIR]` | Shows the workspace that contains the current directory, or `DIR`, the deepest workspace whose path is that directory or an ancestor of it (worktree or folder, from the service's records; a subdirectory counts). Outside every workspace it exits 1 with `workspace.not_found`. With `--host`, `--path` is required and sent as typed: the service's own absolute path. Prints the same fields as `workspace show`. |
| `workspace rm <WORKSPACE> [--force] [--discard-ignored] [--forget-orphan] [--with-children] [--allow-failed-archive]` | Removes a workspace. `--force` discards uncommitted and untracked changes; `--discard-ignored` deletes ignored files; `--forget-orphan` forgets a directory git does not recognise and keeps it; `--with-children` first removes the workspaces of delegated children; `--allow-failed-archive` removes the workspace even when its archive script fails. A worktree's trusted `archive` script runs first. |
| `workspace update <WORKSPACE> [--name NAME] [--comment TEXT]` | Renames a workspace or sets its comment; `--comment ""` clears it. At least one of the two is required. A rename changes only the name: the directory and the branch keep theirs. Prints the workspace. |
| `workspace rename <WORKSPACE> <NEW_NAME> --from-generated [--expected-branch B]` | Renames a workspace whose name the service chose (a blank-name create, or one created with `--name-generated`, and never one on a named or existing branch; `workspace show` prints `name generated: yes`) and, for a worktree, its branch, under the branch prefix. A taken name, local or remote-tracking branch (any case, the workspace's own excluded) moves on to `NEW_NAME-2` .. `NEW_NAME-100`. The folder is never moved. Refused with `workspace.name_not_generated` for a typed name, `workspace.branch_changed` when the worktree is not on the recorded branch or on `--expected-branch`, `git.detached_head` on a detached HEAD, and `workspace.branch_published` when the branch has an upstream or a remote copy; these checks run even when the name does not change. `--from-generated` is required (capability `workspace-rename/1`, decision 0111). |
| `workspace check <WORKSPACE>` | Reports whether the workspace's folder exists and is readable and, for a worktree, whether git still lists it. Prints `NAME: ok` or the first failure, with the path. A failed check still exits 0; `--json` carries `ok` and `reason`. Changes nothing. |
| `git branches [-w WORKSPACE \| --project PROJECT]` | Lists the local branches, the current one first. Without either flag it reads the workspace of the current directory. `--project` lists the branches of the project's own checkout, with no workspace (capability `git-project-branches/1`); a folder project is refused with `git.not_a_repository`. |
| `git remotes [-w WORKSPACE \| --project PROJECT]` | Lists each network remote of a workspace's repository, or of a project's checkout: its name, owner and repository, host and provider (`github`, `gitlab`, `bitbucket`, `azure_devops`, `gitea` or `unknown`), with `primary` on `origin` or else the first one, and `fork of OWNER/REPO` when `origin` and `upstream` name the same repository under other owners. Local-path remotes are left out. `--json` also gives `web_url` when the host's layout is known. Provider actions such as pull requests and CI are not in this command yet (decisions 0010 and 0114). |
| `git patch <PATH> -w WORKSPACE [--staged \| --head \| --from REV [--to REV]] [--original-path PATH] [--max-bytes N]` | Shows one file's patch for the chosen comparison; by default the unstaged changes. For a submodule it shows the pointer instead of a text patch: the commits the parent records on each side as `Subproject commit OLD..NEW`, and whether the submodule's working tree is dirty when that is known (not for `--staged` or a range). |
| `git show-file <PATH> -w WORKSPACE [--index \| --rev REV] [--out FILE]` | Shows one file's bytes: the working tree file, or with `--index` its index entry, or with `--rev REV` the file at that revision. Prints its size and, for a previewable image, its type, such as `image/png`; a file over 4 MiB, or a side with no file, is said so. `--out FILE` writes the decoded bytes, and refuses with `git.no_bytes` when there are none (decision 0103). |
| `terminal create <WORKSPACE> [--shell PROGRAM [--arg ARG]...] [--cwd DIR] [--cols N --rows N] [--env KEY=VALUE]... [--title TITLE]` | Starts the default shell, or `--shell` program, in a workspace. Prints the terminal. |
| `terminal list [--workspace WORKSPACE]` | Lists live and exited terminals, or those of one workspace. |
| `terminal show <TERMINAL>` | Shows one terminal: id, state, shell, cwd, size and, once exited, how it ended. An agent run's terminal also shows `git credential guard: on`: git inside it never waits for a credential prompt. |
| `terminal send <TERMINAL> <TEXT>... [--no-enter]` | Types the words joined by spaces, then Enter unless `--no-enter`. |
| `terminal screen <TERMINAL> [--scrollback]` | Prints the visible screen as text. `--scrollback` prints the history above it first. |
| `terminal attach <TERMINAL>` | Interactive: shows the terminal and types into it. `Ctrl+]` detaches. |
| `terminal resize <TERMINAL> --cols N --rows N` | Sets the terminal's size. |
| `terminal kill <TERMINAL> [--force]` | Ends the terminal's processes. The terminal stays listed as exited until closed. |
| `terminal close <TERMINAL>` | Ends the terminal's processes if needed and forgets the terminal. |
| `terminal wait <TERMINAL> [--for exit] [--timeout SECS]` | Waits until the terminal's process exits, up to `SECS` (default 300), then prints its exit code. Exits 0 once it has exited, whatever the code. Exits 1 with `terminal.wait_timeout` or `terminal.gone` (the terminal was closed while waiting), and 4 with `terminal.unverifiable` when the service stops answering. `--for` takes only `exit`. |
| `pair offer [--ttl <SECS>] [--standard] [--address <HOST[:PORT]>]... [--no-qr]` | Makes a pairing offer for a new device of the service. It is valid 10 minutes by default (1 to 3600 seconds) and works once. Prints the offer alone on the first line. The new device gets full access, every scope: a paired device is its owner. Advanced: `--standard` makes a reduced device, with `read` and `workspace` only. `--admin` is still accepted and changes nothing. `--address` (repeatable or comma-separated, at most 8) lists exactly these addresses in the offer instead of the bound listeners, for a device behind NAT, in a container or an emulator; see Hosts and pairing. Text output ends with the offer as a QR code for a phone to scan (light modules drawn as blocks, for a dark terminal); `--no-qr` leaves it out. A service that predates it is refused with `service.capability_missing`; an address no device can dial with `pairing.invalid_address`. |
| `pair cancel <OFFER-ID>\|--all` | Retires a pending offer before it expires, so a code that leaked no longer pairs. `pair offer` prints the offer id after the offer (`offer_id` in `--json`); `--all` retires every pending offer and reports how many. An offer that is used, expired or unknown is `pairing.offer_not_found`. A service that predates it is refused with `service.capability_missing`. |
| `pair accept <OFFER\|-> [--name <HOST>] [--device-name <NAME>]` | Pairs this computer with the server of an offer and stores it as a host. `-` reads the offer from stdin, which keeps it out of shell history. The host name defaults to the server's name in the offer; the device name to this computer's host name. |
| `device list` | Lists the paired devices with their scopes, pairing and last-seen times, and state. Needs `device-admin`. |
| `device revoke <DEVICE>` | Revokes a device by id, or by name when that is unique. Its connections close at once and it is refused afterwards. |
| `device grant <DEVICE>` | Gives a paired device full access, every scope, from its next connection, so a reduced device does not have to pair again. Needs host-admin. A revoked device is refused with `device.revoked`. |
| `device forget <DEVICE>` | Removes a revoked device from `device list`. Takes the id, or a name that exactly one revoked device has. Needs host-admin. An active device is refused with `device.not_revoked`. A forgotten device stays refused: its key is no longer known. |
| `device whoami` | Shows the principal this command runs as, its scopes and its device. |
| `server status` | Shows the listeners, the server pin, the active devices and the pending offers. Without `--host`, also this computer's Tailscale state. |
| `host list` | Lists the hosts this client paired with. Makes no connection. |
| `host show <NAME>` | Shows one host: addresses, pin, device, scopes and pairing time. |
| `host remove <NAME> [--keep-device] [--uninstall --ssh <TARGET>]` | Forgets a host. Unless `--keep-device`, revokes this device on the server when the host answers. `--uninstall` also stops and removes the service over SSH; if that fails the host is kept, so the command can be run again. |
| `host status [<NAME>]` | Connects to one host, or to every host, within 10 seconds and reports `connected`, `unreachable`, `refused`, `identity_mismatch` or `error`. A single named host that does not answer exits 4. |
| `host status --watch [<NAME>]` | Keeps a connection open to one host, or to every host, and prints a line each time its state changes, until interrupted: `connecting`, `connected` (with the server version), `unreachable` (with why, the next attempt and its delay), `refused`, `identity_mismatch` or `error`. A lost host is retried with the network backoff, 250 ms doubling to 30 s; unreachable says nothing about the work on it. A refusal, a changed server key or any other failure a retry would not fix is reported once and not retried; once every watched host has ended so, the watch exits with that failure and its exit code (the first host by name when there are several), while any host that can still change keeps it running. With `--json`, each line is one `{"ok":true,"result":{...}}` document with `name`, `state`, `server_version`, `detail`, `attempt` and `retry_in_ms` as they apply. Without hosts it prints `no hosts` and ends. |
| `host discover` | Lists the online Tailscale peers and whether each answers on port 7347. Without a tailnet it says why and lists nothing. |
| `host self-test [<NAME>]` | Runs the checks of one paired host in order and prints one line per check: `record` (the stored record and pin), `address` (the addresses resolve), `port` (a TCP connection is accepted), `identity` (the secure connection's server key matches the pin paired with; a different key shows both keys and is not retried), `protocol` (the handshake agrees on a protocol), `paired` (the server still lists this device), `service` (`service.info`: version and uptime) and `latency` (the round trip of that call). A check is `pass`, `fail` with why and the fix, or `skipped` and names the failed check it waited for; the first failure skips the rest. Exits 0 only when every check passes, otherwise 1 with `host.self_test_failed`. With `--json` the one document holds every check: on success under `result`, on failure under `error.data` (`name`, `passed` and `checks[]` with `name`, `state`, `detail` and `fix`). Without `NAME` it checks this computer's own service instead: `endpoint`, `connect` (a local connection that never starts the service), `protocol`, `service`, `latency`, and `network` only when the service listens on the network. When a check fails on a host that `host add-ssh` installed, a `logs:` line prints the `ssh` command that reads its service log (`journalctl --user -u massorca.service -n 100 --no-pager`, through the same target and `--ssh-arg` values). The command prints the line only; it never runs SSH. |
| `host memory` | Shows the memory of the service and of each live terminal's processes (agent runs included), read on the host that runs them: the total, the service, then one row per terminal with its workspace, agent session, process count and memory. Working set on Windows, resident set on Linux and macOS (decision 0081). Sizes use binary units: `N KB` under 1 MiB, `x.y MB` under 1 GiB, `x.yz GB` above. A size that rounds up to the next unit is shown in it (`1.0 MB`, not `1024 KB`). Processes that could not be read are counted, not added. A terminal counts its session on Linux and its parent tree on macOS, so a process that started its own session (Linux) or was reparented away (macOS) is not counted (decision 0081, D6). A terminal whose processes cannot be listed or read shows `-` and `unverifiable` instead of a size and is not in the total. A terminal with some processes read and some not shows its size with `(partial)`: that size covers only the processes read, the total counts it so, and a line says how many terminals are partial. Works with `--host`. A service that cannot be reached or is lost mid-call (`service.unavailable`, `service.disconnected`, `host.unreachable`, `client.io`) prints `unverifiable` (`memory.unverifiable`) and exits 4, never a size; other failures, such as `host.identity_mismatch` or a refused handshake, keep their own code. |
| `host ports` | Shows the TCP ports that the processes of each live terminal listen on, read on the host that runs them (decision 0081): one row per listening socket with its port, bind address, process (name and pid), workspace and agent session. A terminal whose ports cannot be listed or read shows `unverifiable` and lists no port; a terminal with some processes unread is `partial`, and a line counts such terminals, since its list may miss ports. A host whose socket table cannot be read exits 4 with `ports.unverifiable`, never an empty list. Works with `--host`. A service that cannot be reached or is lost mid-call prints `unverifiable` (`ports.unverifiable`) and exits 4; other failures keep their own code. macOS reads no socket table yet, so `host ports` is unverifiable there. |
| `usage [--kind <KIND>] [--account <ID>] [--refresh]` | Shows each agent account's usage and limits, read by the service on the host that runs the agents from the vendor CLI's own logs in the account's folder; nothing is fetched over the network (decision 0154). Off until `settings set usage.enabled true`: while off it prints how to turn it on, and `--json` answers `{"collection": "off", "accounts": []}`. When on, one row per Claude or Codex account: kind, label, state (`pending`, `ok`, `no data`, `error: ...`), the session and weekly windows as a percent, a token count or `limit reached`, with the reset time when known, and when the service last read it. The answer comes from the service's cache, which it refreshes about every minute and backs off on errors; `--refresh` reads the selected accounts again first, at most once every 5 seconds per account. Claude Code records no limit, so its windows show tokens unless a reply said the limit was reached. Works with `--host`, which reads the paired host's own accounts. |
| `host add-ssh <SSH-TARGET> --binary <PATH> [--name <HOST>] [--listen <ADDR>]... [--ssh-arg <ARG>]... [--device-name <NAME>]` | Installs or upgrades massorca on a Linux x86_64 or aarch64 host over SSH, as a systemd user unit, then pairs with it. Progress goes to stderr. A host that already exists is upgraded and not paired again. `--ssh-arg` takes one option per flag: `--ssh-arg -p --ssh-arg 2222`. |
| `agent kinds` | Lists the agent kinds: id, name, owner, package state (`installed`), whether a program resolves (`detected`: `yes`, `no` or `invalid`) and the resolved path. Then one `modes:` line per kind that declares permission modes: each mode's id and label, marked `default` (the vendor CLI's own default), `bypass` (lifts the vendor's prompts or sandbox), `live` (switches in a running session) and `may fall back` (the vendor may start in another mode without failing). Problems follow. Then one `models:` line and one `efforts:` line for each kind that declares model choices or effort levels, listing the ids with the default marked (decision 0096). |
| `agent models <KIND> [--account <ACCOUNT>] [--refresh]` | Lists the models the kind's picker offers for an account: each model's id, label, `default` mark, `efforts` (the levels it takes, or `none`) and whether it is `declared` or `listed`. The declared models come first, then the models the account's vendor CLI lists. Without `--account`, the active account is listed, or the vendor's usual home when the kind has none. A failed listing prints `listing failed:` with its message and still lists the declared models. A listing is cached for 10 minutes; `--refresh` asks the vendor CLI again. Only `antigravity` and `claude` list models (decision 0166). `--json` prints the `agent.models` result. |
| `agent start <KIND> [--workspace <WS>] [--prompt TEXT] [--read-only] [--title TITLE] [--mode MODE] [--model MODEL] [--effort EFFORT] [--fast] [--chat] [-- ARGS...]` | Starts an agent in the workspace root, in a terminal of its own; without `--workspace`, in a folder of its own in the conversations folder (see the notes below). With `--chat` the run is a structured chat instead (`agent.chat.start`, see the notes below): no terminal, and no arguments after `--`. `--mode` names one of the kind's permission modes (`agent kinds`) and passes its flag; without it no mode flag is passed and the agent starts in its vendor's own default (`mode: the CLI's default`). An unknown mode fails with `agent.mode_unknown`. Prints the session, with `mode: MODE`, followed by `(requested)` when it is not confirmed: arguments after `--`, a mode that may fall back, or a typed switch. `--fast` asks for fast mode (decision 0165): the kind passes its fast settings, and the start is refused with `agent.option_unsupported` unless the kind declares fast mode and the model (the default one, without `--model`) takes it. The session then shows `fast: requested` until something confirms the vendor took it; nothing does yet. |
| `agent list [--workspace WS] [--archived]` | Lists agent sessions, newest first: execution, activity and title. Hidden (archived) sessions are listed only with `--archived`. |
| `agent status [SESSION] [--watch] [--until STATE] [--timeout SECS]` | Shows one session's execution and activity, or every session's. `--watch` prints every change; `--until` waits for `working`, `waiting`, `idle`, `done` or `exited`. |
| `agent stop <SESSION> [--force]` | Ends the session's run, and the runs of the sessions delegated from it. The sessions stay listed as exited (`killed`). |
| `agent prompt <SESSION> <TEXT> [--force]` | Types TEXT into the session's terminal and submits it, once the agent is `idle` or `done`. Prints the bytes handed to the terminal. `--force` types it while the agent works or waits. For a chat session it sends TEXT as a message (receipt `sent_to_chat`); `--force` hands it to the agent while a turn runs, which runs it next. |
| `agent queue add <SESSION> <TEXT>` | Queues TEXT behind the session's busy agent and prints the queued prompt (`waiting`, with its reason). A kind that reports no activity is refused with `agent.queue_unavailable`; past 100 queued prompts with `agent.queue_full`. |
| `agent queue list <SESSION>` | Lists the session's queued prompts, head first, with their states and reasons. |
| `agent queue edit <ID> <TEXT>` | Replaces the text of a waiting or failed queued prompt. A prompt being sent is refused with `agent.busy`. |
| `agent queue remove <ID>` | Removes a waiting or failed queued prompt. |
| `agent queue send-now <ID>` | Types the queued prompt at once, with force, ahead of the queue; it leaves the queue. Refused with `agent.busy` while the agent asks the person something. |
| `agent queue resume <SESSION>` | Lifts the session's queue pauses (after a stop or a service restart) and types the head once the agent is ready. Prints the queue. |
| `agent resume <SESSION> [--mode MODE]` | Starts a new run of an exited session with the kind's resume arguments, in a new terminal. The session keeps its id and agent ref. The run is in MODE; without it, in the mode `agent mode` set for this resume unless that is a bypass mode (a pending bypass is used only with `--mode`), else the last chosen mode unless it was a bypass mode, else the vendor's own default. |
| `agent set-options <SESSION> [--model MODEL] [--effort EFFORT] [--fast or --no-fast]` | Changes the session's model, effort or fast mode (decision 0171). A model, or the fast mode toggle (`/fast`), that the kind declares a live command for is typed into a live run and prints `switched in the running session`; the fast toggle is typed only when the run is not already in the state asked for. Every other change prints `applies at the next resume`, and the next `agent resume` launches with it (the kind's default otherwise, decision 0094). An option already at its value is left out, and when nothing changes the text says so. A refused value (`agent.option_unsupported`, `request.invalid_params`) changes nothing; the values are checked as `agent start` checks them. The chosen options are kept in memory with the service. `--json` prints the `agent.set_options` result, whose `applied` names each changed option's route: `live` or `next_resume`. |
| `agent fork <SESSION> [--prompt TEXT] [--mode MODE] [--title TITLE]` | Starts a new session from a copy of an exited session's conversation, with the kind's fork arguments, in the same workspace (decision 0168). The new session keeps the source's title unless `--title` is given, and the model and effort its kind still takes (a stored one it no longer takes is left out and noted), and shows `forked from: SESSION`. Fails with `agent.busy` while the source runs and `agent.fork_unavailable` when its kind declares no fork form or its agent ref is not known. |
| `agent mode <SESSION> <MODE> [--force]` | Switches the session's permission mode. A kind that reports no activity cannot say whether its agent is mid-prompt, so a live switch into it needs `--force`; without it the switch fails with `agent.mode_unavailable`. Where the kind declares a live switch and the run is live, types it into the run once the agent is ready and prints `switched to MODE in the running session`; no hook confirms it, so the mode shows as requested. Otherwise prints `MODE applies at the next resume`, shows it as `next resume mode: MODE`, and leaves the running mode as it is. Fails with `agent.mode_unknown` for a mode the kind does not list, `agent.mode_unavailable` for a mode a resume could not apply, and `agent.resume_unavailable` when the session can neither switch now nor resume. |
| `agent forget <SESSION> [--remove-workspace [--force] [--discard-ignored] [--allow-failed-archive]]` | Forgets a session whose run has ended, with its terminal, generated files and archived output. `--remove-workspace` also removes its workspace as `workspace rm` would (the worktree and the branch it created; a folder workspace keeps its files), only when no other session uses it (`agent.busy` names them) and no team, task or running terminal does (`workspace.busy`), checked before anything is deleted and again as the removal begins; `--force` discards uncommitted and untracked changes, which refuse with `workspace.dirty` otherwise; `--discard-ignored` deletes ignored files such as build output, which refuse with `workspace.ignored_files` otherwise; the worktree's trusted `archive` script runs before the removal, as with `workspace rm`, and a failure is reported as `workspace_error` (`workspace.archive_failed`) after the session is forgotten, with the workspace kept `ready`; `--allow-failed-archive` removes the workspace anyway. |
| `agent forget <SESSION> --preview` | Prints what forgetting the session with its workspace would meet: whether it can be forgotten, the workspace, whether it is removable and why not, its uncommitted changes, untracked and ignored files, and the other sessions, teams, tasks and running terminals that use it. Changes nothing. |
| `agent rename <SESSION> <TITLE>` | Sets a session's title, live or exited. The title is trimmed and must be 1 to 120 characters without control characters (`request.invalid_params` otherwise). |
| `agent hide <SESSION> [--off]` | Hides (archives) an exited session from `agent list`, keeping its record, terminal output and workspace; `--off` shows it again. Refused with `agent.busy` while the run is live. Shown as `archived`. |
| `agent release <SESSION>` | Archives the newest 256 KiB of an exited session's terminal output, frees its terminal and keeps the record, marked released. Refused with `agent.busy` while the run is live. Clears the retain. A run whose terminal was already evicted, or that ended before a service restart, has no output left and archives an empty text. |
| `agent retain <SESSION> [--off]` | Keeps the session's exited terminal from being evicted, durably; `--off` clears it. Listed as `retained`. |
| `agent archive <SESSION>` | Prints the archived output of a released session. Withheld for a run that holds vault secrets, unless the caller is host-admin. |
| `agent attach <SESSION>` | Interactive: `terminal attach` on the session's run. `Ctrl+]` detaches. |
| `agent hook <EVENT> [PAYLOAD]` | Called by an agent's hooks inside its run: reports the event to the service. Always exits 0. |
| `agent delegate [--from SESSION] --prompt TEXT [--kind K] [--name N] [--title T] [--read-only] [--same-workspace] [--mode MODE]` | Starts a child session of SESSION with TEXT as its first prompt, in a new workspace of its own unless `--read-only` or `--same-workspace`, or in the parent's folder when that is a plain folder workspace. While a writing child works in the parent's own workspace, the parent waits. When the child's turn or run ends, the parent's queue gets a one-line notice. The child's title is T, or the first line of TEXT (at most 80 characters). The child runs in MODE, or its vendor's own default: never in its parent's mode. Inside an agent run (where the run's token goes with the call) a bypass MODE is refused with `agent.mode_unavailable`: a person chooses one with `agent start`. Prints the child. |
| `agent tree <SESSION>` | Shows the session's delegation tree: its root, then the root's children. |
| `agent bring-back <CHILD> [--no-merge] [--no-prompt]` | Merges a finished child's branch into its parent's worktree with a merge commit, and tells the parent's run. |
| `agent credentials import <KIND> --from <DIR> [--replace]` | Copies the kind's declared login files from `DIR` into the profile's home for the kind. Prints file names and sizes only, and says to stop using the source login. |
| `agent credentials show <KIND>` | Shows whether the profile has a home for the kind, and which declared files are in it, with sizes. |
| `agent credentials remove <KIND>` | Deletes the profile's home for the kind; its runs use the agent's usual home again. |
| `extension list` | Lists installed extensions: id, version, state (`enabled` or `disabled`). `ext` is an alias of `extension`. |
| `extension catalogue` | Lists the built-in catalogue, with each entry's state (`available`, `enabled` or `disabled`). |
| `extension show <ID>` | Shows one extension, installed or from the catalogue: the item page data, its risk and, for agents, whether the CLI is detected and what the install ran. |
| `extension inspect <SOURCE>` | Shows the install review for `catalogue:<id>`, `dir:<path>` or `file:<path>` without changing anything, ending with its `consent` fingerprint. A `dir:` or `file:` source reads a host path, so a paired device needs host administration for it. |
| `extension install <SOURCE> [--sha256 HEX] [--yes \| --consent FINGERPRINT] [--yes-run-installer]` | Installs a package. Without `--yes` or `--consent` it prints the review and exits 1 with `extension.consent_required`. `--yes` accepts the review this run prints. `--consent` accepts exactly the review an earlier run printed, by its fingerprint, and fails with `extension.consent_mismatch` if the package or what it would run changed since. A package from a folder or archive that declares a vendor command is refused with `extension.vendor_command_refused`; only the built-in catalogue names one. A package whose agent CLI is missing also needs `--yes-run-installer`, which runs the vendor's official command shown in the review; the review says it installs the current release, unpinned. In an agent's run (`MASSORCA_AGENT_SESSION` or `MASSORCA_AGENT_TOKEN` set) `--yes-run-installer` is refused with `extension.needs_person`: a person runs the command. `file:` needs `--sha256`. Never prompts. |
| `extension update <ID> [--sha256 HEX] [--yes \| --consent FINGERPRINT]` | Brings an installed package up to date in place, from the source it was installed from. The source is read and reviewed again. Without `--yes` or `--consent` it prints the new review and exits 1 with `extension.consent_required`; `--consent` accepts exactly that review and fails with `extension.consent_mismatch` if the package changed since. A package whose files are unchanged is reported as already up to date and nothing is written. The enabled state, CLI registrations and install time are kept; no vendor command runs. `file:` needs `--sha256`. In an agent's run it is refused with `extension.needs_person`. An older service that does not know the method is reported as too old to update. Never prompts. |
| `extension enable <ID>` / `extension disable <ID>` | Registers or unregisters the package's contributions. Files and data stay. While an install or removal of the package runs, either is refused with `extension.busy`. |
| `extension remove <ID> [--delete-agent-homes] [--uninstall-cli] [--yes | --uninstall-consent FINGERPRINT]` | Removes the package and its data. `--delete-agent-homes` also deletes its agents' accounts: for each agent kind the package provided and no other installed package provides, every account, its files under the profile (leftovers of an interrupted import included), and the kind's old single credential home (decision 0098). Nothing is signed out at the vendor, so a login a MassOrca sign-in made stays valid there. A busy account, a built-in agent kind named by a package outside `massorca.*`, and the accounts of a package whose files cannot be read are left, and listed in the result's failures. `--uninstall-cli` runs the vendor's uninstall command, only for a CLI this package installed, and only after confirmation: without `--yes` or the fingerprint it changes nothing and prints the command and its fingerprint (`extension.consent_required`). In an agent's run it is refused with `extension.needs_person`. |
| `agent accounts list <KIND>` | Lists the kind's accounts: id, label, state, identity, whether active, and the projects that chose each. |
| `agent accounts add <KIND> [--label L] [--no-attach]` | Signs a new account in: runs the kind's login in a service-owned terminal, prints the account and the terminal, then attaches to it unless `--no-attach`. Refused with `--json` unless `--no-attach`. Built-in sign-in is declared for `claude` only (`claude auth login`); a kind without a sign-in is refused with `agent.credentials_unsupported`, which says to sign in from the vendor's CLI, then import the login. |
| `agent accounts sign-in <KIND> <ACCOUNT> [--no-attach]` | Signs an existing account in again: runs the kind's login in its own home, prints the account and the terminal, then attaches unless `--no-attach`. The account is kept whatever the login ends with (an expired one stays listed, as `expired`, and is not deleted or signed out). Refused with `--json` unless `--no-attach`. Refused with `agent.account_not_found` for an unknown account or one of another kind, and with `agent.busy` while the account is signing in or a live run uses it. |
| `agent accounts import <KIND> [--from <DIR>] [--label L]` | Copies the kind's declared login files into a new account, then checks it. Without `--from`, each file is copied from where the kind's CLI keeps it (decision 0097): the command prints `copying <file> from <path>` for each file it will copy, and `skipping <file>` for a missing optional one, then imports; when a required file is missing it copies nothing and fails with `agent.credentials_invalid`, naming each missing file and where it was looked for. With `--from`, every file comes from `DIR`. Either way a missing optional file is skipped and a missing required one gives `not_found`, with the reason naming the file. Prints `imported`, `failed` (with the reason) or `not_found`. When the login is already an account of the kind, that account gets the new credential files (its other files stay) and keeps its id, label, active mark and project choices, and the command prints `updated account <ID> with the imported files instead of adding one` (`--json`: `updated`) (decision 0098); while that account is busy the import fails instead. Names and sizes only. Without `--from` it needs a service that offers `agent-import-detect/1`. |
| `agent accounts candidates [<KIND>]` | Shows where an import without `--from` would copy from: per kind `ready` or `not ready` and its folder (the folder of the first required file), then each declared file with `required` or `optional`, `found` or `missing`, its path, and the override variable when one was used (`from CLAUDE_CONFIG_DIR`). Without a kind, every kind that declares login files. Reads metadata only, never a file's contents. `--json` prints the `agent.accounts.import_candidates` result. |
| `agent accounts activate <KIND> <ACCOUNT> [--project P]` | Makes the account the kind's active one, or the account project `P` uses for the kind. |
| `agent accounts remove <KIND> <ACCOUNT>` | Deletes the account after its sign-out, if the kind declares one. Refused while a live run uses it. |
| `agent accounts check <KIND> [<ACCOUNT>]` | Runs the kind's check on an account, or on the active one, and prints its state. |
| `agent title --kind <KIND> --prompt TEXT [--reply TEXT] [--model MODEL] [--account ACCOUNT]` | Names a chat from its first prompt and reply by one run of the kind's one-shot form, with the account's home: `--account`, else the kind's active account. Prints the slug alone (`--json`: `{ slug }`): at most four lowercase words joined by `-`, at most 48 characters of `a-z`, `0-9` and `-`, accents folded (`canción` is `cancion`). Refused with `agent.oneshot_unsupported` for a kind with no one-shot form, `agent.credentials_invalid` with no usable account, `agent.oneshot_timeout` after 30 seconds and `agent.oneshot_failed` when the run fails or names no usable title. Below host-admin only the kind's active account may be named (`permission.denied` otherwise). The title is English ASCII, whatever the chat's language. Needs a service that offers `agent-title/1` (decision 0167). |

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
- An in-place workspace (`workspace create --in-place`, decision 0112) is the project's own checkout:
  `workspace rm` only forgets it. Its directory and branch stay, no file is touched, and its
  archive script does not run.
- A directory that git does not recognise as its worktree refuses with `workspace.orphan`, even
  with `--force`. `--forget-orphan` forgets the row and keeps the directory; git keeps its record
  of the worktree until it prunes it.
- The main worktree of a repository is never removed, and a locked worktree is refused
  with `workspace.locked`.
- A workspace whose delegated children write in workspaces of their own, a child's child
  included, refuses with `workspace.has_children`; the message names them. The workspace's own
  refusals above are checked first and come before it. `--with-children` removes them first, deepest first, each with the same checks and the other flags given. A
  refused child stops the removal: the children already removed stay removed, the message names
  them, and the workspace is kept (decision 0110).
- Before anything is stopped or removed, the project's `orca.yaml` `scripts.archive` runs in the
  worktree, in a service terminal titled `Archive`, for up to 120 seconds, and the output prints
  `archive script ran in terminal <ID>`. A non-zero exit or a timeout refuses the removal with
  `workspace.archive_failed`, and the workspace stays `ready`; `--allow-failed-archive` removes it
  anyway and prints `archive failed, removed anyway: <reason>`. An untrusted script does not run
  (`archive skipped: <reason>`) and the removal goes ahead. Folder workspaces have no archive
  script (decision 0095). Each child removed by `--with-children` gets the same archive run.
- Removing any workspace, worktree or folder, also deletes what the service kept of what ran
  in it: its sessions with their archived output, and its terminals with their scrollback,
  ended ones and those a retained session kept included. The archive script's terminal stays,
  so `archive script ran in terminal <ID>` can still be read. Files on disk are not touched.

Settings that affect workspace names are changed with `config set` (`config list` shows every setting).
`workspaces.rename_from_first_prompt` (`true` by default) renames a generated workspace after the first prompt the person
sends to its first session, from a title the kind's one-shot form writes, and its branch with it. Prompts typed by the
prompt queue count too. A rename by this setting makes the name the person's, so the workspace is renamed once, and
never a typed name (decision 0118). Set it to `false` to keep the generated names.

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
- When the connection drops or the service stops, `terminal attach` keeps the session. It
  prints `[connection lost (...); unverifiable until it returns, ...]`, retries with the
  same backoff as the watches, and attaches again from the output already shown, so nothing
  is shown twice or skipped. If the service no longer holds that output, the screen is
  redrawn and it says so. Typing while out of contact is not sent, and it says so; `Ctrl+]`
  still detaches. A refusal, such as a revoked device, ends the attach with its reason.
- `terminal kill` and `terminal close` end the processes; stopping the service ends every
  terminal and nothing is restored on the next start.
- A program that asks for the foreground, background or cursor colour (OSC 10, 11 or 12)
  while no answering emulator is attached gets the service's answer in the xterm form
  (`rgb:RRRR/GGGG/BBBB`, with the query's terminator). The colours are the desktop's
  terminal defaults, light when `appearance.theme` is `light` and dark otherwise, `system`
  included. While an emulator is attached (`terminal attach`, the desktop), the service
  leaves colour queries to it and never waits for its reply.

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
- `agent start --chat` (decision 0101) starts the session as a structured chat: the service runs the kind's CLI with
  piped streams, keeps its transcript, and answers its permission requests through `agent.chat.approval.answer`.
  Only a kind with a `chat` block whose CLI version is in range can: `agent kinds --json` shows each kind's `chat`
  (`supported`, and otherwise `reason`: `not_detected`, `version_unknown` or `version_outside_range`, with the probed
  `version` and the supported `range`). The built-in `claude` kind supports Claude Code 2.1.258 through 2.x, and the
  built-in `codex` kind Codex 0.172.0 through 0.x (`codex app-server`, decision 0173; its `--model` and `--effort` apply,
  and a resume reopens the conversation the first run named). Any other kind, or a version outside the range, fails with `agent.chat_unsupported`, whose `data` is that `chat` block; start it
  without `--chat` instead. `agent resume` of a chat session resumes it as a chat, `agent stop`, `agent status` and the
  queue work as for a terminal session.
- `agent start` without `--workspace` starts a session with no project (decision 0094). It
  runs in its own folder, named after the session, inside the conversations folder
  (`conversations.folder`, default `~/MassOrca/Conversations`, made on first use). The session
  has no worktree and no branch, and `project list` does not show it. A start that fails
  leaves no folder or workspace behind. If the conversations folder is itself a project, the
  start fails with `config.invalid_value`. `--model` and `--effort` pass their values as the
  kind declares them, as a flag the value follows or as `args` with `{value}` (decision 0096). A
  kind that declares neither refuses the option with `agent.option_unsupported`. An `--effort`
  that the kind's levels do not list, or that the named model does not accept, fails with
  `request.invalid_params` naming the allowed ids. A resumed run gets the model and effort of the
  start again when the kind still takes them (decision 0168); one it no longer takes is left out, and the run's `notes` say
  so. The session shows the model and effort its latest run used, as `model:` and `effort:`, with `branch:` and a
  one-line `preview:` of its result.
- `agent start` runs the kind's program directly in a new terminal of the workspace (no
  shell in between), with the first prompt as the kind declares. A worktree workspace holds one
  live session that is not `--read-only`; a second writing start there fails with
  `agent.workspace_busy`. A folder workspace takes several writing sessions (decision 0169), and each lists the
  others that are live in `shared_folder_with`. A kind that is not installed fails with `agent.not_installed`.
  Arguments after `--` go to the agent after its own. A kind whose `prompt.mode` is `none`
  receives `--prompt` at its first `idle` report, and a kind that reports no `idle` refuses
  such a prompt with `agent.prompt_invalid`; the held prompt is dropped if no `idle` comes
  within 120 s or the run ends first, and a drop is noted in the session's `notes`. A prompt longer than 4 KiB, or one with a line break for a
  kind that pastes bracketed, is typed at the first `idle` the same way when the kind reports one,
  instead of riding the command line (decision 0168); for a `.cmd` or `.bat` program the limit is 2 KiB, since `cmd.exe`
  holds 8,191 characters on a line, and a longer prompt the kind cannot type is refused with `agent.prompt_invalid`.
  The text of `--prompt` follows the same
  rules as `agent prompt`, control characters included.
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
  the new run reports. The arguments and environment of the first start are not kept; its
  model and effort are, when the kind still takes them (decision 0168). A live
  run fails with `agent.busy`; a kind with no resume form, or a session whose agent ref is not
  known yet, or whose last run has no recorded end, fails with `agent.resume_unavailable`; a
  session whose agent saved no conversation for its ref fails with `agent.resume_not_found`
  before any run starts (decision 0160); another
  writing session in the same worktree
  fails with `agent.workspace_busy`; in a folder workspace it runs beside the others (decision 0169).
- `agent status` shows, for an exited session that cannot resume, a line
  `resume: unavailable: REASON (CODE)`, for example
  `resume: unavailable: the agent saved no conversation for this session (agent.resume_not_found)`.
  With `--json` every session carries `resume: { available, code?, reason? }`. A resumed run
  that fails because its agent found no conversation (Claude: "No conversation found with
  session ID") leaves the session exited with that state, and its exit as it was.
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
  parent are not carried over, and the result's `note` says so. A writing child of a parent in
  a plain folder workspace runs in that folder, with no copy, and its `note` and
  `notes` say "This workspace has no Git, so the child works in this folder."; the
  folder takes several writers, so the child's writing start is not refused there (decisions 0161, 0169). A parent in a worktree with
  no branch fails with `agent.delegate_needs_git` unless `--read-only` is given. A `--read-only` child
  runs in the parent's workspace and does not hold its writing slot. `--same-workspace` (not with
  `--name`) runs a writing child in the parent's own worktree, the person's manual pick; its `note`
  and `notes` say "The child works in its parent's worktree, and the parent waits until it
  finishes." (decision 0170). A second writing `--same-workspace` child, while another writing
  session of the tree (a sibling or a grandchild) works unfinished in that worktree, or an ancestor
  of the delegating session has a live run there, fails with `agent.workspace_busy` naming it in
  `data.session`; `agent resume` of such a child is refused the same way. While a writing child works unfinished in the
  parent's own workspace (that pick, or a plain folder), the parent waits: its queued prompts hold with reason `child_writing`, and
  `agent prompt` to it fails with `agent.busy` naming the child in `data.child`, unless `--force`.
  A child is finished from its first `done` report until its next prompt; a later `waiting` does
  not reopen it, and a kind that never reports `done` (Antigravity, Gentle Shell, a chat run)
  finishes only when its run ends. When a child becomes finished, or its run ends unfinished, the
  parent's queue gets a one-line notice:
  "MassOrca: child session ID finished: OUTCOME. Read its result with massorca agent status
  ID --json, or bring it back with massorca agent bring-back ID." OUTCOME is `done`, `failed`,
  `cancelled`, `interrupted`, `stopped` or `exited`; neither the child's title nor its result text is in it. A
  parent with no live run gets the notice paused (`paused_stopped`). A delegated session
  delegates only while its depth stays within `agents.delegation_depth` (default 1, so a child cannot
  delegate); past it the call fails with `agent.depth_exceeded`, which names the limit and the setting.
  Each session's `--json` record carries `delegation`: `writing_child` (`own_worktree`,
  `shared_folder` or `unavailable`), `read_only_child`, and a `reason` when no writing child is
  possible.
- Every agent run finds `massorca` first on its `PATH` (the folder of the service's own
  executable). A kind that declares an `instructions` channel (the built-in `claude`, through
  `--append-system-prompt`) gets a short text, when `claude --help` lists that flag, on its command line telling it to use `agent
  delegate`, `agent tree`, `agent status`, `agent stop` and `agent bring-back` when the person
  asks for a child session, and that its own subagents are not shown in the Sidebar; only while
  `agents.delegation_depth` leaves room for a child (decision 0161). A CLI that does not list the
  flag starts without the text, and the session's `notes` say so.
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

`service run` prints its readiness record once the service accepts connections, and only after its
network listeners have made their first bind attempt (bound, or waiting with the reason). It is never
printed earlier, and it is printed once per process. In text mode it is a few lines: `service ready for
profile <name> (pid <n>)`, `version <v>, protocol <p>`, `local endpoint: <path or pipe>`, the listener
table, `server pin`, `advertised endpoints`, `pending offers`, and `pairing: available` or
`pairing: unavailable: <reason>`, and `live terminals`. With no network listener it says so instead.

With `--json` it is one line on stdout, `{"ok":true,"result":{...}}`. `result` holds:

- `schema` (`orca_server_ready`) and `schema_version` (`1`);
- the start identity: `profile`, `pid`, `version`, `protocol`, `started_at_ms`, `capabilities`;
- `endpoint`, the local socket path or named pipe;
- `bound_endpoints` and `advertised_endpoints`, `host:port` lists: the bound listener addresses, and
  what a pairing offer lists when it names no address (the bound listeners and their MagicDNS names).
  Both are empty without a network listener;
- `pairing`: `available`, `reason` when not available, and `pending_offers`, each
  `{offer_id, expires_at_ms, scopes}`. Pending offers at readiness are normally none;
- `health`: `live_terminals`, the terminals alive in the service's in-process terminal host;
- `network`, with a network listener only: `listeners`, `server_pin` (a public key fingerprint) and
  `pending_offers` (a count).

A pairing secret is never printed. Three fields of the startup contract are not in the record, because
the service does not produce them: a runtime id, the managed-WSL reconciliation state, and terminal-daemon
health. A web client URL and a pairing QR or URL are not produced either: no offer exists at startup, and
an offer's secret is shown only when it is made. `server status` shows the same listeners and pin later.

The service log goes to the profile's log files only unless `--log-stderr` is given.

### Automations

Scheduled automations (decision 0123) and their runs (decision 0125). The service runs each enabled automation's
agent in its project on its schedule, and keeps its run history.

| Command | What it does |
|---|---|
| `automations create <NAME> --project <PROJECT> --prompt <TEXT> [--agent <KIND>] [--cadence hourly\|daily\|weekdays\|weekly\|cron] [--time <HH:MM>] [--weekday <DAY>] [--cron <EXPRESSION>] [--zone <ZONE>] [--grace <MINUTES>] [--creation-key <KEY>]` | Creates an automation in a project, enabled, with its next run from its schedule. `--project` takes an id, a name or a path. Defaults: agent `claude`, daily at `09:00`. `--cadence hourly` runs every hour at the minute of `--time`; `weekly` needs `--weekday` (`mon` to `sun`); `cron` needs `--cron` with five fields, such as `0 9 * * 1-5`, and ignores `--time`. Without `--zone` the host's zone is used; the create is refused when the host's zone cannot be read. `--grace` is 0 to 10080 minutes, 720 when unset. A retry with the same `--creation-key` returns the automation the first create made. |
| `automations edit <AUTOMATION> [--name <NAME>] [--project <PROJECT>] [--prompt <TEXT>] [--agent <KIND>] [--cadence <CADENCE>] [--time <HH:MM>] [--weekday <DAY>] [--cron <EXPRESSION>] [--zone <ZONE>] [--grace <MINUTES>]` | Changes the options given and no others. A schedule change recomputes the next run. `--zone ""` returns the automation to the host's zone. |
| `automations show <AUTOMATION>` | Shows one automation, by id or by exact name. |
| `automations list` | Lists the automations, in creation order. |
| `automations remove <AUTOMATION>` | Removes an automation. It cannot be restored. |
| `automations enable <AUTOMATION>` | Enables an automation and schedules its next run from now. |
| `automations disable <AUTOMATION>` | Disables an automation; it has no next run until it is enabled. |
| `automations run <AUTOMATION> --now` | Runs the automation now and prints the run it recorded. The schedule does not move. `--now` is required. A run that cannot start, such as one whose automation has no project, is still recorded with the reason. |
| `automations runs <AUTOMATION> [--limit <N>] [--before <NUMBER>]` | Lists the automation's runs, newest first by run number: 20 when `--limit` is unset, 100 at most. Each run shows its number, its state (starting, running, completed, missed, not started or failed), the time it stands for in UTC and why it did not run. `--before` lists the runs numbered below it; the last line names the next page's `--before`. |

An automation whose project was removed stays listed with no project and does not start. A name two automations
share is refused until the one meant is named by its id.

A scheduled occurrence runs once when the service claims it within the automation's grace; a later one, and earlier
occurrences the service missed, are recorded as missed and never replayed. A run ends completed when its agent reports
done, with the agent's result, or failed when the agent exits without reporting done. The newest 100 finished runs of
each automation are kept. A host that does not offer `automation-runs/1` says it is too old.

### Calendar

The runs of routines and automations in a window of days, on the shared schedule (decision 0124). Read only.

| Command | What it does |
|---|---|
| `calendar [--from <DATE>] [--days <N>] [--kind routines\|automations] [--done]` | Lists the runs in a window that starts at `--from`, a UTC date (`YYYY-MM-DD`, today in UTC when unset), and lasts `--days` days (1 to 62, 7 when unset). `--kind routines` lists only Bot Teams routines and `automations` only Workspaces automations; both when unset. Each run shows its date and time on its routine's or automation's zone, or on the host's zone when it has none. Done routine runs and completed automation runs show only with `--done`. |

Upcoming runs start at the time of the call. A paused routine and a disabled automation have no upcoming runs.
A host that does not offer `calendar/1` says it is too
old for the calendar.

### Hosts and pairing

A server is `massorca service run --listen <ADDR>` on another machine. `--listen` takes
`tailscale[:PORT]` (this node's Tailscale address, the default port 7347), `localhost:PORT` or
`IP:PORT`. An address on every interface (`0.0.0.0`, `::`) needs `--allow-all-interfaces`.

The server's owner makes an offer, and the other computer accepts it:

```sh
# on the server
massorca pair offer
# on the other computer, reading the offer from stdin
massorca pair accept - --name vps
```

The offer lists the addresses the service is bound to. When the other device reaches this computer at a
different address (a NAT, a container, or an Android emulator, which reaches its computer at `10.0.2.2`), give it
with `--address`: `massorca pair offer --address 10.0.2.2`. It takes a host name, `host:port`, an IPv4 or IPv6
literal (`[fd7a::1]:7347`) or a `wss://` or `https://` URL; a value without a port takes the listener's port.
Wildcard addresses (`0.0.0.0`, `::`), port 0, paths, queries and user names are refused.

An offer is a credential until it is used or expires. If one leaks, retire it with
`massorca pair cancel <OFFER-ID>`, or every pending one with `massorca pair cancel --all`.

The accepted host is stored in this profile. `--host vps` then addresses it, and `host list`,
`host status` and `host remove` manage it. A host that does not answer is reported as
`unreachable`; nothing is inferred about the work on it (decision 0044). `host self-test <NAME>` runs
each check of that host in turn and names the step that fails, with its fix.

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
| 1 | The service answered with an error, or local data could not be used. The code is printed. `open` exits 1 with `desktop.not_found` when no desktop app is found. |
| 2 | The command line is invalid, including an invalid profile name or log filter, an invalid `--listen` (`usage.invalid_listen`), `--host` on a command that works on this computer only (`cli.host_not_supported`), or `terminal attach` without a terminal on stdin and stdout. |
| 3 | `service run`: another service already owns this profile. |
| 4 | The service is not running and could not be reached or started. For a host: `host.unreachable`, `host.identity_mismatch`, or a refusal such as `auth.device_revoked`. `host memory` reports a lost service (`service.unavailable`, `service.disconnected`, `host.unreachable`) as `memory.unverifiable` (`host ports`: `ports.unverifiable`), with the original code in `data.cause`; identity mismatches and refusals keep their code. |

## The command on PATH

`massorca cli install` puts `massorca` on your PATH for this user only (decision 0088). It needs no administrator
rights, edits no system setting and never edits a shell profile.

- Unix (macOS, Linux): a link at `~/.local/bin/massorca` to the binary that ran the install. If `~/.local/bin` is not
  on your `PATH`, `cli status` says so; add it to your shell profile yourself.
- Windows: a command file `massorca.cmd` in `%LOCALAPPDATA%\Programs\MassOrca\bin`, which runs the binary with every
  argument. That folder is appended to your user `Path` (`HKCU\Environment`), and running programs are told about the
  change. Open a new terminal to use it.

Install is idempotent and points an existing command at the running binary. It refuses to overwrite a file it did not
make (`cli.command_conflict`, exit 1). If Windows does not let massorca read the user PATH, install stops before it
changes anything (`cli.user_path_unreadable`). Uninstall removes only the link or command file that massorca made, and
removes the folder's PATH entry only when nothing else is in that folder.

`cli status` reports one `state`; its JSON result carries the same fields as the text:

| `state` | Meaning |
|---|---|
| `installed` | The command runs this binary and its folder is on PATH. |
| `not_on_path` | The command runs this binary, but its folder is not on PATH. |
| `different_binary` | The command runs another binary. `cli install` points it at this one. |
| `stale` | The command runs a binary that is no longer there, such as one an update moved. The desktop's service start points it at the binary that started (decision 0113); `cli install` does it at once. |
| `not_installed` | No command is installed. |
| `conflict` | A file massorca did not make is at the command path. |

The result also has `command`, `target`, `points_to`, `bin_dir`, `bin_dir_on_path` (`null` when the Windows user PATH
could not be read) and `changed`. The command runs the binary that ran `cli install`. When an update moves that binary,
the command is `stale` until the desktop starts its service: the desktop runs its bundled `massorca` with a hidden
`--repair-command` flag, which points a stale command at that binary and changes nothing else (decision 0113). A binary
that an update leaves at its old path still runs, so the command keeps it, and `different_binary` shows that until
`cli install` runs.

## Logs and files

Each profile lives in `<home>/profiles/<name>/`. The service writes daily log files to
`logs/` as `service.<date>.log`, keeping the last seven days.

## Desktop app

`massorca open` starts this app on the profile of the command, passing `--profile` and `--home`, and
returns at once. The app has no single-instance hand-off yet, so an `open` while it runs starts a second
window. Its stderr is not shown, so a start that fails after launch is silent.

`massorca-desktop` opens the MassOrca window on a profile's service, or on sample data with
`--sample`. It takes `--profile`, `--home` and `--no-autostart` as the CLI does, and two flags of its
own:

| Option | Meaning |
|---|---|
| `--sample` | Open on sample data and never touch a service. |
| `--page <NAME>` | Open one page or dialog once the data it shows has arrived. An unknown name exits with status 2 before the window opens, and the error lists the valid names. |

The page names are `workspace`, `settings-general`, `settings-hosts`, `settings-vault`, `pairing`,
`board`, `add-host`, `add-secret`, `replace-secret`, `remove-secret`, `new-team`, `add-bot`, `session`, `delegate` and `bring-back`.

A page waits while the service is still answering. When the service has answered without the data
a page or dialog needs, it opens its parent page instead, and stderr says why. `replace-secret` and
`remove-secret` need an owned secret (parent: Settings, Vault). `board` and `add-bot` need a team, and
`new-team` and `add-bot` need Bot Teams (parent: the Bot Teams home). `session` opens the newest agent
session, or the first workspace's start page when there is none. `delegate` opens the delegate form under
the newest agent session, and `bring-back` opens the confirm for the first child session not yet brought
back. Each falls back to its parent when the service has no such session; `delegate` also needs a service.

`MASSORCA_DESKTOP_BACKGROUND=1` opens the window hidden and unfocused, so a page can be checked
without showing it:

```sh
MASSORCA_DESKTOP_BACKGROUND=1 massorca-desktop --sample --page settings-vault
```

In PowerShell, set the variable first: `$env:MASSORCA_DESKTOP_BACKGROUND = "1"`, then run
`massorca-desktop --profile dev --page board`.

## Not yet available

Extension commands arrive in later milestones. Deferred from ADR 0008 (branch prefixes,
generated names, terminal and agent guards on removal, squash-merge detection) are
not available yet.
