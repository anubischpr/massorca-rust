# M1 validation: core and CLI on Windows and WSL

Milestone M1 (`docs/WORKING-RULES.md`): workspaces, terminals that outlive the
client, and git, validated on Windows and on WSL (Ubuntu 24.04). Validated on
2026-11-21 with the `massorca` CLI built from the commit below. The run followed
the scenario in the M1 brief, command by command, and the output is verbatim
apart from the placeholders listed under "Method".

## Result

- Windows: 41 of 42 claims pass. Claim 42, the full `cargo test` run, had one
  failing test (`a_terminal_is_created_attached_typed_into_resized_detached_and_killed`),
  which passed in 4 of 5 isolated reruns (see 11). Format, clippy and the rest of the
  tests passed.
- WSL: all 42 claims pass, including format, clippy and the full test run (1658 passed,
  0 failed, 3 ignored).
- Failures and observations are numbered 1 to 11 below. Each claim row refers to the
  item that explains it.

## Commit and build

- Tested commit: `e07899d9c5ebd35daee6fb5a89c97bc327e81d15` (`origin/main`).
  Windows and WSL built the same commit; each checkout was verified to resolve to it.
- Build on both systems: `cargo build --locked -p massorca-cli` (debug profile).
  Windows: built in 2m 09s, no warnings. WSL: built in 1m 36s.

## Environments

| | Windows | WSL |
|---|---|---|
| OS | Windows 11 Pro, build 10.0.26300 | Ubuntu 24.04.1 LTS on WSL 2 |
| git | 2.49.0.windows.1 (Git for Windows) | 2.43.0 |
| Rust | rustc 1.98.1 (48a229cea 2026-09-01), from `rust-toolchain.toml` | rustc 1.98.1, from `rust-toolchain.toml` (the distro default 1.92.0 was not used) |
| Terminal shell | PowerShell 7.7.0 (`pwsh`), the default | `/bin/bash`, the default |
| Logical processors | 16 | 16 |

## Method

- One isolated profile per system: `MASSORCA_HOME` was a fresh `mktemp -d`
  directory. Every git project was added with `--workspaces-root` inside the
  same temp area, so the scenario runs did not write to a real profile or home directory.
- Test repositories were made with `git init` in the temp area, with the
  placeholder identity `deploy <deploy@example.com>`. `GIT_CONFIG_GLOBAL` pointed
  at a placeholder config (that identity, `core.autocrlf = false`), so the
  service's git calls did not read the user's global configuration.
- The remote for fetch, pull and push was a local bare repository in the temp area.
- Services were stopped with `massorca service stop`, and clients and other
  processes were ended by process id. No process was ever killed by image name.
- Placeholders in the transcripts: `<tmp>` is the temp root, `<repo>` the test
  repository, `<pid>` a process id, `<host>` the machine's network name. Windows
  paths use the `<tmp>\...` form and WSL paths `<tmp>/...`.
- The attach command needs a real terminal on stdin and stdout. Windows: a
  hidden console started with PowerShell `Start-Process`. WSL: `script`, which
  provides a pseudo-terminal and records its output.

## Claims

Result key: **pass** = observed as the docs state. Notes refer to the numbered
items under "Failures and observations".

| # | Claim | Windows | WSL |
|---|---|---|---|
| 1 | The CLI starts the service on first use | pass | pass |
| 2 | `service status` reports and never starts the service | pass | pass |
| 3 | `--no-autostart` fails when no service runs (exit 4, `service.unavailable`) | pass | pass |
| 4 | A second service for the same profile is refused (exit 3, `service.already_running`) | pass | pass |
| 5 | A git project and a folder project can be added | pass | pass |
| 6 | A git project creates a worktree on a new branch; a folder project registers its folder | pass | pass |
| 7 | `workspace list` and `workspace show` report the workspaces | pass | pass |
| 8 | Removing a worktree with changes is refused (exit 1, `workspace.dirty`) | pass | pass |
| 9 | `workspace rm --force` discards the changes, deletes the worktree and its branch | pass | pass |
| 10 | Removing a folder workspace forgets it and keeps the folder and its files | pass | pass |
| 11 | `project rm` is refused while workspaces exist (`project.has_workspaces`) | pass | pass |
| 12 | Projects and workspaces are unchanged after a service restart | pass | pass |
| 13 | A terminal starts a shell in its workspace and the create command returns | pass | pass |
| 14 | Input sent to a terminal runs, and `terminal screen` shows the output | pass | pass |
| 15 | `terminal resize` changes the size | pass | pass |
| 16 | `terminal attach` without a terminal fails (exit 2, `usage.not_a_terminal`) | pass | pass |
| 17 | Ending the attach client leaves the service and the shell running; output still arrives while detached | pass | pass |
| 18 | Reattaching shows earlier output in the snapshot, and new input works | pass (see 5) | pass |
| 19 | `terminal kill` leaves the terminal `exited` (reason `killed`) and ends its shell | pass | pass |
| 20 | Stopping or restarting the service ends its terminals' shells; `terminal list` is empty afterwards | pass (see 6) | pass |
| 21 | `git status` lists staged, unstaged and untracked files | pass | pass |
| 22 | `git diff` lists changed files; `git patch` shows one file's patch | pass | pass |
| 23 | `git stage` and `git unstage` move changes between index and working tree | pass | pass |
| 24 | `git discard` refuses staged changes (`git.discard_staged`) and untracked files (`git.discard_untracked`); both waivers work | pass | pass |
| 25 | `git commit` records the staged changes with the configured identity | pass | pass |
| 26 | `git commit --amend` replaces HEAD | pass | pass |
| 27 | Amending a published HEAD is refused (`git.commit_published`); `--allow-published` waives it | pass | pass |
| 28 | `git log` lists history newest first | pass | pass |
| 29 | Branch create, switch, rename and delete | pass | pass |
| 30 | Deleting an unmerged branch is refused (`branch.unmerged`); `--force` deletes it | pass | pass |
| 31 | A switch that would overwrite local changes is refused (`git.checkout_would_overwrite`) | pass | pass |
| 32 | `git watch` reports a change after a tracked file is edited | pass (see 2) | pass |
| 33 | Fetch and push against a local bare remote; the first push sets the upstream | pass | pass |
| 34 | Pull fast-forwards; a divergence is refused unless `--mode merge` is given | pass (see 3) | pass |
| 35 | A non-fast-forward push is rejected and never forced | pass (see 3) | pass |
| 36 | Fetch prunes a remote branch that was deleted | pass | pass |
| 37 | Typed error codes with exit 1; bad arguments and unknown commands exit 2 | pass | pass |
| 38 | Exit codes 0 to 4 as documented in `docs/architecture/core.md` | pass | pass |
| 39 | `--json` prints the error object with its code | pass | pass |
| 40 | `cargo fmt --all --check` | pass | pass |
| 41 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | pass |
| 42 | `cargo test --workspace --locked --no-fail-fast` | fail: 1658 passed, 1 failed, 3 ignored (see 11) | pass (1658 passed, 0 failed, 3 ignored) |

## Repository checks

| Check | Windows | WSL |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | exit 0, no output |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, finished in 5m 45s | exit 0, no warnings |
| `cargo test --workspace --locked --no-fail-fast` | exit 101: 83 result blocks (71 test binaries and 12 doc-test blocks), 1658 passed, 1 failed, 3 ignored. The failing test is `a_terminal_is_created_attached_typed_into_resized_detached_and_killed` (see 11) | exit 0: 83 result blocks (71 test binaries and 12 doc-test blocks), 1658 passed, 0 failed, 3 ignored |

The three ignored tests are screenshot tests (`service_screenshots`,
`shell_screenshots`, `gallery_screenshots`) that write PNG files and run only with
`-- --ignored`; they were not run.

## Failures and observations

1. **Windows test run, first attempt: did not run.** `cargo test` failed while
   building the test binary: `error: failed to remove file
   <target-win>\debug\massorca.exe`, `Caused by: Acceso denegado. (os error 5)`.
   The cause was the environment: the Windows service started for the scenario was
   still running from that same file. The service was stopped with
   `massorca service stop`, its process id was confirmed gone, and the test was run
   again. The result of the second run is in the repository checks table. No test
   result came from the first attempt.
2. **`git watch` on Windows is slow to become ready.** In a timed run, the
   `watching` line appeared 16.7 s after launch; in another run it had not appeared
   after 29 s. An edit made 3 s after launch, before that line, was not reported within
   40 s. An edit made 3 s after the `watching` line was reported as
   `changed: worktree` and the command exited 0. On WSL the line appeared after
   0.5 s and the same edit was reported. The likely explanation is that a change made
   before the watcher is ready is not reported; this was not confirmed, and the cause
   of the delay on Windows was not investigated.
3. **A refused pull or push exits 0.** `git pull` on a divergence prints
   `not pulled: ... use --mode merge or --mode rebase` and exits 0. A rejected push
   prints `origin/feature: rejected (non-fast-forward)` and exits 0. The JSON form is
   `{"ok":true,"result":{...,"outcome":"diverged"}}` and
   `{"ok":true,"result":{"outcome":{"kind":"rejected","reason":"non-fast-forward"}}}`.
   Other refusals in this run exit 1 with a typed `error.code`. A script that checks
   only the exit code would not notice a refused pull or push. Both systems behaved
   the same.
4. **Git commands on Windows were slow.** Through the service, a refused pull took
   21 s, a merge pull 28 s, a push 9 s, and a raw `git fetch` in the same repository
   took 14 s. WSL operations took about 1 s. The Windows machine was shared with
   other builds and sessions (at one check: 8 cargo, 2 rustc, 17 massorca and 26 git
   processes). These times are not benchmarks.
5. **Attach output on Windows could not be captured.** `terminal attach` needs a
   console. It was run in a hidden console, so its own screen output went nowhere
   visible. The snapshot was checked with `terminal screen`, which prints the same
   screen model, not with the attach output. The reattach result is therefore
   partial evidence on Windows. On WSL, `script` captured the attach output, and the
   snapshot and live input were checked there directly.
6. **A false positive during the Windows restart check, corrected.** The first check
   that the shell survived `service restart` compared only the process id. Windows
   had reused that id for a `link.exe` process, so the shell was reported alive when
   it was not. The check was repeated with the process start time: the shell was
   confirmed alive before the restart and was gone at the first sample after the
   restart returned (t+0 s). A second run on `service stop` recorded the shell's
   identity at creation and found it gone at the first sample, 4 s after the stop
   returned; liveness right before that stop was not sampled. The result in claim 20
   comes from the repeated restart check and this stop run.
7. **Terminal size changed when a Windows attach client connected.** After
   `terminal resize` set 120x40, a hidden attach client was connected and ended.
   `terminal show` then reported `size: 120x30`, probably the size of that client's
   console (not read directly). The attach request probably carries the client's
   size. The WSL attach client was started with 120x40, so no change was seen there.
8. **One unexplained line in WSL setup.** The first WSL setup command printed
   `WSL HEAD: b89216da8b33...`, the tip of the main checkout's local `main` branch,
   not the fetched `origin/main`. The clone's reflog shows HEAD was only ever moved to
   e07899d, and both sides were checked to be e07899d before the build. The line was
   not reproduced.
9. **The first WSL repository-check launch did not run.** The launch passed a
   `/mnt/c/...` script path as an argument, and Git Bash rewrote it, so the script was
   not found and nothing ran. The checks were relaunched with the script on stdin.
10. **Exit reason strings differ by platform.** `terminal kill` reports
    `exit: killed, code 3221225786` on Windows and `exit: killed, signal Hangup` on
    WSL. Both are the documented `killed` reason; the detail is platform-specific.

11. **One Windows test failed in the full run, and the failure is intermittent.**
    `cargo test` on Windows exited 101 with one failing test:
    `a_terminal_is_created_attached_typed_into_resized_detached_and_killed` (package
    `massorca-desktop`, test binary `terminal_bridge`). It panicked at
    `crates\desktop\tests\terminal_bridge.rs:254:5` with `no output after a snapshot
    reattach within 30s; the screen was [...]`. The captured screen shows the cmd.exe
    banner over the `echo third` line and no new output. The same test was rerun alone
    five times with `cargo test --locked -p massorca-desktop --test terminal_bridge --
    a_terminal_is_created_attached_typed_into_resized_detached_and_killed --exact`.
    Four runs passed (14.3 s, 19.1 s, 17.1 s and 30.0 s) and one failed with the same
    panic at the same line (36.2 s). On WSL the same test passed in the full run. The
    cause was not investigated, and no product code was changed. Other builds were
    running on the machine during these runs (see 4).

## Not checked

- Keyboard input in an attach client, on both systems: typing into the attach
  console, the `Ctrl+]` detach key, arrow keys, and Windows key translation. Input
  was sent with `terminal send`, which goes through the same input path. The
  Windows attach console was hidden, so no key was sent to it.
- Service crash: the service was only stopped or restarted. Terminal survival when
  the service process dies unexpectedly was not checked.
- Remotes other than a local bare repository, and any authentication path.
- `git pull --mode rebase`, `git push --force-with-lease`, `git abort`, the
  `git log` paging cursor, `git switch` to a branch checked out elsewhere, and
  `git discard --dry-run` on staged changes.
- `project rm` on a project with no workspaces (the refusal was checked, not the
  success path). `workspace create` options `--base`, `--path`, `--existing-branch`
  and `--request-id`. `terminal screen --scrollback`, `terminal close`, and
  `terminal kill --force`.
- Several clients attached at once, and a second client while one is attached.
- macOS, a native Linux host, the VPS, and the Android client: outside M1's
  validation targets.

## Transcripts

### Windows

Shell for the scenario: Git Bash for the CLI and git commands; PowerShell 7.7 for
the attach clients and process checks. Trimmed to the meaningful lines.

```text
$ git init repo && commit README.md   # in <tmp>\repo, placeholder identity
b7aa687 chore: initial commit

$ massorca service status
not running
[exit 0]
$ massorca --no-autostart project list
error: the service is not reachable: El sistema no puede encontrar el archivo especificado. (os error 2) (service.unavailable)
[exit 4]
$ massorca project list
no projects
[exit 0]
$ massorca service run                 # second service, same profile
error: another service already runs this profile (service.already_running)
[exit 3]

$ massorca project add <tmp>\repo --git --workspaces-root <tmp>\ws
added project repo (git) at <tmp>\repo
[exit 0]
$ massorca project add <tmp>\notes --folder
added project notes (folder) at <tmp>\notes
[exit 0]
$ massorca workspace create repo feature
created workspace feature (worktree) at <tmp>\ws\feature
[exit 0]
$ massorca workspace create notes main
created workspace main (folder) at <tmp>\notes
[exit 0]
$ massorca workspace create repo wsdirty
created workspace wsdirty (worktree) at <tmp>\ws\wsdirty
[exit 0]
$ git -C <tmp>\ws\wsdirty status --short        # after editing README.md and adding scratch.txt
 M README.md
?? scratch.txt
$ massorca workspace rm wsdirty
error: <tmp>\ws\wsdirty has 1 uncommitted and 1 untracked change(s); remove with force to discard them (workspace.dirty)
[exit 1]
$ massorca --json workspace rm wsdirty
{"error":{"code":"workspace.dirty","data":null,"message":"<tmp>\\ws\\wsdirty has 1 uncommitted and 1 untracked change(s); remove with force to discard them"},"ok":false}
[exit 1]
$ massorca workspace rm wsdirty --force
removed workspace wsdirty
deleted branch wsdirty
[exit 0]

$ massorca project rm repo
error: project "repo" still has workspaces; remove them first (project.has_workspaces)
[exit 1]
$ massorca service restart
profile: default
pid: <pid>
...
[exit 0]
$ massorca project list                 # ID column omitted
notes   folder  <tmp>\notes
repo    git     <tmp>\repo
[exit 0]
$ massorca workspace list               # ID column omitted
feature  worktree  feature  <tmp>\ws\feature  ready
main     folder    -        <tmp>\notes       ready
[exit 0]
$ massorca workspace rm main
removed workspace main
kept directory <tmp>\notes
[exit 0]

$ massorca terminal create feature --cols 100 --rows 30 --title m1-term
id: 01a200f4-6391-742f-b69f-c6736dc824b5
state: live
shell: <home>\AppData\Local\Microsoft\WindowsApps\pwsh.exe
cwd: <tmp>\ws\feature
size: 100x30
pid: <pid>
[exit 0]
$ massorca terminal send <id> echo M1-HELLO-1
[exit 0]
$ massorca terminal screen <id>
PowerShell 7.7.0
PS <tmp>\ws\feature> echo M1-HELLO-1
M1-HELLO-1
PS <tmp>\ws\feature>
[exit 0]
$ massorca terminal resize <id> --cols 120 --rows 40
size: 120x40
[exit 0]
$ massorca terminal attach <id> < /dev/null
error: attach needs a terminal on stdin and stdout; `terminal screen` prints a screen without one (usage.not_a_terminal)
[exit 2]

# outlive test: attach client A in a hidden console (PowerShell Start-Process), pid <pid-A>
$ massorca terminal send <id> Write-Output M1-WHILE-ATTACHED
$ massorca terminal screen <id>       # shows M1-HELLO-1 and M1-WHILE-ATTACHED
$ Stop-Process -Id <pid-A> -Force     # client A ended by its process id
client A gone
$ massorca service info               # pid unchanged: <pid>
$ massorca terminal show <id>
state: live
size: 120x30                          # see note 7
pid: <pid-shell>
$ shell <pid-shell> still alive
$ massorca terminal send <id> Write-Output M1-WHILE-DETACHED
$ massorca terminal screen <id>       # M1-WHILE-DETACHED shown while no client is attached
# client B attached in a hidden console, pid <pid-B>
$ massorca terminal screen <id>       # M1-HELLO-1, M1-WHILE-ATTACHED, M1-WHILE-DETACHED, then...
$ massorca terminal send <id> Write-Output M1-AFTER-REATTACH
$ massorca terminal screen <id>       # ...M1-AFTER-REATTACH shown
client B alive before end: True; gone now: True

$ massorca terminal kill <id>
state: exited
exit: killed, code 3221225786
[exit 0]
shell <pid-shell>: gone
$ massorca terminal list                # the killed terminal stays listed as exited
$ massorca service restart              # with a second live terminal; its shell was gone at t+0 s
$ massorca terminal list
no terminals
[exit 0]
```

Git and workspace checks in `feature` (branch `feature`):

```text
$ massorca git status -w feature
on feature
 M README.md  +1 -0
?? a.txt
$ massorca git diff -w feature
+1 -0	README.md
1 files, +1 -0
$ massorca git patch -w feature README.md
diff --git a/README.md b/README.md
...
+second line
$ massorca git stage -w feature a.txt        -> staged
$ massorca git status -w feature
 M README.md  +1 -0
A  a.txt  +1 -0
$ massorca git unstage -w feature a.txt      -> unstaged
$ massorca git discard -w feature --dry-run README.md
would discard README.md (unstaged edits)
$ massorca git stage -w feature README.md
$ massorca git discard -w feature README.md
error: these files have only staged changes; pass include_staged to drop them: README.md (git.discard_staged)
[exit 1]
$ massorca git discard -w feature --include-staged README.md
discarded README.md (staged and unstaged changes)
[exit 0]
$ massorca git discard -w feature a.txt
error: discarding these deletes files git has no copy of; pass delete_untracked: a.txt (git.discard_untracked)
[exit 1]
$ massorca git discard -w feature --delete-untracked a.txt
deleted a.txt
[exit 0]

$ massorca git commit -w feature -m "feat: add b"
[feature 0531eba8da37] feat: add b
[exit 0]
$ git log -1 --format="author=%an <%ae>"       -> author=deploy <deploy@example.com>
$ massorca git commit -w feature --amend -m "feat: add b (amended)"
[feature 8c0f9fbda4b4] feat: add b (amended)
[exit 0]
$ massorca git log -w feature -n 3
8c0f9fbda4b4 deploy feat: add b (amended)
b7aa687ed48c deploy chore: initial commit

$ massorca git branch-create -w feature topic
created   topic 8c0f9fbda4b4
$ massorca git switch -w feature topic
switched to topic
$ massorca git branch-rename -w feature topic topic2
renamed topic to topic2
$ massorca git branch-delete -w feature topic2
deleted topic2 (was 8c0f9fbda4b4)
$ massorca git branch-create -w feature throwaway --switch
created * throwaway 8c0f9fbda4b4
   ... commit on throwaway ...
$ massorca git branch-delete -w feature throwaway
error: branch `throwaway` is not merged; pass force to delete it (branch.unmerged)
[exit 1]
$ massorca git branch-delete -w feature throwaway --force
deleted throwaway (was d23565c564b3)
[exit 0]
$ massorca git switch -w feature other          # other branch changes README.md; README.md has a local edit
error: the checkout would overwrite local changes: README.md (git.checkout_would_overwrite)
[exit 1]

# git watch, edit made after the watching line
$ massorca git watch -w feature --max-events 1
watching
changed: worktree
[exit 0]

# remote: local bare repository in <tmp>\remote.git
$ massorca git fetch -w feature
already up to date
$ massorca git push -w feature
origin/feature: created, upstream set
$ massorca git pull -w feature                  # other clone pushed one commit
fast-forwarded 8c0f9fbda4b4..03bd79e123b0
$ massorca git pull -w feature                  # local and remote commits diverge
not pulled: 1 local and 1 remote commits diverge; use --mode merge or --mode rebase
[exit 0]
$ massorca git pull -w feature --mode merge
merged as a2167c17d848
[exit 0]
$ massorca git push -w feature
origin/feature: updated
[exit 0]
$ massorca git commit -w feature --amend -m "feat: merge test amended"
error: HEAD is already on origin/feature; amending rewrites published history (git.commit_published)
[exit 1]
$ massorca git commit -w feature --amend --allow-published -m "feat: merge test amended"
[feature adf492ce2c8e] feat: merge test amended
[exit 0]
$ massorca git push -w feature                  # remote moved on
origin/feature: rejected (non-fast-forward)
[exit 0]
$ massorca git fetch -w feature                 # remote branch created by the other clone
new      origin/gone
[exit 0]
$ massorca git fetch -w feature                 # after the other clone deleted it
pruned   origin/gone
[exit 0]
```

Error paths, one or more per area (the usage text that clap prints after some
usage errors is omitted):

```text
$ massorca frobnicate
error: unrecognized subcommand 'frobnicate'
[exit 2]
$ massorca service bogus
error: unrecognized subcommand 'bogus'
[exit 2]
$ massorca project show no-such-project
error: no project named "no-such-project" (project.not_found)
[exit 1]
$ massorca project add
error: the following required arguments were not provided: <PATH>
[exit 2]
$ massorca workspace show no-such-ws
error: no workspace named "no-such-ws" (workspace.not_found)
[exit 1]
$ massorca terminal show 00000000-0000-0000-0000-000000000000
error: no such terminal (terminal.not_found)
[exit 1]
$ massorca terminal resize 00000000-0000-0000-0000-000000000000 --cols abc --rows 3
error: invalid value 'abc' for '--cols <COLS>': invalid digit found in string
[exit 2]
$ massorca git switch -w feature no-such-branch
error: branch `no-such-branch` does not exist (branch.not_found)
[exit 1]
$ massorca git log -w feature -n abc
error: invalid value 'abc' for '--limit <LIMIT>': invalid digit found in string
[exit 2]
$ massorca --json git switch -w feature no-such-branch
{"error":{"code":"branch.not_found","data":null,"message":"branch `no-such-branch` does not exist"},"ok":false}
[exit 1]
$ massorca ext
error: extensions arrive in milestone M6 (extension.not_implemented)
[exit 1]
```

### WSL

Shell for the scenario: bash in Ubuntu 24.04. Trimmed to the meaningful lines.

```text
$ git init <tmp>/repo && commit README.md   # placeholder identity
3088785 chore: initial commit

$ massorca service status
not running
[exit 0]
$ massorca --no-autostart project list
error: the service is not reachable: No such file or directory (os error 2) (service.unavailable)
[exit 4]
$ massorca project list
no projects
[exit 0]
$ massorca service run                 # second service, same profile
error: another service already runs this profile (service.already_running)
[exit 3]

$ massorca project add <tmp>/repo --git --workspaces-root <tmp>/ws
added project repo (git) at <tmp>/repo
$ massorca project add <tmp>/notes --folder
added project notes (folder) at <tmp>/notes
$ massorca workspace create repo feature
created workspace feature (worktree) at <tmp>/ws/feature
$ massorca workspace create notes main
created workspace main (folder) at <tmp>/notes
$ massorca workspace create repo wsdirty
created workspace wsdirty (worktree) at <tmp>/ws/wsdirty
$ massorca workspace rm wsdirty
error: <tmp>/ws/wsdirty has 1 uncommitted and 1 untracked change(s); remove with force to discard them (workspace.dirty)
[exit 1]
$ massorca --json workspace rm wsdirty
{"error":{"code":"workspace.dirty",...},"ok":false}
[exit 1]
$ massorca workspace rm wsdirty --force
removed workspace wsdirty
deleted branch wsdirty
[exit 0]

$ massorca project rm repo
error: project "repo" still has workspaces; remove them first (project.has_workspaces)
[exit 1]
$ massorca service restart
pid: <pid>
[exit 0]
$ massorca project list          # both projects still listed
$ massorca workspace list        # feature and main still listed
$ massorca workspace rm main
removed workspace main
kept directory <tmp>/notes
[exit 0]

$ massorca terminal create feature --cols 100 --rows 30 --title m1-term
id: 01a2011b-68cd-7114-956e-ef2209535c83
state: live
shell: /bin/bash
size: 100x30
pid: <pid>
[exit 0]
$ massorca terminal send <id> echo M1-HELLO-1
$ massorca terminal screen <id>
root@<host>:<tmp>/ws/feature# echo M1-HELLO-1
M1-HELLO-1
root@<host>:<tmp>/ws/feature#
$ massorca terminal resize <id> --cols 120 --rows 40
size: 120x40
$ massorca terminal attach <id> < /dev/null
error: attach needs a terminal on stdin and stdout; `terminal screen` prints a screen without one (usage.not_a_terminal)
[exit 2]

# outlive test: attach client A under script(1), a pseudo-terminal sized 120x40
# (captured output shown; prompt host name removed)
--- client A snapshot on attach:  ...echo M1-WHILE-DETACHED, M1-WHILE-DETACHED, ...
$ massorca terminal send <id> echo M1-WHILE-ATTACHED
--- client A output after live input:  echo M1-WHILE-ATTACHED / M1-WHILE-ATTACHED
client A ended by PID <pid-A> (kill -9)
service after client A ended: pid: <pid-service>        # unchanged
$ massorca terminal send <id> echo M1-WHILE-DETACHED
M1-WHILE-DETACHED                                       # output while detached: the shell is alive
# client B reattached under script(1), pid <pid-B>; shell start ticks read from /proc before and after
--- client B snapshot on reattach: M1-HELLO-1, M1-WHILE-ATTACHED and M1-WHILE-DETACHED are in the captured output
$ massorca terminal send <id> echo M1-AFTER-REATTACH
--- client B output after new input:  echo M1-AFTER-REATTACH / M1-AFTER-REATTACH
client B ended by PID <pid-B>; shell start ticks unchanged
$ massorca terminal send <id> echo M1-POST-DETACH
M1-POST-DETACH

$ massorca terminal kill <id>
state: exited
exit: killed, signal Hangup
[exit 0]
shell <pid-shell>: gone
$ massorca terminal create feature --title m1-live ...    # live terminal, shell <pid-shell-2>
$ massorca service restart
pid: <pid-service-2>
shell <pid-shell-2>: gone at t+0 s, t+1 s, t+3 s
$ massorca terminal list
no terminals
[exit 0]
```

Git and remote checks (the same sequence as Windows, output identical except
commit ids and timings):

```text
$ massorca git status -w feature       # ' M README.md +1 -0' and '?? a.txt'
$ massorca git discard -w feature README.md
error: these files have only staged changes; pass include_staged to drop them: README.md (git.discard_staged)
[exit 1]
$ massorca git discard -w feature a.txt
error: discarding these deletes files git has no copy of; pass delete_untracked: a.txt (git.discard_untracked)
[exit 1]
$ massorca git commit -w feature --amend -m "feat: add b (amended)"
[feature ebb88cc02349] feat: add b (amended)
$ massorca git branch-delete -w feature throwaway
error: branch `throwaway` is not merged; pass force to delete it (branch.unmerged)
[exit 1]
$ massorca git switch -w feature other
error: the checkout would overwrite local changes: README.md (git.checkout_would_overwrite)
[exit 1]

# git watch: launch to watching line 512 ms; edit 3 s later
watching
changed: worktree
[exit 0]

$ massorca git push -w feature
origin/feature: created, upstream set
$ massorca git pull -w feature
fast-forwarded ebb88cc02349..b125bafb0891
$ massorca git pull -w feature
not pulled: 1 local and 1 remote commits diverge; use --mode merge or --mode rebase
[exit 0]
$ massorca --json git pull -w feature
{"ok":true,"result":{"ahead":1,"behind":1,"outcome":"diverged"}}
$ massorca git pull -w feature --mode merge
merged as 34b41e18fb79
$ massorca git push -w feature
origin/feature: updated
$ massorca git commit -w feature --amend -m "feat: merge test amended"
error: HEAD is already on origin/feature; amending rewrites published history (git.commit_published)
[exit 1]
$ massorca git commit -w feature --amend --allow-published -m "feat: merge test amended"
[feature a318d01f547a] feat: merge test amended
[exit 0]
$ massorca git push -w feature
origin/feature: rejected (non-fast-forward)
[exit 0]
$ massorca git fetch -w feature
new      origin/gone
$ massorca git fetch -w feature
pruned   origin/gone
[exit 0]
```

Error paths:

```text
$ massorca frobnicate                  error: unrecognized subcommand 'frobnicate'   [exit 2]
$ massorca service bogus               error: unrecognized subcommand 'bogus'        [exit 2]
$ massorca project show no-such-project  error: no project named "no-such-project" (project.not_found)  [exit 1]
$ massorca project add                 error: the following required arguments were not provided: <PATH>  [exit 2]
$ massorca workspace show no-such-ws   error: no workspace named "no-such-ws" (workspace.not_found)  [exit 1]
$ massorca terminal show 00000000-...  error: no such terminal (terminal.not_found)   [exit 1]
$ massorca terminal resize ... --cols abc   error: invalid value 'abc' for '--cols <COLS>'  [exit 2]
$ massorca git switch -w feature no-such-branch   error: branch `no-such-branch` does not exist (branch.not_found)  [exit 1]
$ massorca git log -w feature -n abc   error: invalid value 'abc' for '--limit <LIMIT>'  [exit 2]
$ massorca --json git switch -w feature no-such-branch  {"error":{"code":"branch.not_found",...},"ok":false}  [exit 1]
$ massorca ext                         error: extensions arrive in milestone M6 (extension.not_implemented)  [exit 1]
```

## Cleanup

Done before this report was committed:

- Services: the Windows service started for the scenario was stopped with
  `massorca service stop` and its process id was confirmed gone; the WSL service was
  stopped the same way. `service status` reports not running on both. On WSL, no
  process still had the test profile in its environment; on Windows, no `massorca`
  process from this build was left running. The attach clients and the stdin feeders
  of the WSL attach tests were ended by process id.
  `git watch` processes ended after their event or on their timeout. Services and
  console processes of other sessions on the same machine were not touched.
- Removed, after checking for junctions and symlinks (none were found, so nothing
  was followed): the Windows scenario root (repositories, bare remote, profile); the
  Windows test homes of both full `cargo test` attempts (each rerun removed its own);
  the two test temp directories
  that the failing `terminal_bridge` runs left behind, identified by the names in
  their logs; the WSL scenario root, the WSL test home, the WSL clone, the WSL target
  directory and the WSL build log; the Windows target directory of this run.
- Not removed: four other `.tmp*` directories in the Windows temp folder. Their
  creation times (18:44 to 21:49) and the start time of the service in one of them
  show that they were created before the second full test run of this validation
  began, so they were not attributed to it. Also not removed: the shared cargo caches
  and the WSL Rust toolchain, which were already present.
- The git worktree used for this validation is removed after the PR branch is
  pushed. The branch `docs/m1-report` stays as the PR head.
