# Local git and source control (GIT)

This inventory covers the local git side of the current MassOrca product: how the core runs the user's own git binary; the working-tree view of a workspace (status, file diffs, staging, discarding, committing); branches and checkout; remote synchronisation (push, pull, fetch, fast-forward, rebase onto the base branch, fork sync); the lifecycle of git worktrees that serve as workspaces (create from a base branch, list, remove, keep branches); and AI-written commit messages. Each operation runs on an execution host: the local machine, a WSL distribution on Windows, or an SSH host through a relay. Hosted review providers (pull requests, issues, forge APIs), terminals, agent sessions, the renderer and the mobile client are outside this domain and appear here only as dependencies.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits).

## Source directories read

Paths are repo-relative to the reference repository. The pv96 delta was read with read-only git against commit `d3c6311a531`; the base reading of custom/integration still applies to files pv96 did not change.

- `src/main/git/` (about 150 non-test files; key files read, including `command-runner/`)
- `src/main/git/source-control/` (status, staging, commit, discard, diffs, conflict state, staged-commit context)
- `src/main/text-generation/` (commit-message and source-control generation files only)
- `src/main/ripgrep/` (searched only; it reuses the git WSL spawn helper but has no git feature of its own)
- `src/main/git-bash.ts` (root file starting with `git`; `git-bash.test.ts` read by name only)
- `src/main/ipc/filesystem/` (`git:` channel names), `src/main/ipc/filesystem/git-remote/`, `src/main/ipc/repos/` (clone, init, git availability), `src/main/ipc/worktree-remote.ts` and `src/main/ipc/worktrees/` (selected lines)
- `src/main/runtime/rpc/methods/` (`git*.ts`, RPC method names) and `src/main/runtime/` (worktree create and removal files, selected lines)
- `src/main/providers/` (`git-provider-contract.ts` and the `ssh-git-*.ts` class structure)
- `src/shared/` (git helper modules and source-control settings types, selected files)
- pv96 delta, added: `src/main/git/` and `src/main/git/command-runner/` (changed-file check: none changed in pv96); `src/main/runtime/runtime-git-sync-commands.ts`, `orca-runtime-git.ts` and `runtime-git-command-surface.ts` (the internal `openRuntimeGitWorktree` only); `src/main/delegated-session/child-bring-back.ts`, `child-bring-back-automatic.ts` and `src/shared/child-bring-back.ts` (bring-back merge, conflict prediction and policy); `src/main/runtime/rpc/methods/repo.ts` and `src/main/runtime/runtime-rpc/runtime-rpc-mobile-method-allowlist.ts` (`repo.initGit` on the phone allowlist); `src/shared/protocol-version.ts` (capability names); `src/main/text-generation/source-control-text-generation-requests.ts` (internal run record); `src/main/marketplace/host-programs/host-program-catalog.ts` (Git install methods per platform); `src/main/ipc/repos/repo-ipc-arg-schemas.ts`; `src/renderer/src/components/sidebar/NonGitFolderDialog.tsx` and `worktree-list/rows/FolderProjectGitMarker.tsx` (file names); `odd/tasks/no-git-folder-warning.md` (the init path). The other odd docs had no git-specific items.

## Not read / low confidence

- Renderer and mobile UI: not read by this worker. Searched only to confirm there is no stash feature. The commit-message sub-review read a few renderer files by reference (commit box, settings pane).
- Hosting providers (`src/main/github`, `gitlab`, `gitea`, the hosted-review files in `source-control/`): out of domain.
- Relay internals (`src/relay`) and SSH transport (`src/main/ssh`): named only. SSH providers were read by class structure.
- WSL routing: names and a few comments read; not traced end to end.
- Worktree operations were mapped by a read-only sub-review. Spot-checked: the default of the archive hook on the runtime removal path, SSH skipping shared paths, the squash-merge rule before a branch delete, and the SSH removal path. Not read: `worktree-listing.ts`, `worktree-scan-cache.ts`, `worktree-base-divergence.ts`, `worktree-preparation-add.ts`, `worktree-preparation-tip-refresh.ts`, `local-worktree-removal-recovery.ts`, `worktree-removal-records.ts`. Figures from the sub-review that were not individually re-checked (base-refresh wait budget, base-fallback reporting, pool limits, sparse cache lifetime) are marked as such where they appear.
- Commit-message generation was mapped by a second sub-review. Verified: the 60-second timeout, the 40 KiB output cap, the 72-character subject rule with its fallback, the argument budgets, the 200,000-byte staged-patch budget, and the caller-supplied settings path. One figure from that sub-review was wrong (it said 4 MiB; the code value is 40 KiB) and was corrected. Not verified: agent and model precedence (taken from the sub-review), lane sharing between discovery and generation, the Codex home lock, per-agent argument details.
- Not read in full: `shared/git-history.ts` (limit and has-more behaviour only), `shared/git-fork-sync.ts`, the submodule modules, the admission scheduler, pack-maintenance internals, the sparse-checkout code, the previewable binary type list, and the path-safety helper used by discard.
- The Git version floor comes from code comments (a 2.25 baseline; newer flags have fallbacks). No explicit minimum-version refusal was found in the files read.
- Tests were skipped except where names were informative.
- The reference repository has no `.codegraph/` index. It is read-only, so no index was created. The base pass ran no git command there; the pv96 delta ran only read-only git with explicit refs. Reading used Read, Grep, Glob and read-only shell commands.
- pv96 delta, not read or low confidence: the bring-back merge was read in outline only (the conflict and agent-resolving paths and the automatic state machine); nothing was run.
- pv96 delta: the Git install route through the Marketplace (the commands run and any privilege prompt) was not traced; only the catalog rows were read.
- pv96 delta: the No Git dialog, popover and phone sheet come from the odd doc and file names; their renderer code was not read.
- pv96 delta: `openRuntimeGitWorktree` is an internal runtime call; no RPC method or CLI command exposing it was found.

## How to read the proposals

Sizes (S, M, L, XL) are for reimplementing the behaviour. First-version marks follow the plan's rule: the smallest set that makes daily agent work in workspaces with terminals and git possible. Twelve features were proposed for the first version before pv96; pv96 adds GIT-30 (bring-back) as a first-version feature. The table rows are in the scratch tables file for the README assembly.

## Features

### GIT-01 Git availability and version baseline

- Source: pv96 (unmerged). The Marketplace can install Git on this computer in one click (GIT-31); the detection rules are unchanged.

What happens: when a host is used for git work, the core checks that a git binary runs there. If the spawn proves the binary absent, the answer is "missing". Any other failure, including a missing working directory, leaves the answer "unknown" rather than claiming git is absent. The repository-creation flow asks the same question. The app does not bundle git; it uses the user's install.

Rules and edge cases:
- Results are cached in memory per execution host: the local host, each WSL distribution, and each SSH connection. The local cache is bounded to 128 hosts.
- The code comments state Git 2.25 as the baseline. Newer features are used through capability fallbacks: a fetch flag is retried without itself on older git; a config-by-environment protocol needs 2.31; an absolute path format needs 2.31 and has an older-git fallback.
- On Windows the runner refreshes the process PATH before launching git while the host environment is still loading. For the terminal shell, Git Bash is located from standard install roots taken from environment variables (`git-bash.ts`).

Requests and pushback: "is git available" for a host. The answer is available, missing or unknown.
Persisted: nothing; in-memory cache only.
Depends on: GIT-02 (runner), GIT-28 and GIT-29 (host routing).

### GIT-02 Non-interactive git runner and credential policy

What happens: every git command the app starts runs non-interactively, so a credential or host-key question fails with an error instead of waiting for input. This matters because on a headless server a stuck prompt blocked every client.

Rules and edge cases:
- Terminal prompting is disabled (GIT_TERMINAL_PROMPT). Askpass helpers (GIT_ASKPASS, SSH_ASKPASS) are emptied unless the caller supplied one.
- For SSH transport, when no SSH command is set, the policy uses batch mode, so interactive questions are refused. Before that default, the core probes the user's configured SSH command for git with a short timeout (2.5 seconds; 10 seconds under WSL). A value set by the caller is used as is.
- The core configures no credential helper. HTTPS credentials come from the user's existing git setup; if none is available, the operation fails with git's own message.
- Output is pinned to untranslated English so that parsers work for any user locale. Interactive terminals keep their own locale.
- A helper sets optional locks off (GIT_OPTIONAL_LOCKS=0), which makes git skip opportunistic index writes. Where it is applied was not traced.
- Each git call has a timeout and a capped buffer. Mutating operations on a worktree (pull, fast-forward, rebase from base) take a per-worktree operation lock. Concurrent git processes pass through an admission scheduler with tiers; the environment variable ORCA_GIT_ADMISSION_DISABLED=1 turns it off.
- On Windows the runner sets WSL_UTF8 for wsl.exe so its failures are readable, and forwards the SSH command into WSL only when the app set it.

Requests and pushback: all git requests can carry a cancel signal; a cancelled operation stops the child process tree.
Persisted: nothing.
Depends on: GIT-01.

### GIT-03 Repository detection and registration

What happens: the user adds a folder. The core asks git whether the folder is inside a work tree and whether it is bare. A clean negative answer means a plain folder workspace; a clean positive answer registers a repository. If git's answer is inconclusive, the core looks for a valid .git marker and accepts the folder if one is found, logging one warning per session.

Rules and edge cases:
- A path inside a repository is normalised to the repository root. A linked worktree resolves to its main repository root.
- Registration distinguishes the working root, the git directory and the common directory. Linked worktrees share one common directory.
- A canonical repository key identifies a repository across path spellings. The key logic accounts for a git 2.31 path-format difference.
- Plain folders never run git.

Requests and pushback: add a folder. The core answers with kind (git or folder) and the root path.
Persisted: repository record (path and kind) in the persistence domain.
Depends on: GIT-01, GIT-02.

### GIT-04 Clone and initialize repositories

- Source: pv96 (unmerged). Turning a folder into a git repository is now also offered from the folder warning, the No Git marker and the phone sheet (WS-31, WS-32), and the phone method allowlist includes `repo.initGit`.

What happens: the user can clone a repository from a URL into a chosen destination, or create a repository in a folder with git init. Cloning derives the repository name from the URL, validates the destination, and claims the target folder. If the clone fails, the claimed folder is cleaned up and the failure is mapped to a readable message.

Rules and edge cases:
- A saved clone target can be reused (saved-clone-target logic).
- Creating a repository of kind git runs git init in the new folder. Kind folder creates no git state.
- On a remote SSH host, initialization runs through that host's git provider.
- Clone can use a configured hosted account for authentication arguments. That touches the hosting domain.

Requests and pushback: clone (URL, destination); create repository (kind). Pushback: a readable clone failure message.
Persisted: repository record; clone target claims are transient.
Depends on: GIT-02, GIT-03; hosted accounts (hosting domain).

### GIT-05 Status with line counts and listing caps

What happens: the workspace shows its changed files, grouped as staged, unstaged and untracked, with added and removed line counts per file. The list refreshes on a poll and after mutations.

Rules and edge cases:
- The listing is capped at 1,000 entries by default. A flag reports when the cap was hit. A cap of zero disables it. When the cap is hit, the client can offer to ignore well-known dependency folders (GIT-24).
- Shared symlinks that the app created for dependency folders are removed from the untracked list. Git would otherwise report them as permanently untracked.
- Status reads are read-only. Identical concurrent reads share one git run. Mutations clear cached reads before and after they run.
- A status request can be cancelled by its token.
- Parsing uses porcelain version 1 records.
- Branch and upstream totals are optional and computed only when the caller asks (GIT-19).

Requests and pushback: status for a worktree, with options (line stats, ignored paths included, limit, shared link paths). Pushback: entries, totals and the cap flag.
Persisted: nothing; in-memory caches only.
Depends on: GIT-02, GIT-18 (shared links), GIT-19, GIT-24.

### GIT-06 File diffs for unstaged, staged and HEAD comparisons

What happens: the user opens the diff of a changed file in one of three modes: unstaged (index against worktree), staged (HEAD against index), or the file against HEAD. The result carries original and modified content, with markers for binary or unreadable content.

Rules and edge cases:
- Content is read from the index, from a HEAD object id, or from the worktree file. A blob read that fails to prove absence marks the result as not reusable, so it is not cached.
- Binary types from a fixed list get a preview instead of a text diff.
- A submodule path produces a pointer diff and, when both commits are known, an inner commit range diff (GIT-23).
- Results are cached against a stamp of git state taken before the read. Anything that changes during or after the read makes the cached entry miss. Identical concurrent requests coalesce.
- A staged diff does not depend on the working tree.

Requests and pushback: diff for a file, with a staged flag and a compare-against-HEAD flag. Pushback: the diff result.
Persisted: nothing.
Depends on: GIT-05, GIT-23.

### GIT-07 Stage and unstage files

What happens: the user stages one file, a selected set, or everything; or unstages a file or a set.

Rules and edge cases:
- Paths are passed as literal pathspecs, so glob characters in file names are not expanded.
- Bulk operations send their path list through standard input with NUL separators, which avoids argument length limits.
- Unstage uses a reset that also works before the first commit (an unborn HEAD), keeping the working file.
- When the capped listing cannot name every change, stage-all runs an add-all but excludes the app's own shared symlinks, so a machine-specific link never reaches a commit.
- Each mutation clears cached reads before and after.
- Not offered: partial staging by hunk or line (see the absence list).

Requests and pushback: stage, unstage, bulk stage, bulk unstage, stage-all with a scope. Pushback: success or git's error text.
Persisted: the git index only.
Depends on: GIT-05.

### GIT-08 Discard changes

What happens: the user discards a file's changes or deletes an untracked file, one at a time or in bulk. There is no undo and no trash.

Rules and edge cases:
- A path that resolves outside the worktree is refused.
- A tracked file is restored from the index into the worktree. An untracked file is deleted through a path-safety helper that refuses unsafe targets.
- Confirmation belongs to the client. The core performs the action it is asked to perform.

Requests and pushback: discard one file; bulk discard. Pushback: success or error.
Persisted: nothing beyond the working tree.
Depends on: GIT-05, GIT-07.

### GIT-09 Commit with a typed message

What happens: the user writes a message and commits what is staged. Repository hooks run as usual.

Rules and edge cases:
- The commit call takes only the message. The core exposes no amend, no hook bypass, no signing option and no author override (see the absence list).
- On failure, the returned text prefers git's standard error (hook and signing failures appear there), then standard output ("nothing to commit" appears there), then a generic message.
- The core does not pre-check an empty staged set in this path; git reports it.

Requests and pushback: commit with a message. Pushback: success, or readable error text.
Persisted: git history only.
Depends on: GIT-02, GIT-07. Messages can be generated by GIT-10.

### GIT-10 AI-generated commit messages

- Source: pv96 (unmerged). Local commit-message generation runs are recorded as internal agent runs, so Session History can hide them as automated; remote runs are not recorded.

What happens: in the commit box the user generates a message from the staged changes. With defaults configured, generation starts at once; otherwise a dialog asks for the options. Only one generation runs per worktree at a time. The user can cancel, which stops the agent process.

Input:
- The branch name, a capped summary of staged files, and the staged patch within a 200,000-byte budget. The patch is split across files and clipped at line boundaries. If the patch is too large, a placeholder is sent instead.
- Working-tree edits that are not staged are not sent.
- Custom instructions are folded into the prompt template. A template in settings takes precedence over free text.

Agent and model:
- A set of built-in command-line agents has commit-message specifications; the sub-review counted fourteen. A custom command template is also supported.
- Agent precedence (from the sub-review): the action-specific agent, then the source-control agent setting, then the default TUI agent if enabled, then Claude if enabled. An enabled default without a commit specification is an error, not a silent fallback.
- Some agents take the prompt on standard input, others as an argument. Argument-delivered prompts are checked before spawn against a Windows command-line budget (30,000 UTF-16 units) and a Linux per-argument budget (120 KiB).
- Models are discovered by running each CLI's list command, with a 60-second timeout and a 40 KiB output cap. Some agents use fixed model lists. A stored model that is absent from every list silently becomes the specification default.

Output and failures:
- Output is cleaned: line endings normalised, JSON envelopes and reasoning blocks removed, a surrounding code fence removed, list markers stripped.
- The subject is the first line, without trailing periods, cut at 72 characters. An empty subject becomes "Update project files". The prompt asks for imperative mood; the code does not strip trailers.
- Failures: a non-zero exit, empty output, the 60-second timeout, or output above the cap. Captured agent output is returned for branch and conversation naming, but not for commit generation.

Routing: the local host; a WSL distribution (with a fixed set of forwarded Windows environment keys); or an SSH host through the relay, where requests are queued and cancellation is sent to the host.

Requests and pushback: generate (worktree, options), cancel, discover models. Pushback: messages for no staged changes, for an unreadable staged set, and for a missing or unsupported agent.

Settings: the source-control AI settings hold the enabled flag, actions, the default agent, selected and discovered models per agent and per host, the thinking level per model, the custom command, instructions, operation overrides and pull request defaults. Repository overrides apply only to worktrees of that repository. No stored generated messages were found.

Security note for the owner: the generation request can carry already-resolved settings, including a custom command template and an agent command override. When these are present, the runtime uses them without re-resolving the user's settings. Whether clients may supply command templates is an open decision.

Depends on: GIT-09, GIT-07, GIT-02; agents domain; settings domain.

### GIT-11 Branch list and checkout

What happens: the user lists local branches, the current one first, and switches the workspace to an existing local branch.

Rules and edge cases:
- Empty branch names and names that start with a dash are refused before git sees them.
- The checkout does not force. If uncommitted changes would be overwritten, git refuses and its message reaches the user.
- The checkout path takes a local branch name. No path that creates a tracking branch from a remote was found.
- Branches are created only inside worktree creation (GIT-13), first-work rename (GIT-12) and pre-warmed checkouts (GIT-17).

Requests and pushback: list local branches; checkout a branch. Pushback: git's message.
Persisted: the checked-out branch in git.
Depends on: GIT-02, GIT-05.

### GIT-12 Rename branch and folder after first agent work

What happens: when an agent starts work in a fresh worktree, the app can rename the current branch to a name derived from that work, and move the worktree folder.

Rules and edge cases:
- The rename uses a unique-name search: the base name first, then numbered suffixes from 2 up to 100. The current branch counts as available. If the search finds no free name, nothing is renamed.
- The folder move uses git's worktree move command (reported by the sub-review) and refuses an existing target.

Requests and pushback: triggered by the agent-hook path; no client request. Pushback: none beyond the resulting branch and folder names.
Persisted: branch name in git; worktree folder path in workspace metadata.
Depends on: GIT-11, GIT-13, GIT-14, agents domain (first-work hook).

### GIT-13 Create a worktree from a base branch

What happens: the user creates a workspace. The core creates a new worktree folder and a new branch cut from a base ref, normally the repository's default base branch.

Rules and edge cases:
- Branch naming: the requested name can be prefixed by a git login. Sources are checked in order: explicit config keys, the hosted-account login (hosting domain), then the git user name when it is a branch-safe token. Name clashes are skipped and another candidate name is tried (reported).
- The branch is created with no upstream tracking, and the base is recorded in the branch configuration so later comparisons know it.
- Existing branches: if the name matches an existing local branch and the caller allows reuse, that branch is checked out instead. Such worktrees keep their branch when removed (GIT-15).
- Base refresh (reported by the sub-review): a remote-tracking base is fetched before the add, with a 30-second wait budget; a refresh still running after that continues in the background. A base missing locally gets a best-effort origin fetch. If the refresh fails, a same-named local branch is used only when the caller allows local fallback, and the result reports that fallback.
- The local base branch is fast-forwarded only when that is possible and the worktree that owns it is clean. The setting that controls this refresh defaults to off (reported).
- A push target can point the worktree at a fork remote (GIT-19, GIT-26).
- Folder workspaces never run git.
- Over SSH, creation requires a registered SSH git provider. A missing remote-tracking base falls back to the same-named local ref. Shared paths and copies are not applied (GIT-18).

Persisted per worktree: base ref, creation source (local or SSH), creator provenance, push target, a preserve-branch flag for reused branches, and sparse settings (GIT-18).
Requests and pushback: create (repository, name, base, options). Pushback: the created worktree; a base-fallback flag; a removing state while a background removal holds the path; readable errors.
Depends on: GIT-03, GIT-14, GIT-19, GIT-02; GIT-18 as an option.

### GIT-14 Worktree listing and scanning

What happens: the workspace list shows every linked worktree git knows for a repository, with locked and prunable state.

Rules and edge cases:
- Listing uses git's porcelain output, with a fallback parser.
- Older git versions do not report prunable entries, so the core probes each linked path for existence as a backstop.
- Scan results are cached with a time limit and invalidated by mutations. Creation and removal bump a scan generation.
- Remote catalogs refuse an empty fallback from an older relay (reported), so a failed listing is never shown as an empty list.

Requests and pushback: list worktrees for a repository. Pushback: the worktree entries with their flags.
Persisted: git holds the worktree registrations. Orca worktree identity metadata lives in the persistence domain.
Depends on: GIT-02, GIT-03.

### GIT-15 Worktree removal with safety checks

What happens: the user removes a workspace. The core stops work that depends on the folder, removes the git worktree, and keeps or deletes the branch according to rules.

Rules and edge cases:
- A locked worktree is refused. The repository root, the home folder and any path containing the repository are refused.
- Unless forced, a dirty check runs first: status including untracked files, with a 30-second timeout. Dirty work is refused.
- Before git removes the folder, the core stops terminal sessions in it and unlinks the app's shared paths.
- On desktop, native checkouts are deleted in the background and the call returns a removing state. WSL removals run inline (reported).
- A repeated identical removal joins the one in flight. A different removal for the same repository is refused while one runs.
- Branch handling: the branch is kept when the caller asks to keep it, or when the worktree was created from an existing branch. Otherwise the core runs a safe delete. A forced delete is used only in a failed-creation rollback (reported). A safe delete that git refuses can still succeed when git proves that no unmerged tree changes exist, because squash merges rewrite commit ids. Otherwise the branch is kept and recorded as preserved (GIT-16).
- Archive hook: a per-repository step runs before removal on the desktop path unless the caller skips it. On the runtime path it runs only when explicitly requested, and by default it does not run.
- Unpushed commits: the removal path does not check for unpushed commits. A separate cleanup preflight counts commits not reachable from any remote.
- Orphan and leftover directories: deletion requires Orca provenance (a creation time and source, or legacy fields) and a .git file that proves the folder belongs to a worktree. Without force, leftover directories raise an error.
- Windows: junction leftovers are removed on the host side (reported). Long-path support is enabled for worktree commands.
- Over SSH: the desktop path runs the dirty check unless forced. The runtime path has no dirty check and relies on git refusing removal of a dirty worktree without force. Removal with the archive hook is refused on SSH.

Requests and pushback: remove (worktree, options: force, keep branch, skip archive). Pushback: refusals with reasons; a removing state; errors.
Persisted: removal of Orca worktree metadata and of the git worktree; a preserved-branch record when the branch is kept.
Depends on: GIT-14, GIT-16; terminals and agents domain (session stop); workspace domain (archive hook, folders).

### GIT-16 Preserved branches and force-delete

What happens: a kept branch is recorded with its head commit, so it can be deleted later from the cleanup UI.

Rules and edge cases:
- A forced delete happens only when the branch still points at the saved commit (a compare-and-set update). The core refuses a branch that is checked out in another worktree (reported).
- Over SSH the force-delete needs a newer relay. An older relay reports it unavailable.

Requests and pushback: force-delete a preserved branch. Pushback: refusal with reason, or success.
Persisted: preserved-branch record (name and head) in workspace metadata.
Depends on: GIT-15.

### GIT-17 Pre-warmed worktree checkouts

What happens: for local repositories, the app can prepare a few locked checkouts in advance. A later creation then finalises faster by moving and renaming a prepared checkout instead of adding one from scratch.

Rules and edge cases (reported by the sub-review):
- Pool limits in the code: up to three prepared entries, each with a five-minute time limit.
- Only plain creation uses the pool (no existing-branch or sparse options).
- The preparation lock is written as git's lock marker with a reason, and is re-verified at each step.

Proposed as dropped: this is a latency optimisation of the current design. Measure creation latency before carrying it over.
Requests and pushback: none directly; it changes creation latency.
Depends on: GIT-13.

### GIT-18 Sparse checkout and shared paths for new worktrees

What happens: when creating a worktree, the user can pick a sparse checkout (a preset or a list of directories). The app can also link or copy selected paths into the new worktree.

Rules and edge cases:
- Sparse checkout uses cone mode: the worktree is added without checkout, sparse patterns are set, then checkout runs. If setup fails, the new branch and its base configuration are removed. If that cleanup also fails, the user is told a manual removal may be needed (reported).
- Sparse state is detected from the sparse pattern file and the config flag, because disabling sparse checkout keeps the pattern file. Sparse detection results are cached for five minutes (reported).
- Three kinds of shared path: per-user shared paths (a symlink; an APFS clone on macOS where possible; junctions tried first on Windows); repository-level shared directories from the orca configuration (always linked; must exist and be gitignored); and project include files (copied; literal paths only; globs and negations are skipped with a warning; a copy budget applies). The three kinds are reported by the sub-review; the Windows and macOS details were not re-checked.
- Over SSH, none of the three applies (verified in code: the path is skipped with a stated reason).
- Shared links are excluded from status and stage-all (GIT-05, GIT-07).

Persisted: sparse directories, sparse base ref and preset identifier on the worktree; shared path settings in repository configuration.
Requests and pushback: options on create; pushback: setup errors with cleanup status.
Depends on: GIT-13, GIT-05, GIT-07.

### GIT-19 Push, pull, fetch and upstream ahead or behind

What happens: the user pushes the current branch, pulls, fetches, or fast-forwards, and sees how far the branch is ahead of or behind its upstream.

Rules and edge cases:
- Push goes to the configured upstream remote and branch, so a worktree that tracks a contributor fork pushes there. If no upstream exists, the first push creates the remote branch on origin and sets it as upstream.
- An optional force-with-lease push is part of the push contract for clients.
- Push targets are validated (branch name format) before use.
- Pull runs git's configured pull strategy (merge by default), so divergent branches reconcile. Conflicts are left for GIT-21. For a legacy branch whose upstream differs from its name, the pull uses the upstream the client displays.
- Fast-forward is a pull restricted to fast-forward merges.
- Fetch runs with prune. It pauses idle ref packing while it runs (GIT-25). Fork remotes are configured not to follow tags, and stale branch refspecs are repaired during fetch (GIT-26).
- Push and pull errors are normalised into readable messages.
- Upstream status reports ahead and behind counts against the effective upstream, with caches and a negative cache for unknown upstreams. A client can watch the upstream ref so the status refreshes.

Requests and pushback: push, pull, fetch, fast-forward, upstream status. Pushback: counts, or readable error text.
Persisted: upstream configuration in git; the push target on the worktree (GIT-13).
Depends on: GIT-02, GIT-03, GIT-13.

### GIT-20 Rebase worktree onto base branch

What happens: the user brings a worktree branch up to date with its base by rebasing onto the base.

Rules and edge cases:
- The base is fetched into a private temporary ref, so concurrent fetches cannot change the source during the rebase.
- The fork point between HEAD and the fetched base is computed first, and the rebase runs from that point onto the fetched base. If HEAD has no commits yet, the operation is a fast-forward merge instead.
- The temporary ref is deleted at the end, including after a failure.
- The operation holds the worktree's operation lock. A conflict leaves the rebase in progress for GIT-21.
- Only the non-interactive form exists. There is no interactive rebase.

Requests and pushback: rebase from base. Pushback: readable error text.
Persisted: none; the branch history changes in git.
Depends on: GIT-19, GIT-21, GIT-13.

### GIT-21 Merge and rebase conflict state and abort

What happens: the workspace reports that a merge, a rebase or a cherry-pick is in progress. The user can abort a merge or a rebase.

Rules and edge cases:
- Detection uses git's state files: the merge head for merges, the cherry-pick head for cherry-picks, and the rebase state directories for rebases. Detection does not rely on the single rebase head file, which can linger.
- Abort is available for merge and rebase only.
- Not offered: continue, skip, per-file resolution, a merge tool, or ours and theirs choices. Conflicts are resolved by editing files, then staging and committing with GIT-07 and GIT-09.
- A cherry-pick in progress is reported but has no action.

Requests and pushback: conflict operation for a worktree; abort merge; abort rebase. Pushback: the operation kind; success or error.
Persisted: none.
Depends on: GIT-05, GIT-19, GIT-20.

### GIT-22 History, branch and commit comparison

What happens: the user sees a log of recent commits for the workspace, the changes a branch carries relative to its base, or the changes in one commit.

Rules and edge cases:
- History returns up to a limit, with a flag that says more commits exist. Commit references are grouped by category.
- Branch comparison uses the base recorded on the worktree (GIT-13). Branch and commit diffs use the same diff reader as GIT-06.
- Cached comparison results are keyed to the compared object ids, so a moved branch or base is not served stale.

Requests and pushback: history (limit); branch compare; commit compare; branch diff; commit diff. Pushback: results and the has-more flag.
Persisted: none.
Depends on: GIT-06, GIT-13.

### GIT-23 Submodule status and pointer diffs

What happens: the workspace lists submodules and their status, and shows differences in a submodule pointer and in the submodule's own commit range.

Rules and edge cases:
- Submodule status uses the same listing cap as GIT-05.
- Pointer values are read from the index or the HEAD tree. The submodule's own worktree state is read from its path.
- A submodule diff is not cached when its inputs live in another repository.

Requests and pushback: submodule status; diff for a submodule path. Pushback: entries or a diff.
Persisted: none.
Depends on: GIT-05, GIT-06.

### GIT-24 Ignore helpers and huge-folder suggestions

What happens: the client can check which paths are ignored, get suggestions for well-known dependency folders that exist and are not ignored, and append a folder rule to the worktree's ignore file.

Rules and edge cases:
- The well-known folders are node_modules, .next, dist, build, target and vendor.
- Long path lists are checked through standard input, in chunks.
- Appending a rule accepts only a folder name from the known list: a single path segment, with no separators or line breaks. It never writes a duplicate line, so the ignore file cannot receive injected lines.

Requests and pushback: check ignored paths; find huge folders to ignore; append an ignore rule. Pushback: lists, or a write result.
Persisted: the worktree's ignore file. It shows up as a tracked file change in status.
Depends on: GIT-05.

### GIT-25 Idle ref packing and repository maintenance

What happens: for local repositories (native or WSL), the app packs loose refs when the workspace is idle, so large repositories stay responsive.

Rules and edge cases:
- Packing runs an all-refs pack with prune, under a timeout and a busy probe for the repository. User operations that take the packed-refs lock (fetch, pull) postpone it.
- Pack-index maintenance runs for repositories that have pack indexes.
- The check reads git's automatic maintenance configuration first; the exact interaction with packing was not traced.
- Repositories on SSH hosts are excluded. Their execution host lacks the same controls today.

Requests and pushback: none; background behaviour.
Persisted: packed refs in each repository (git data only).
Depends on: GIT-02, GIT-19.

### GIT-26 Fork sync and fork remote setup

What happens: in a fork workflow, the user syncs the fork's default branch with the upstream repository. Fork remotes are configured so that tags do not follow automatically.

Rules and edge cases:
- Fork sync takes an expected upstream and runs with a 60-second limit that the caller can cancel. The sync rules themselves were not read in full.
- Fork remotes are configured not to follow tags. Stale branch refspecs are repaired during fetch.

Requests and pushback: fork sync; fetch through a fork target. Pushback: a sync result, or a readable error.
Persisted: remote configuration in git; the push target on the worktree (GIT-13).
Depends on: GIT-13, GIT-19; the hosting domain for the fork concept.

### GIT-27 Remote file and commit web links

What happens: from a file or a commit, the user opens the same item on the hosted web interface.

Rules and edge cases:
- Links are built from the parsed remote URL, so only recognised hosts produce a link.
- The link format depends on the hosting provider. That logic belongs to the hosting domain.

Requests and pushback: remote file URL; remote commit URL. Pushback: the URL, or nothing for an unrecognised host.
Persisted: none.
Depends on: GIT-03; hosting domain.

### GIT-28 Git on SSH hosts through the relay

What happens: a repository on a remote SSH host is operated through a relay that runs git on that host. The client sees the same operations as for local repositories, through a git provider for each SSH connection.

Rules and edge cases:
- The provider classes are layered by capability: non-interactive reads, working-tree operations, remote synchronisation, review head, and worktree operations.
- Shared paths, symlinks and include copies are not applied (GIT-18).
- The archive hook is refused on removal when requested (GIT-15). The dirty-check behaviour differs between the desktop and runtime paths (GIT-15).
- Preserved-branch force-delete needs a newer relay (GIT-16). Older relays are refused for empty catalogs (GIT-14).
- Commit-message generation runs on the host through the relay queue (GIT-10).
- Whether the relay applies the same non-interactive environment on the host was not verified.
- Host-key verification and SSH connection handling belong to the remote domain.

Requests and pushback: the same as the local operations, with the differences above.
Persisted: git data on the host; in-memory provider caches on the client side.
Depends on: GIT-02, GIT-13, GIT-19; remote domain (SSH, relay).

### GIT-29 Git inside WSL distributions

What happens: a repository or worktree located in a WSL distribution is operated by git inside that distribution, while the Windows app drives it.

Rules and edge cases:
- Commands for a path under a WSL network path run in that distribution. Read-only commands can run without a login shell, once the distribution's git environment has been probed and cached.
- A linked worktree whose git directory points at a Windows path is operated by host git. One that points at a POSIX path is operated by the distribution's git. The route decision is cached for 30 seconds (reported).
- A fixed set of Windows environment keys is forwarded into WSL. The SSH command is forwarded only when the app set it.
- Worktree removal for WSL runs inline and skips the leftover-folder step (reported).
- Commit-message generation in a distribution uses that distribution's agent install (GIT-10).

Requests and pushback: the same as local operations, with the routing above.
Persisted: none new.
Depends on: GIT-01, GIT-02, GIT-13, GIT-15.

### GIT-30 Bring a delegated child's commits back into the parent branch

- Source: pv96 (unmerged). New: a finished delegated child that has its own git worktree can be brought back, which merges its branch into the parent's branch after predicting conflicts.

What happens: a child session that works in its own git worktree can be brought back to its parent. MassOrca merges the child's branch into the parent worktree's current branch with `git merge --no-ff`, using a message that names the child. Before merging, it predicts conflicts with `git merge-tree --write-tree` and reports the commits and files to bring back, the parent's commits ahead, and any uncommitted changes on either side. A conflicting merge is aborted and reported as conflicts; the conflicts can then be handed to the child's agent to resolve.

Rules and edge cases:
- Blocks stop the action: parent uncommitted changes, parent work in progress, or a parent with no branch.
- The policy comes from the parent session's access: `full-access` maps to full, `auto` to auto, and anything else to ask. Under ask, the person starts the bring-back. Under auto, MassOrca brings a child back by itself only when nothing blocks it and no conflict is predicted; otherwise the parent gets a notice. Under full, MassOrca brings a child back by itself, and a conflict goes to the agent to resolve.
- Automatic states are waiting, asked-commit and undone.
- A child in the parent's own worktree, or a child in a folder workspace, has no control.
- Git older than `merge-tree --write-tree` gets no conflict prediction.
- The git runner is per worktree, on the host that runs it, including SSH and WSL hosts.

Requests and pushback: internal to the delegation runtime, which receives a git runner for one worktree. No RPC method or CLI command was found.
Persisted: the merge commit in the parent branch; the automatic mark on the delegation request.
Depends on: GIT-13, GIT-19, GIT-21, agents domain (delegation).
Proposal: First version. Size M. Reason: orchestration is in the first version, so children write in their own worktrees and their commits need a way back.
- Owner direction for the new product (2026-11-12, child placement rules from the design session): with the child placement rules, writing children get their own worktree by default and parallel children always do, so this merge is the normal return path. A child that the user put in the current worktree, or a child in a plain folder, writes in place while the parent waits (rule 3) and has nothing to merge. Proposal unchanged.

### GIT-31 One-click Git install on this computer

- Source: pv96 (unmerged). New: the Marketplace can install Git on this computer with one click, through the platform's package manager.

What happens: the Marketplace host-program catalog lists Git with install methods per platform: winget on Windows (package `Git.Git`), Homebrew on macOS, and apt or dnf on Linux. The same catalog lists the GitHub CLI, Node and the agent CLIs. The catalog also has an uninstall method for Git.

Rules and edge cases: the core still does not bundle Git (GIT-01). After an install, GIT-01 detection decides whether git is available.

Requests and pushback: Marketplace host-program install and uninstall methods, per host; not traced further.
Persisted: nothing in MassOrca; the package manager records the install.
Depends on: GIT-01; the Marketplace (outside this pass).
Proposal: Later. Size S. Reason: a convenience; the first version can require Git to be installed by hand, which GIT-01 detects.

## Explicitly absent from the current product

Verified by search of the core git, source-control and text-generation modules and of the IPC and runtime layers. The stash check also searched the renderer; its hits were unrelated identifiers.

- Stash (save, list, apply, drop).
- Blame or annotate.
- Interactive rebase; rebase continue or skip; cherry-pick actions (cherry-pick is detected only).
- Commit amend; hook bypass; commit signing options; author override.
- Partial staging by hunk or line (no patch mode).
- A merge tool, and ours or theirs resolution.
- Reflog, bisect and revert. No reset-to-commit action was found; reset appears only inside the unstage path and the pre-warmed checkout reset.
- Tag operations: no tag subcommand was found in the git and source-control modules.

## Boundaries with other domains

- Hosted review providers (pull requests, review lookups, pull request field generation, issues, forge APIs): hosting domain.
- Agents (launch, accounts, agent CLI models), terminals (sessions in a worktree), and workspace policy (archive hook, folder workspaces, first-work hooks): their own domains. This domain covers only the git effects.
- Remote transport (SSH connection, host keys, relay protocol): remote domain.
- Settings persistence and migration: settings domain.
- The bundled ripgrep is not a git feature. It reuses the git WSL spawn helper.
