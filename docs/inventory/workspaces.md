# Workspaces, worktrees and projects: functional inventory (WS)

A **project** is the durable entry the user adds to MassOrca: a git repository or a plain folder. Its identity spans execution hosts (the local machine, an SSH host, a WSL distribution, a paired runtime), and the same repository on two hosts is two separate registrations. A **workspace** is the unit of agent work under a project. For a git project it is a git worktree with its own folder and branch; MassOrca's store adds a display name, links, status, lineage and provenance on top of what Git reports. For a folder project, or a chat that is not tied to a repository, it is a plain folder. Users create, list, rename, organise, sleep, remove, forget and clean up workspaces through one core whose rules are the same on every host. Git is the source of truth for which checkouts exist; the store holds identity and user-authored metadata. Removal is the destructive path. It is guarded by typed refusals (Git lock, dirty files, orphan or missing registration, terminals that cannot be proven stopped, running agent sessions, a failed archive hook), and every refusal except the Git lock can be waived by an explicit user choice. Removing a project from MassOrca never deletes files on disk.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits).

Method: static reading of the reference worktree only. The base pass read custom/integration. The pv96 delta used read-only git with explicit refs (diff stats, commit subjects, targeted diffs against custom/integration), the five named odd docs and selected files. Nothing was built, run or tested. The reference worktree has no `.codegraph/` index. The brief forbids writes there, so CodeGraph was not initialised and the reading used Read, Grep, Glob and read-only git.

## Source directories read

Paths are relative to the reference worktree `massorca-server-language-820c84` (read-only).

- `src/main/worktree-setup/`
- `src/main/project-groups/`
- `src/main/projectless-chats/`
- `src/main/orca-profiles/`: project transfer, move intents, profile index and storage paths, other-profile catalog, profile UI scope. Cloud-auth and organisation files were skipped as the accounts domain.
- `src/main/local-builds/`: headers only. It is the application build chooser, not workspace behaviour.
- `src/shared/worktree/`: types, metadata, create and removal types, ownership, visibility, lineage, base-ref drift, archive-hook gate, configured base path, name suggestion and retirement, create timing vocabulary.
- `src/shared/new-workspace/` and `src/shared/workspace-source.ts`.
- `src/shared/`: project, project-group, folder-workspace, orca.yaml hook, global-settings, workspace-status, workspace-cleanup, projectless-chat-folder and name-pool modules.
- `src/main/` root files prefixed `worktree`, `workspace`, `folder`, `repo`, `project`, `nested`, `destination` and `forked`. Non-test headers were read for all of them; bodies for about 25.
- `src/main/ipc/`: worktree and repo channel names, `worktree-logic.ts` (placement rules), `workspace-cleanup-*`, `worktree-symlinks.ts`, `worktree-remote.ts` (branch and base rules only).
- `src/main/runtime/`: `rpc/methods/repo.ts`, `rpc/methods/worktree.ts`, `rpc/methods/project-runtime-rpc-methods.ts`, `runtime-rpc/runtime-rpc-mobile-method-allowlist.ts`, repository registration and settings controllers, folder git initialisation, local create-candidate resolution, managed worktree listing and sleep.
- `src/main/agent-hooks/first-work-*.ts` (first-message auto-rename), `src/main/git/worktree-branch-removal.ts`, `src/main/git/branch-rename.ts`, `src/main/git/worktree-include-file.ts`, `src/main/git/worktree-shared-directories.ts`, `src/main/git/worktree-symlink-detection.ts` (headers and key branches).
- `src/renderer/src/`: grep only, for user-facing labels (`src/renderer/src/i18n/locales/en.json`) and entry points (forget, sleep, rename, hook trust).

### Added for pv96 (delta pass)

- Odd docs at `d3c6311a531`: `odd/tasks/worktree-one-line-sidebar.md`, `odd/tasks/no-git-folder-warning.md`, `odd/tasks/floating-workspace-checklist.md`, `odd/tasks/checklist-card.md`, and `odd/tasks/desktop-simplification.md` (workspace and git parts only).
- Diff stats and targeted diffs: `src/main/github/` (account switch, logout, remote accounts, sign-in, issue create), `src/main/gitea/` and `src/main/azure-devops/` (credential source and PR creation), `src/main/persistence/tracking-repos/worktree-identity-migration-session-refs.ts`, `src/main/runtime/` (worktree RPC method, git sync surface, the child bring-back caller), `src/main/ipc/` (repo argument schema, worktree metadata merge, GitHub account handlers, feedback), `src/main/delegated-session/child-bring-back*.ts` (names and policy only), `src/shared/` (worktree card layout, branch display label, native chat checklist, capability names, folder and worktree params, workspace name), `src/renderer/src/components/sidebar/` (file and component names only), and `src/cli/` (`folder-workspace` and `worktree sessions` specs).
- RPC method names diffed across `src/main/runtime/rpc/methods` and `src/main/ipc`: added `agentSessionChecklist.read`, `agentSessionChecklist.setItem`, `github.switchActiveAccount` and `github.removeAccount`; none removed.
- Scoped commit log (`custom/integration..d3c6311a531`): subjects listed, keyword subjects read.

## Not read or low confidence

- Renderer and mobile UI beyond grep hits. Behaviour is described from the core side. The hook-trust dialog and the rename, sleep and forget flows are seen only through their core calls and labels.
- Test files, except where a test name clarified a rule.
- `src/main/git/` beyond the helpers listed above. Worktree add, base-ref probes and the Git runner were not read.
- `src/main/persistence/`: only the project removal path, the removal sidecar and the profile-state paths. The on-disk schema was not read in full.
- SSH relay, WSL distribution internals and the ephemeral VM runtime (`src/main/ephemeral-vm-*`): only the workspace-facing decisions.
- Setup-script terminal mechanics (PTY creation, tab placement) and the chat roster UI: taken from registry comments.
- Profile cloud sign-in, organisation members, session exchange, profile session owner transfer and session markdown transfer.
- Effects of `projectGroup.delete` on child projects and workspaces: not verified.
- Whether removing a project touches any directory on disk: the removal path only edits the store and caches. Not verified beyond that.
- Linux-specific behaviour: no Linux-only branch was found in the files read. Low confidence.
- Prepared-checkout burst thresholds, telemetry events and create timing phases: described at a high level only.
- CLI commands (`orca worktree …`, `orca project …`) are named only, not described.
- The stored hook-trust record exists in shared types. The gating dialog is renderer-side and was not read.
- The reference `docs/` folder and its README were not read.
- pv96 delta: the renderer sidebar was read by file and component names and by the odd docs. The one-line rows, hover, session menu, placement and folded status are described from `worktree-one-line-sidebar.md` and commit subjects; nothing was run.
- pv96 delta: the floating workspace (checklist tab, session picker, host selector), the create page and the Tasks page come from `floating-workspace-checklist.md` and `desktop-simplification.md` only. Their code was not read; the create page was tested in components, not seen in the running app.
- pv96 delta: mobile rows and sheets come from `worktree-one-line-sidebar.md` only; no device run.
- pv96 delta: the runtime worktree-agent row, process summary and conversation-name bodies were read by name only. The persistence migration was read as its session-reference rewrite header.
- pv96 delta: CLI handler bodies were not read; specs and usage lines only.
- pv96 delta: `src/main/git/` has no changed files in pv96, so the base reading of that directory still applies.
- pv96 delta: bot-team code (`src/main/bot-teams/`, bot-team sidebar files) belongs to the bot-teams domain; only the Workspaces filter (WS-42) and team folder listing are noted here.

## Index

| ID | Feature | Proposal |
|---|---|---|
| WS-01 | Add an existing git repository as a project | first version |
| WS-02 | Add a folder as a plain-folder project | first version |
| WS-03 | Create, clone or initialise a project | later |
| WS-04 | Per-project base branch and workspace location | first version |
| WS-05 | Create a git worktree workspace | first version |
| WS-06 | Workspace and branch naming | first version |
| WS-07 | Create from an existing branch or a PR/MR head | later |
| WS-08 | Create from a work item (issue, PR/MR, Linear, Jira) | later |
| WS-09 | Plain-folder workspaces | first version |
| WS-10 | Projectless chats | later |
| WS-11 | Project groups and nested repository import | later |
| WS-12 | Workspace placement on disk | first version |
| WS-13 | Setup script on workspace create | later |
| WS-14 | Copy and share files into new workspaces | later |
| WS-15 | Archive script before removal | later |
| WS-16 | Remove a workspace | first version |
| WS-17 | Remove a project from MassOrca | first version |
| WS-18 | Auto-rename from the first agent message | later |
| WS-19 | Rename, pin, order, annotate and status | later |
| WS-20 | Sleep and wake a workspace | later |
| WS-21 | Workspace lineage (parent and child) | first version |
| WS-22 | Links from workspaces to issues, PRs and MRs | later |
| WS-23 | Base-branch drift and local base refresh | later |
| WS-24 | Worktrees created outside MassOrca: visibility and import | later |
| WS-25 | Disk usage and stale-workspace cleanup | later |
| WS-26 | Prepared checkouts for faster creates | later |
| WS-27 | Ephemeral VM workspaces from environment recipes | later |
| WS-28 | Windows runtime per project (Windows host or WSL) | later |
| WS-29 | Workspace records: persistence and live catalog | first version |
| WS-30 | Profiles: switcher and moving projects between profiles | drop (owner to confirm) |
| WS-31 | No Git warning when adding a folder | first version |
| WS-32 | No Git marker and Initialize Git on folder projects | later |
| WS-33 | One-line worktree sidebar (default layout) | first version |
| WS-34 | Session row actions and hover card | later |
| WS-35 | Child sessions listed under their parent across worktrees | first version |
| WS-36 | Checklist card in agent chats | later |
| WS-37 | Checklist tab in the floating workspace | later |
| WS-38 | Open an existing session in the floating workspace | later |
| WS-39 | Host selector in the floating workspace | later |
| WS-40 | Start a session in an existing branch or worktree from the create page | first version |
| WS-41 | Prototypes in the Workspaces side panel | first version |
| WS-42 | Show bot teams filter in Workspaces | later |
| WS-43 | CLI: list and close a worktree's agent sessions | later |
| WS-44 | CLI: folder workspace commands | first version |
| WS-45 | Sidebar card layout choice | later |

## Features

### WS-01 Add an existing git repository as a project

**Behaviour.** The user picks a folder with the desktop folder picker (`repos:pickFolder`, `pickDirectory`) or passes an absolute path to `repo.add`. MassOrca registers the folder as a project named after its folder name. The display name can be changed later. Adding the same path on the same host again returns the existing project and creates nothing. The new project gets a random id, a default badge colour, a detected repository icon and a detected upstream remote. Its default base branch is resolved in this order: `origin/HEAD`, `origin/main`, `origin/master`, local `main`, local `master`.

**Rules and edge cases.**
- The path must be absolute.
- For a git project the folder must be a git repository. Otherwise the add fails with "Not a valid git repository".
- Git commands for a Windows host wait for a Windows Git environment readiness step before the check.
- On Windows, path equality is case-insensitive after separator normalisation. The two WSL UNC spellings (`\\wsl.localhost\` and `\\wsl$\`) name the same location. On POSIX hosts paths are compared case-sensitively.
- A project is bound to one execution host. The same repository path on a second host is a second project. Adding through a paired runtime adopts an existing local row that has no host stamp.
- Projects can be reordered (`repos:reorder`, per host with `reorderForHost`).
- Mobile cannot add projects. The mobile method allowlist does not include `repo.add`.

**Core interface.** Clients call `repo.add` or the IPC channel `repos:add`. The core replies with the project record and emits `repos:changed`.

**Persists.** A project record: id, path, display name, badge colour, icon, upstream and remote identity, kind (`git`), host stamp, added time.

**Depends on.** Settings and persistence (WS-29). Local git. Remote execution hosts for the host stamp. Source-control providers for remote-derived identity.

**Proposal.** First version. Size S. Reason: the only way to bring a repository into MassOrca, and Phase 1 needs workspaces for git worktrees.

### WS-02 Add a folder as a plain-folder project

- Source: pv96 (unmerged). Folder projects now show a No Git marker on desktop and mobile, and adding a non-git folder shows a warning first that can offer Initialize Git (WS-31, WS-32).

**Behaviour.** The user adds a directory that is not a git repository as a project of kind `folder` (`repo.add` with kind folder). The folder itself is the single primary workspace of the project. The rest of the UI keeps one stable workspace identity for it. A folder project creates no worktrees and has no branch. It can hold folder workspaces inside a project group (WS-09). It can be upgraded to a git project later (WS-03).

**Rules and edge cases.**
- The same absolute-path and duplicate rules as WS-01 apply, without the git check.
- A create against a folder project registers a folder workspace in place (WS-09), with no branch.
- A folder on an SSH host carries the SSH target as its connection.

**Core interface.** `repo.add` with kind `folder`. Emits `repos:changed`.

**Persists.** The same project record with kind `folder`.

**Depends on.** WS-01, WS-09, WS-03.

**Proposal.** First version. Size S. Reason: "git worktree vs plain folder" is a carried-over product rule, and Phase 1 names folders as a workspace kind.

### WS-03 Create, clone or initialise a project

- Source: pv96 (unmerged). The initialise step is now reachable from the folder warning, the No Git marker and the phone sheet, gated by the `repo.init-git.v1` capability, and the phone method allowlist now includes `repo.initGit`.

**Behaviour.** Three entry points.
- **Create.** The user gives a parent folder, a name and a kind (git or folder). MassOrca creates the folder, including missing parents, and registers it. Names may not contain slashes and may not be `.` or `..`. If the target already exists as a registered project, that project is returned. The default parent is the configured workspace root, unless that value is still the untouched default. In that case the default is `<home>/orca/projects`.
- **Clone.** `repos:clone` (or `repo.clone`) takes a URL and a destination, reports `repos:clone-progress` events and can be aborted with `repos:cloneAbort`. An optional GitHub account can be used for authentication. A remote clone on an SSH host runs as `repos:cloneRemote`.
- **Initialise git.** `repo.initGit` turns a folder project into a git project. It runs `git init` and an empty first commit in the folder's own path on the host that owns it, then changes the kind to `git`. If a `.git` already exists, it only changes the kind. The operation is refused while the folder project has folder workspaces, because the git branch would drop them.

**Rules and edge cases.**
- `repos:isGitAvailable` probes for git before the git-dependent actions.
- An aborted clone removes the target folder only when this attempt created it.
- Initialisation on a remote host follows the same steps on that host.

**Core interface.** `repo.create`, `repo.clone`, `repo.initGit`, `repos:getDefaultCreateProjectParent`, `repos:isGitAvailable`. Progress is pushed as `repos:clone-progress`.

**Persists.** The project record. Clone progress is transient.

**Depends on.** WS-01, WS-02. Local git. Remote execution hosts for remote clone and create.

**Proposal.** Later. Size M. Reason: convenient, but daily work starts from a repository already on disk. Initialising in place can wait.

### WS-04 Per-project base branch and workspace location

**Behaviour.** For each project the user can set the base branch that new workspaces start from (`repo.setBaseRef`). The base picker searches refs and lists remote-tracking branches first (`repo.searchRefs`, `repos:searchBaseRefs`). The user can also set a per-project workspace location (`worktreeBasePath`), which overrides the global workspace root (WS-12).

**Rules and edge cases.**
- The base branch falls back to the detected default (WS-01 order) when the stored base no longer exists.
- Changing the workspace location invalidates the cached worktree scan of that project.
- The location may be absolute or relative to the project folder. Its interpretation on SSH and WSL is described in WS-12.

**Core interface.** `repo.setBaseRef`, `repo.baseRefDefault`, `repo.searchRefs`, `repo.update` (fields `worktreeBasePath` and `baseRef`). Mobile can call `repo.update`, `repo.baseRefDefault` and `repo.searchRefs`.

**Persists.** The per-host project setup record: path, `worktreeBasePath`, base branch, display name.

**Depends on.** WS-01, WS-12, WS-05. Local git. Remote execution hosts.

**Proposal.** First version. Size S. Reason: a create needs a base branch and a location. The other project options (hooks, visibility, Windows runtime, icons) are spread across WS-13, WS-24 and WS-28.

### WS-05 Create a git worktree workspace

**Behaviour.** From a git project the user chooses a name (optional), a base branch (default: the project base, WS-04), and may tick options such as sparse directories or a linked item (WS-08, WS-22). The renderer can prefetch the base before a create (`worktrees:prefetchCreateBase`). On create, MassOrca:
1. resolves the folder for the new workspace (WS-12);
2. creates a branch from the base, named by WS-06;
3. adds a git worktree at that folder;
4. registers the workspace with its metadata and a display name (WS-06);
5. runs the setup script if the project has one (WS-13);
6. returns the workspace, any base fallback, the setup receipt, timing data and a catalog version.

The create can be called from the desktop, from the CLI, from mobile (`worktree.create`, deduplicated by a client mutation id), from automations and from agents.

**Rules and edge cases.**
- The name is sanitised. Unicode letters and digits are kept. Other characters become dashes. Runs of dashes and dots collapse. Emoji become shortcodes. An empty result is refused with "Invalid worktree name". A name made only of emoji becomes `workspace`.
- Name collisions get numeric suffixes (`-2`, `-3`, …) up to a bounded number of attempts.
- A branch with the same name that already exists locally is reused only when the request asks for an existing branch (WS-07). A remote branch with the same name blocks the create unless it matches the selected review.
- A stored base that no longer exists falls back to the detected default. A usable custom base stays authoritative.
- If the requested origin base is missing but a local branch of the same name exists, the create uses the local branch and reports a base fallback.
- A create against a folder project registers a folder workspace in place: no folder is created and no branch is made.
- On a WSL-hosted project, git runs inside the distribution and the worktree is placed on the WSL filesystem (WS-12).
- On an SSH project, git runs through the SSH relay and the folder rule is the remote one (WS-12).
- Concurrent creates are allowed. Disk-heavy preparation is tracked as competing work (WS-26).

**Core interface.** `worktree.create`, `worktrees:create`. The core pushes `worktrees:changed` and progress events keyed by a creation id. The reply carries the workspace record, lineage, warnings, setup receipt, base fallback, base status, startup terminal and timing.

**Persists.** The workspace metadata record: display-name provenance, base ref, creation time, creating agent, creation source, creator provenance (host or paired device), sparse setup, push target, `preserveBranchOnDelete`, host and project ids. Git holds the worktree registration and the branch.

**Depends on.** WS-04, WS-06, WS-07, WS-08, WS-12, WS-13, WS-26, WS-29. Local git. Remote execution hosts. Agent integrations (creating agent). Automations and CLI (other creators).

**Proposal.** First version. Size M. Reason: create is one of the three Phase 1 workspace operations. Prepared checkouts, sparse setup, PR bases and setup scripts are separate features.

### WS-06 Workspace and branch naming

- Source: pv96 (unmerged). Generated slugs fold accented letters to their plain letter, and a delegated child's worktree is named from its chat's name.

**Behaviour.** The branch name is `<prefix>/<name>`. The prefix comes from `branchPrefix`:
- `git-username` (default): the git user name of the project, resolved from git configuration and, when needed, the GitHub CLI.
- `custom`: the value of `branchPrefixCustom`.
- `none`: no prefix.

The folder name is the sanitised name, plus a collision suffix when needed. The display label defaults to the name. A label the user types is pinned. An automatic label follows the branch.

When the user leaves the name empty, MassOrca picks a generated name from a marine-creature pool (for example `Nautilus`). Generated names are chosen at random among unused names, so new workspaces do not all start at the same entry. Once the pool is spent, names take suffixes (`name-2`, `name-3`).

**Rules and edge cases.**
- An invalid custom prefix fails the create with a settings hint.
- A git-username that resolves to garbage (for example an error body) is skipped and the create continues without a prefix. A custom prefix fails loudly.
- A branch name supplied as an override must pass git's ref-format check and may not start with `-`.
- A generated name is retired when its workspace is created, not when it is removed. A retired name is never reissued in the same project. This matters because agent CLIs key conversation state by path, so reusing a folder name would hand old history to a new workspace.
- Retirement records are kept per project and copied to a namespace that survives removing and re-adding the project.
- A backfill scan of agent history directories (Claude project directories on the host, Claude profile directories and WSL distribution homes) marks old workspace paths as retired. The scan has a deadline. A stalled network mount degrades to "retire nothing" rather than blocking create.
- Duplicate checks compare Windows paths case-insensitively.

**Core interface.** Part of `worktree.create`. `worktree.listRetiredNames` lists retired names for clients.

**Persists.** The per-project retired-name registry and its namespace copy. The display-name provenance flag on the workspace. The branch name lives in git.

**Depends on.** WS-05, WS-18 (later auto-rename changes the branch leaf), WS-29. Agent integrations (history directories). Settings and persistence.

**Proposal.** First version. Size S. Reason: create needs a deterministic branch name and a folder name. Retirement can start simpler (unique suffixing) and be tightened later, but the owner should know that generated names are otherwise reissued.

### WS-07 Create from an existing branch or a PR/MR head

**Behaviour.**
- **Existing local branch.** The user picks a local branch. MassOrca checks it out into a new worktree instead of creating one, and marks the workspace so deleting it keeps the branch (`preserveBranchOnDelete`).
- **Pull or merge request.** The user picks a PR (GitHub) or MR (GitLab). MassOrca resolves the head branch and head commit (`worktrees:resolvePrBase`, `resolveMrBase`), creates or reuses a local branch under the PR's branch name when the PR comes from the same repository, and uses the verified head commit as the start point. The review's base branch is kept as the compare target for source control.
- **Fork PR.** MassOrca adds the fork as a push remote (flagged as created by Orca when it was added). It records whether maintainers may edit the branch. A warning says a later push may still be rejected by GitHub permissions.

**Rules and edge cases.**
- An existing branch checked out in another worktree, or a remote branch with the same name, goes through the same branch-conflict policy as WS-05. A conflict that matches the selected PR is allowed.
- Creating from a PR uses the head commit as the start point, so the workspace does not move if the branch advances later.
- Creating from an MR follows the same flow for GitLab.

**Core interface.** `worktree.resolvePrBase`, `worktree.resolveMrBase`, `worktrees:resolvePrBase`, `worktrees:resolveMrBase`, plus the create options above.

**Persists.** Push target (remote name, branch name, remote URL, remote-created flag), start-point head, compare base, `preserveBranchOnDelete`, and the linked review.

**Depends on.** WS-05, WS-22, WS-06. Local git. Source-control providers (GitHub, GitLab).

**Proposal.** Later. Size L. Reason: useful for review flows, but it brings fork remotes, branch conflicts and provider calls. Daily agent work can start from new branches.

### WS-08 Create from a work item (issue, PR/MR, Linear, Jira)

**Behaviour.** The new-workspace composer has a smart input with search modes: smart (all sources), GitHub, GitLab, branches, Linear, Jira and plain text. The user types free text, or pastes a GitHub, GitLab, Linear or Jira link, or a number. Results come from the matching providers. Choosing an item creates a workspace:
- its display name is the item title;
- its git-safe seed name is derived from the item (`getWorkspaceSourceName`);
- the workspace is linked to the item (WS-22);
- for a PR or MR, it uses the review's branch (WS-07).

The auto-generated name replaces the input only while the input is empty, still equals the last auto-name, or is a lookup reference. Deliberate names are kept.

**Rules and edge cases.**
- Search is debounced (about 200 ms). The composer keeps the last results while the user is still typing and hides them as soon as the field is cleared.
- A pasted task link that cannot be resolved blocks creation. A bare text query does not.
- A Linear issue can supply a usable branch name, applied as an exact branch override. Otherwise the generated name is used.
- GitLab link recognition uses the project-internal `/-/` separator, so self-hosted GitLab works.
- Jira and Linear identifiers are strings. GitHub and GitLab numbers are numeric.

**Core interface.** Uses the provider lookups of the source-control domain through the client. The core receives the resulting create (WS-05) with the linked-item fields.

**Persists.** Linked item (provider, type, number or identifier, title, URL, repo id, Linear workspace), task-source context and the display name.

**Depends on.** WS-05, WS-07, WS-22. Source-control providers and issue trackers (Linear, Jira). Local git.

**Proposal.** Later. Size L. Reason: valuable for issue-driven work, but it depends on every tracker integration. Plain names cover the first version.

### WS-09 Plain-folder workspaces

- Source: pv96 (unmerged). Folder workspaces can take a one-line row name (WS-19), and the phone's folder sheet offers no Delete because `folderWorkspace.delete` is not on the mobile allowlist.

**Behaviour.** Inside a folder project or a project group, the user creates a folder workspace: a workspace bound to an existing directory with no git operations. It gets the same workspace features as a git worktree: terminals, agents, notes, status, lineage and links. It has no branch. The directory name is the default workspace name.

**Rules and edge cases.**
- The folder must exist and be a directory. Otherwise the create is refused with one of: `folder_workspace_path_missing:<path>`, `folder_workspace_path_not_directory:<path>`, `folder_workspace_connection_ambiguous:<path>`, `folder_workspace_path_unavailable:<path>`.
- The connection (local or an SSH target) is inferred from the project. If one path matches more than one connection or scope, the create is refused as ambiguous.
- Chats started from the create-chat surface are folder workspaces whose first agent message renames them (WS-18).
- Removing a folder workspace closes its terminals, deletes its remote terminal history for SSH folders and forgets its record. The directory is not deleted: the removal path has no filesystem delete. The root workspace of a folder project cannot be removed this way; the project is removed instead (WS-17).
- The folder root of a folder project counts as a workspace with a stable id.

**Core interface.** `folder` workspace operations in the runtime; folder workspace identity uses the `folder:<id>` key. Clients receive the folder workspace in the same catalog as git workspaces.

**Persists.** The folder workspace record: id, project group, name, folder path, connection, host stamp, creator provenance, linked task, comment, flags, timestamps. Diff comments are stored per workspace and self-prune when the workspace goes.

**Depends on.** WS-02, WS-11, WS-19, WS-29. Remote execution hosts for SSH folders. Agent integrations.

**Proposal.** First version. Size S. Reason: Phase 1 names worktrees and folders as the two workspace kinds.

### WS-10 Projectless chats

**Behaviour.** A chat can be started without a project. MassOrca creates a folder `<projectlessChatsDir>/<YYYY-MM-DD>/<6-character id>` and a folder workspace inside a project group named `Chats`, which is created on first use. The chat id is a temporary name. The first agent message renames the folder workspace (WS-18).

**Rules and edge cases.**
- The default root is `<home>/orca/chats`. A blank, relative or invalid value falls back to the default. `~` expands to the home directory.
- Projectless chats are refused on SSH locations (`ssh_location_unsupported`). The root is a local setting, and the code avoids creating a local folder for a remote location.
- The chat id is drawn from lowercase letters and digits. A folder collision is retried up to 1000 times.

**Core interface.** The create-chat runtime path. The core returns the workspace key and the folder path.

**Persists.** The folder workspace record, the `Chats` group and the folder on disk.

**Depends on.** WS-09, WS-11, WS-18. Agent integrations.

**Proposal.** Later. Size S. Reason: a convenience. WS-09 covers the need for a folder workspace.

### WS-11 Project groups and nested repository import

**Behaviour.** A project group is a sidebar container. It holds projects and folder workspaces, can nest under another group, and has a colour, a collapsed state and a tab order. Groups are created by hand, by a folder scan or by migration. Projects can be moved between groups (`projectGroup.moveProject`). Groups can be renamed, updated and deleted (`projectGroup.create`, `update`, `delete`).

Folder scan: the user picks a parent folder and MassOrca scans it for nested repositories (`projectGroup.scanNested`):
- default depth 3, default maximum 100 repositories;
- skips `node_modules`, `dist`, `build`, `vendor`, `.cache`, `__pycache__`, `.turbo` and `.parcel-cache`;
- honours the `.gitignore` files it meets on the way;
- does not follow symlinked directories;
- recognises VCS metadata folders (`.git`, `.svn`, `.hg`, `.jj`, `.sl`, `.repo`, `CVS`).

The scan result reports the repositories found, whether it was truncated, timed out or stopped, and its duration. Import (`projectGroup.importNested`) runs in one of two modes:
- `group`: create a group that contains the imported repositories;
- `separate`: import each repository as its own project.

Each repository is reported as imported, already known, or failed.

**Rules and edge cases.**
- Import only uses the bounded scan of the chosen parent. Paths outside it cannot be smuggled in.
- A linked worktree collapses to its owner project only when Git proves both belong to the same non-bare worktree graph.
- A partial scan (timed out or stopped) imports only what was found.
- Groups created by a folder scan get a root group.

**Core interface.** `projectGroup.list` (also allowed on mobile), `create`, `update`, `delete`, `moveProject`, `scanNested`, `importNested`.

**Persists.** Group records: name, parent path, parent group, created-from, tab order, collapsed state, colour, connection id, host stamp.

**Depends on.** WS-01, WS-02, WS-09. Remote execution hosts for remote folders.

**Proposal.** Later. Size M. Reason: organisation for users with many repositories. Phase 1 can use a flat project list.

### WS-12 Workspace placement on disk

**Behaviour.** New git worktrees go under a workspace root:
- global default `<home>/orca/workspaces` (setting `workspaceDir`);
- overridden per project by `worktreeBasePath` (WS-04);
- overridden per host by the host's `defaultWorktreeLocation`.

The layout setting `nestWorkspaces` (default on) decides the leaf:
- on: `<root>/<repo name>/<workspace name>`;
- off: `<root>/<workspace name>`.

A relative root is resolved against the project folder. A project base that is set explicitly is trusted as that project's own folder even when the global layout is flat.

**Rules and edge cases.**
- **SSH.** An absolute desktop root is not used on the remote host. The worktree goes to a sibling of the repository folder named `<repo name>-<workspace name>`. A relative root, or a project base that is an absolute path on the remote host, is used as given.
- **WSL.** A project on a WSL UNC path places its worktrees under that distribution's home at `~/orca/workspaces`, mirroring the layout there, so git and the terminal stay on the WSL filesystem. A Windows project with a configured WSL mirror distribution mirrors an absolute local root into that distribution's home. A UNC root is not mirrored.
- **macOS.** Creating the root touches it before the first create. This triggers the macOS folder-access prompt where needed. Access is re-checked when settings change.
- **Windows.** Folder names are sanitised as in WS-05. Path comparisons are case-insensitive (WS-01). Auto-rename never moves folders (WS-18).
- Changing the root does not move existing workspaces (inferred: no move path was found in the code).

**Core interface.** The placement is computed by the core. Clients receive the resulting path in the create reply. The core watches the workspace roots for changes.

**Persists.** `workspaceDir`, `nestWorkspaces`, the recent root history (`workspaceDirHistory`), the per-project `worktreeBasePath`, and the per-host default location.

**Depends on.** WS-04, WS-05, WS-28. Remote execution hosts. WSL distribution handling.

**Proposal.** First version, for local roots and the nesting layout only. Size M. Reason: needed to create workspaces at all. WSL mirroring and SSH sibling paths can follow the host work.

### WS-13 Setup script on workspace create

**Behaviour.** A project can define a setup command. It comes from the committed `orca.yaml` (`scripts.setup`) or from a local setting, and `commandSourcePolicy` decides which one counts (`shared-only`, `local-only`, or `run-both`). After a worktree is created, the setup command runs in a terminal. By default it opens a background tab titled "Setup" and leaves focus on the first tab. A setting can split the first pane vertically instead. The chat roster lists setup runs as rows, live or settled.

The create request can pass `setupDecision` (`run`, `skip` or `inherit`). `setupRunPolicy` sets the default:
- `run-by-default` (the default);
- `skip-by-default`;
- `ask`. Under `ask`, a create without an explicit decision fails with "Setup decision required for this repository".

`setupAgentStartupPolicy` decides whether the agent starts immediately (default) or waits for setup to finish.

The create reply carries a setup receipt with one of four states: `running`, `skipped`, `not_configured`, `spawn_failed`. A spawn failure does not undo the workspace.

**Rules and edge cases.**
- Main owns the setup run registry, so a renderer reload keeps the run and its row.
- A user stop kills the process with its own code. Only a run that finished by itself reports completion.
- Long commands are written into a runner script so the terminal line editor does not wrap or truncate them.
- Trust: the desktop asks the user to trust committed `orca.yaml` commands before they run. The approval is stored per project and per hook, keyed by content hash. The gate is renderer-side.
- Default terminal tabs from `orca.yaml` (`defaultTabs`) are created for a new worktree and reported in the create reply. Their commands follow the same trust rule.
- On SSH the same policy applies in the remote create path. The mechanics of the remote run were not read.

**Core interface.** `repo.hooks`, `repo.hooksCheck`, `repo.setupScriptImports` (lists imports the setup script pulls in), `repo.issueCommandRead` and `repo.issueCommandWrite` (the shared issue command). The create reply carries the setup receipt.

**Persists.** The per-project hook settings (`setupRunPolicy`, `setupAgentStartupPolicy`, `commandSourcePolicy`, local scripts). Trusted-hook approvals are stored in UI state. Setup runs are in memory in the run registry.

**Depends on.** WS-05, WS-04. Terminals (PTY sessions). Agent integrations (startup policy). Remote execution hosts.

**Proposal.** Later. Size M. Reason: a new worktree is usable without it, because the user can run the install command in a terminal. Add once terminals exist and the owner wants setup automation.

### WS-14 Copy and share files into new workspaces

**Behaviour.** A new worktree can receive three kinds of material from the main checkout:
- files and folders named in the repository-root `.worktreeinclude` file (gitignored items such as `.env` files, each copied per worktree);
- directories listed in `orca.yaml` `worktree.sharedDirectories`, which are linked rather than copied so one dependency install serves every workspace;
- user-configured symlink paths (`symlinkPaths`), linked when a copy-on-write clone is not available.

**Rules and edge cases.**
- `.worktreeinclude` takes only literal file and directory lines. Glob and negation lines are skipped with a warning.
- Copies on macOS use APFS copy-on-write clones when available. Otherwise they fall back to a real copy or a link.
- Shared directories are always linked. A clone would give each workspace its own dependency tree.
- Windows tries a directory junction first, which needs no administrator rights or developer mode. It falls back to a symbolic link, which is needed for WSL UNC paths because a junction cannot target them. Without either, an ordinary Windows user would silently get no shared directory.
- A copy budget limits how much `.worktreeinclude` material is copied.
- Symlinks are checked for safety: a path that escapes the project root is refused.

**Core interface.** Applied inside the create (WS-05). Clients see nothing beyond the create result.

**Persists.** The project's `symlinkPaths` and the `orca.yaml` settings. The copied or linked files live on disk.

**Depends on.** WS-05, WS-04, WS-13. Settings and persistence.

**Proposal.** Later. Size M. Reason: speeds up JavaScript and environment-heavy repositories. A fresh checkout works without it.

### WS-15 Archive script before removal

**Behaviour.** A project's `archive` script (orca.yaml `scripts.archive` or the local setting) runs before a workspace's checkout is deleted. It is the user's last chance to save work off the checkout. If the script exits non-zero, times out after 120 seconds, or its outcome cannot be observed (spawn failure, lost contact with the host), the removal is refused with a message. The desktop toast then offers "Delete Anyway", and the CLI offers a force flag. A waiver is reported in the removal result.

**Rules and edge cases.**
- The archive script runs before any stop, deregister or delete. A failed hook must not allow the delete to proceed silently.
- Loss of contact with the execution host is treated as unverifiable, which blocks removal. It is never treated as success.
- Within one project, each removal's archive hook, preflight and teardown wait until the previous removal in that project was accepted. The Git checkout delete runs in parallel under its own limit, which keeps hooks that write refs from racing the repository's ref locks.
- Clients waiting for a removal reply wait at least as long as the hook timeout, so they do not report a failure for a hook that is still running.
- The SSH runner returns the same outcome shape as the local runner.

**Core interface.** Part of `worktree.rm` (WS-16). The refusal has a stable error code (`worktree_archive_hook_failed`) and a prefix that clients match.

**Persists.** The hook's outcome is part of the removal record (WS-16).

**Depends on.** WS-16, WS-13 (hook trust). Remote execution hosts.

**Proposal.** Later. Size S. Reason: the removal guards in WS-16 are enough for the first version. The archive hook adds a user-owned safety net.

### WS-16 Remove a workspace

- Source: pv96 (unmerged). "Delete with descendants" also removes the worktrees of delegated child sessions, and closing a child session's last tab now closes only the tab instead of removing its worktree.

**Behaviour.** The user removes a git worktree workspace. MassOrca stops its terminals and agent sessions, runs the archive hook (WS-15), applies the safety checks below, and deletes the worktree folder through Git. The row shows "removing" while the delete continues in the background. The removal is accepted quickly, is recorded on disk before Git starts deleting, and resumes after a restart if the app quit in the middle. A failed delete stays listed with its error until it is retried or the folder disappears. Nested worktrees inside the folder are part of the same plan, deepest first, and each must be approved explicitly with its current head and branch.

**Refusals and waivers.** Each refusal is recognised from its message text by a classifier, and the desktop force affordance depends on that classifier. The table lists the cases.

| Refusal | Cause | Waiver |
|---|---|---|
| Locked worktree | Git lock on the worktree | None in MassOrca. The user runs `git worktree unlock` |
| Dirty | uncommitted or untracked changes | Force |
| Orphan directory | folder still exists but Git no longer lists it | Force |
| Missing registration | Git no longer lists it and the folder is gone | Force |
| Unstopped terminal | a terminal could not be proven stopped | Explicit waiver |
| Running agent session | a session is still attached | Explicit waiver, with "still live" evidence when observed |
| Archive hook failed | WS-15 | Delete Anyway or the force flag |

The main worktree and a folder project's root cannot be removed through this path. A home directory path is refused by a home guard, which decides whose home a path belongs to on the host that runs the delete.

**Branch handling.** The branch is deleted only when Git proves it has no unmerged changes, and squash merges are considered. Otherwise the branch is kept and reported as preserved. A branch that was checked out from an existing branch (WS-07) is never deleted. A preserved branch can be deleted later with `worktree.forceDeleteBranch`, which checks that the branch head has not moved since the user saw it.

**Rules per host.**
- Windows: an agent's working directory locks its folder, so removal stops the workspace's terminals and agent sessions first and refuses while any of them still runs.
- SSH: the delete runs on the host that owns the checkout. A disconnected host makes the outcome unknown, and cleanup treats it as a blocker (WS-25).
- Removal targets are host-qualified. The same worktree id on two hosts is two workspaces, and the removal goes to the host the user confirmed, never to whichever host routing prefers.

**Core interface.** `worktree.rm` and `worktrees:remove` (with force, branch and nested-approval options). `worktrees:previewNestedRemoval` returns the nested plan. `worktree.forceDeleteBranch`. The core pushes `worktrees:changed`, removal-in-progress state and the refusal kind.

**Persists.** The removal record, written before Git deletes anything, in a sidecar beside the profile state. The accepted-removal table, mirrored to disk on every change. Workspace metadata for the removed workspace is dropped. The generated name stays retired (WS-06).

**Depends on.** WS-15, WS-06, WS-07, WS-25, WS-29. Terminals and agent sessions. Remote execution hosts.

**Proposal.** First version. Size M. Reason: removal is one of the three Phase 1 workspace operations, and the refusals are what keep it from losing work.

### WS-17 Remove a project from MassOrca

**Behaviour.** The user removes a project from its list. The project's registration on that host is removed, together with its workspace records, its workspace session state on that host, and its terminal layout. The folder, the git repository and every worktree stay on disk. A project id registered on another host stays.

**Rules and edge cases.**
- Retired names and sparse presets for the project are dropped when no registration with that id remains on any host. The namespace copy of retired names survives, so re-adding the project does not reissue old names.
- Residue of removed projects is swept at load.
- Mobile cannot remove projects. The mobile method allowlist does not include `repo.rm`.

**Core interface.** `repo.rm` and `repos:removeForHost`. Emits `repos:changed`.

**Persists.** Removes the project record and the associated workspace state from the store.

**Depends on.** WS-01, WS-29.

**Proposal.** First version. Size S. Reason: undoing a wrong add is needed from the start. Nothing on disk is touched.

### WS-18 Auto-rename from the first agent message

**Behaviour.** Setting `autoRenameBranchFromWork` (default on; can be turned off in Git settings) controls this flow. When a workspace created with a generated name starts its first agent work, MassOrca:
1. generates a short kebab-case branch name from the first prompt, using the configured text-generation model (at most four words);
2. renames the branch;
3. sets the display label to a readable form of the new name;
4. on macOS and Linux, for a local project, moves the folder to match the new name with `git worktree move`, and migrates the workspace identity so its terminals and metadata follow.

If a step fails, the workspace shows a "rename failed" badge and a dialog with the git output. The agent is never interrupted.

**Rules and edge cases.**
- Only Orca-generated creature branch names are eligible. A branch the user named is never renamed.
- A branch with an upstream is never renamed, because renaming would orphan the remote branch and any open PR.
- The folder move is skipped, and the branch and label still change, when:
  - the project is remote (SSH folder moves are not mirrored);
  - the platform is Windows, because the OS locks a folder that is an agent's working directory;
  - the target folder already exists;
  - the path settings would put the new folder under a different parent (a rename, not a relocation, is required).
- A chat (WS-10 or WS-09) gets only its display label renamed by its first agent message. Its folder keeps the chat id.

**Core interface.** Internal to the runtime, triggered by the first agent work event from the agent status domain. Clients receive branch-renamed and worktree-renamed notifications, and `worktree.set` can clear the failure badge indirectly through the UI.

**Persists.** The branch (git), the folder (disk), the display label and its provenance, the pending-first-message flag and the last rename error.

**Depends on.** WS-05, WS-06, WS-12, WS-29. Agent status and hooks. Text generation (branch-name model). Local git.

**Proposal.** Later. Size M. Reason: a polish feature. Users can name workspaces by hand in the first version.

### WS-19 Rename, pin, order, annotate and status

- Source: pv96 (unmerged). The one-line row has its own manual name (`manualDisplayName`) that never renames the git branch and does not replace the display name used by the other layouts; Update edits it in every layout.

**Behaviour.**
- **Rename.** Inline rename of the workspace title. Empty text returns the title to automatic (branch-derived).
- **Pin.** A pinned flag on the workspace. Cleanup still lists a pinned workspace and can queue it, but skips its broad git read (WS-25).
- **Order.** Manual order (higher values render earlier) or recent order. Persisted sort orders are sent in one batch.
- **Unread.** A marker the user or the activity system sets.
- **Comment.** Free text on the workspace.
- **Status.** A user-assigned board status. Defaults are Todo, In progress, In review and Done. The user can edit the list. Status definitions are stored in UI state, not per host.

**Rules and edge cases.**
- A rename with text pins the label, so a later branch change keeps it.
- Auto-rename (WS-18) respects a pinned label.
- The status list is shared across projects.

**Core interface.** `worktree.set` (display name, pin, unread, comment, order, status, base ref, sparse fields, push target), `worktree.persistSortOrder`.

**Persists.** Workspace metadata fields: display name and its pinned flag, pinned, unread, sort order, manual order, comment, status. UI state: status definitions.

**Depends on.** WS-05, WS-18, WS-29.

**Proposal.** Later. Size S. Reason: the Phase 1 exit names create, list and remove. Organisation comes after that.

### WS-20 Sleep and wake a workspace

**Behaviour.** Sleeping a workspace stops its running terminals and agent sessions and keeps the workspace in the list. On desktop, the renderer tears down the workspace's tab state first and then kills its PTYs, so cleanup runs in the right order. Activating a sleeping workspace restores its layout. When a phone activates it, the host wakes its slept agents. `worktree.activate` takes a navigation target and a notification flag.

**Rules and edge cases.**
- Sleep is initiated by the renderer on desktop. The core notifies the renderer rather than killing processes itself.
- The renderer keeps a per-workspace sleep-intent record.
- Removal of a sleeping workspace still goes through WS-16 guards.

**Core interface.** `worktree.sleep`, `worktree.activate` (both allowed on mobile).

**Persists.** Nothing beyond the workspace record and the agent resume records owned by the agent domain.

**Depends on.** WS-16. Terminals (PTY sessions). Agent integrations. Server and pairing (mobile wake).

**Proposal.** Later. Size M. Reason: useful for resource control with many workspaces. The first version can stop terminals explicitly.

### WS-21 Workspace lineage (parent and child)

- Source: pv96 (unmerged). Parent links reach paired clients (the host stamps the delegating parent on the status summary), and the sidebar lists a child session under its parent across worktrees (WS-35).

**Behaviour.** A workspace can record a parent workspace: the one it was created from, by an orchestration run, by a CLI call made from a terminal in another workspace, or by a manual link. Parent and child relationships are shown and can be changed. Setting no parent clears the link (`noParent`).

**Rules and edge cases.**
- Each link records its origin (orchestration, CLI or manual) and its capture confidence (explicit or inferred). Inferred captures come from the terminal, the working directory, the environment or the active workspace.
- Links are tied to a workspace instance id. A link to a path that was reused for a different workspace is rejected as stale.
- Pruning removes an edge only after more than one observation shows it gone. One empty observation is not proof.
- Folder workspaces can be parents and children.

**Core interface.** `worktree.lineageList`, `worktrees:listLineage`, `worktrees:listLineageForHost`, `worktrees:updateLineage`, and the `lineage` field of `worktree.set`.

**Persists.** Lineage records for worktrees and for workspace keys, with warnings.

**Depends on.** WS-05, WS-09, WS-29. Automations and orchestration. CLI.

**Proposal (before the owner's direction).** Later. Size M. Reason: needed for orchestration views, not for one agent in one workspace.

**Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`):** orchestration is in the first version, and a coordinator launches child agents with worktree handoff (X-DL-01), so parent and child workspace links are needed. Proposal: first version (was later).

### WS-22 Links from workspaces to issues, PRs and MRs

**Behaviour.** A workspace can be linked to one or more work items: GitHub issue or PR, GitLab issue or MR, Linear issue, Jira issue, Bitbucket PR, Azure DevOps PR or Gitea PR. The first link is the active one for older readers. PRs found by branch name appear automatically. An explicit unlink hides that GitHub PR from branch discovery (`suppressedGitHubPR`).

**Rules and edge cases.**
- Each link records where it was added from (tab, pane, agent session).
- Linear identifiers are strings. GitHub and GitLab numbers are numeric.
- A link can seed the display name (WS-08).
- Review state (open, draft, merged, closed) feeds cleanup filters (WS-25).

**Core interface.** The linked fields of `worktree.set` (`linkedIssue`, `linkedPR`, `linkedItems`, `linkedWorkItem`, and the per-provider fields). Review and issue data come from the source-control domain.

**Persists.** Link fields and the attachment list in the workspace metadata.

**Depends on.** WS-08, WS-29. Source-control providers and issue trackers.

**Proposal.** Later. Size M. Reason: links are how work is tracked across a team, but a workspace works without them.

### WS-23 Base-branch drift and local base refresh

**Behaviour.** Each workspace reports whether its base branch has moved since it was created. The states are: checking, current, drift (with the number of commits behind and recent subjects), base changed and unknown. When the local base branch (for example `main`) is behind its remote, MassOrca can fast-forward it before a create. The setting `refreshLocalBaseRefOnWorktreeCreate` (default off) turns this on. Otherwise MassOrca shows a suggestion once, which the user can dismiss for good (`localBaseRefSuggestionDismissed`).

**Rules and edge cases.**
- The local base is fast-forwarded only when its checkout is clean and the update is a fast-forward. The result is one of: updated, skipped because the worktree is dirty, skipped because it is not a fast-forward, or skipped because of an error.
- The check runs on the host that owns the project: local, WSL, or SSH through the relay. The same policy is shared across them.
- Separately, local projects get idle ref packing (`pack-refs`). It waits while a create or removal is in flight, while an agent is running, on battery power, and during shutdown.

**Core interface.** Base-status events per workspace. `worktrees:prefetchCreateBase` (also used by mobile through `worktree.prefetchCreateBase`).

**Persists.** The workspace `baseRef`. The two settings flags above.

**Depends on.** WS-05, WS-04. Local git. Remote execution hosts.

**Proposal.** Later. Size M. Reason: keeps workspaces current with the base. The first version can require the user to update the base manually.

### WS-24 Worktrees created outside MassOrca: visibility and import

**Behaviour.** Git worktrees that MassOrca did not create (made by the git CLI or other tools) are detected per project (`worktrees:listDetected`). Each detected row has an ownership class:
- `orca-managed`: MassOrca's metadata proves it created the worktree;
- `external`: created outside MassOrca;
- `unknown-legacy`: created before the metadata existed;
- `agent-scratch`: created by an agent tool in a scratch root.

Whether a row is shown follows these rules, in order:
1. the selected checkout and any `orca-managed` worktree are always shown;
2. a path explicitly imported by the user is shown;
3. a row inside a visibility source (a built-in or custom root) follows that source's setting;
4. `agent-scratch` follows the agent-worktree setting, which defaults to hidden;
5. `unknown-legacy` in a legacy project is shown;
6. otherwise the external setting applies. A project's own setting wins, then the host default, then the default. The default is hidden for projects added after the rollout date and shown for older ones.

**Built-in visibility sources.** `.claude/worktrees` (id `claude`) and `.gsd-workspaces` (id `gsd`). Both default to hidden. Custom sources are absolute root paths; up to 32 are allowed. Each has a show or hide setting at host or project level.

**Rules and edge cases.**
- A project whose configured workspace base points at or inside a scratch root is not treated as scratch. The code comments cite issues #9388 and #15232 for this rule.
- When Git cannot be asked, the listing is marked unavailable rather than empty. "Could not ask" is not "nothing there".
- Catalogs carry a version so a stale listing does not overwrite a newer one.

**Core interface.** `worktrees:listDetected` (cancellable with `worktrees:cancelListDetected`), `worktrees:listAll`, `worktree.detectedList`, `worktrees:listKnownForExecutionHost`, `worktrees:forgetRemovedForExecutionHost`.

**Persists.** Visibility preferences (per project and as host defaults). Imported external paths.

**Depends on.** WS-01, WS-12, WS-29. Local git.

**Proposal.** Later. Size M. Reason: useful when users also work from the CLI, but the first version can list only worktrees MassOrca created.

### WS-25 Disk usage and stale-workspace cleanup

**Behaviour.** The cleanup view lists candidate workspaces: those that are archived, or idle and clean. "Idle" is measured from the chosen signal: last visited, last activity, or creation. Each candidate shows the blockers that stop it from being queued. Filters narrow the list by idle threshold, git state (clean, dirty, unpushed, unknown), agent state (working, permission prompt, idle), review state (open, draft, merged, closed, unknown), and presence of unread or diff comments. A candidate can be dismissed. A dismissal is fingerprinted, so a material change brings the candidate back.

Disk usage: a space analysis measures each workspace's size with `du`, with a 120-second deadline per measurement. Scans run with limited concurrency: 2 repository scans, 1 local worktree scan, 2 remote fallback scans. A scan can be cancelled. Results are kept as snapshots until invalidated.

**Blockers.** Main worktree, folder project, pinned, active workspace, running terminal, terminal liveness unknown, dirty editor buffer, volatile local context, recent visible context, live agent, SSH disconnected, git status error, dirty files, unpushed commits, unknown base, and dismissed.

**Rules and edge cases.**
- Git is read only when needed to protect dirty or local-only work. Skipping it is allowed on broad scans. A forced git read runs at confirm time, before any destructive action, so a delete does not proceed without evidence.
- Removal of a selected candidate goes through WS-16 and its refusals.
- Pinned workspaces are queueable but skip the broad git read.

**Core interface.** Cleanup scan and snapshot calls through IPC. Space-analysis calls. Removal reuses `worktrees:remove`.

**Persists.** The cleanup scan snapshot (fingerprinted), dismissals, and the space-analysis snapshot.

**Depends on.** WS-16, WS-19, WS-22, WS-29. Terminals and agent status. Remote execution hosts.

**Proposal.** Later. Size L. Reason: keeps disks and lists tidy with many workspaces. Not needed for the first version.

### WS-26 Prepared checkouts for faster creates

**Behaviour.** When a user creates workspaces repeatedly in one project, MassOrca builds spare full checkouts of the base in the background, up to three at a time. A spare is kept for five minutes. The next create claims a spare whose base still matches, and skips most of the checkout time. An isolated single create builds nothing, so the first create of a session pays nothing for a spare nobody claims.

**Rules and edge cases.**
- The claim key includes the base. Before a claim, a base probe and drift check run.
- Spare disk use is bounded. The oldest spare of the requesting workspace is evicted first.
- A failed discard is remembered and retried on the next preparation for the same host.
- Spares left by dead processes are reclaimed in the background. This never blocks a user's create.
- Preparation's disk work is counted as competing with creates in the create timing data.

**Core interface.** `worktrees:prefetchCreateBase` and `worktree.prefetchCreateBase`. The create reply reports a prepared-checkout hit or miss, with the reason for a miss.

**Persists.** Nothing beyond the workspace created. Spares are on disk until claimed or evicted.

**Depends on.** WS-05, WS-23. Local git. Remote execution hosts.

**Proposal.** Later. Size M. Reason: an optimisation. Creates work without it.

### WS-27 Ephemeral VM workspaces from environment recipes

**Behaviour.** A project's `orca.yaml` can define environment recipes. Each recipe has a create command and optional suspend, resume and destroy commands, and a checkout mode: a worktree inside the VM (`orca-worktree`) or a provisioned root (`provisioned-root`). A workspace can be backed by a VM provisioned from a recipe. Invalid recipe entries are listed as diagnostics without failing the whole file.

**Rules and edge cases.**
- A recipe can disable destroy (`destroyDisabled`).
- Provisioning, suspend, resume and cleanup are separate runtime flows, with a failed-start cleanup path.
- A workspace records its checkout mode, and removal of a VM workspace follows the VM lifecycle rather than a plain worktree delete. The reading of this path is shallow.

**Core interface.** Runtime provisioning and cleanup calls. Clients see the workspace with its checkout mode.

**Persists.** The checkout mode on the workspace. The VM state lives in the remote runtime domain.

**Depends on.** Remote execution hosts (WS and SSH), WS-05, WS-13, WS-16. Cloud services.

**Proposal.** Later. Size XL. Reason: a separate runtime product area. Revisit after remote hosts exist.

### WS-28 Windows runtime per project (Windows host or WSL)

**Behaviour.** On Windows, each local project runs its git commands, agents and terminals either on the Windows host or inside a WSL distribution. A project override (`localWindowsRuntimePreference`: inherit the global default, Windows host, or a named WSL distribution) takes precedence over the global default (`localWindowsRuntimeDefault`, which defaults to the Windows host and can name a WSL distribution). The resolved runtime is reported with a reason: project override, global default, migration fallback, or non-Windows.

**Rules and edge cases.**
- Local git must run in the resolved runtime. A broken runtime state does not fall back silently to host git.
- A project on a WSL distribution keeps its workspaces on that distribution's filesystem (WS-12).
- The choice is made for the project, not per workspace.

**Core interface.** `project.list` and `project.update` (field `localWindowsRuntimePreference`).

**Persists.** The project's preference, and the global default in settings.

**Depends on.** WS-04, WS-12. Terminals. Remote execution hosts (WSL is treated as a host).

**Proposal.** Later. Size L. Reason: real Windows users rely on WSL, but supporting two runtimes per project is a large cross-cutting change. The first version can run on the Windows host only, after the owner decides on WSL.

### WS-29 Workspace records: persistence and live catalog

- Source: pv96 (unmerged). Worktree and folder records keep a manual row name behind the `worktree.manual-display-name.v1` capability, and an identity migration rewrites session references when a worktree id changes.

**Behaviour.** Git is the source of truth for which checkouts exist. MassOrca's store adds:
- identity: project id, host, instance id;
- metadata: the fields listed in WS-19, plus links (WS-22), lineage (WS-21), base ref, sparse setup, creator and automation provenance, and host setup id;
- per-project state: retired names (WS-06), sparse presets, and removal records (WS-16).

Clients receive catalog snapshots stamped with a catalog version, plus change notifications: `worktrees:changed`, `repos:changed`, git status and head-identity updates. Listings reconcile the store with Git. Failed deletes and checkouts that Git no longer lists stay visible as their own rows until resolved (WS-16).

**Rules and edge cases.**
- A workspace id is the repository id and the path. Two hosts that share a path therefore need the host to disambiguate. A bare repository id is never a valid workspace id.
- Renaming a folder changes its path-derived id. Prior ids are kept so sessions created under the old id are still found.
- Writes are batched and saved. Removal records are written before Git deletes anything.
- Storage today is a JSON store file (`orca-data.json`) with sidecar files for heavy or transient data (for example the GitHub cache and removal records). A SQLite profile-state database is being introduced, and the code contains a cutover path.
- A catalog that could not be read is reported as unavailable, not empty.

**Core interface.** `worktree.list` and `worktrees:list`, `worktrees:listAll`, change events, catalog versions. Mobile reads the same catalog through its allowlisted methods.

**Persists.** The store file and its sidecars. The removal sidecar. Profile-state paths.

**Depends on.** Settings and persistence (store, sidecars, migration). Protocol surface (catalog versions and change events). Server and pairing for mobile reads.

**Proposal.** First version. Size M. Reason: a workspace list that survives restarts and stays in step between desktop, mobile and CLI is a Phase 1 requirement.

### WS-30 Profiles: switcher and moving projects between profiles

**Behaviour.** Each profile has its own index entry, data file and state. A project can be transferred from one profile to another. Its workspaces, sessions, lineage, UI preferences, retired names and sparse presets move; the source copy is removed after the transfer. A transfer into a profile that already holds the same repository is reported as a duplicate and stops there. Pending transfers are recovered after a crash through durable move intents. The full profile switcher is hidden behind the environment toggle `ORCA_MULTI_PROFILE_UI=1`. The product direction is an accounts-first interface.

**Rules and edge cases.**
- The source and target must both be known profiles, and they must differ.
- Terminal history cleanup treats workspace ids from other profiles as live, so their shell history is not deleted after a profile switch.
- The transfer touches Orca's records. The reading did not confirm whether it moves any folders; it appears not to.

**Core interface.** `transferOrcaProfileProject` (runtime), not exposed on mobile.

**Persists.** Profile index, per-profile data and state files, and durable move-intent records.

**Depends on.** WS-29. Settings and persistence. Accounts and usage (profile accounts).

**Proposal.** Drop, pending the owner's decision. Reason: the switcher is gated off, and the transfer only matters if profiles stay. Size L.

### WS-31 No Git warning when adding a folder

- Source: pv96 (unmerged). New: adding a folder that is not a git repository shows a dialog that says what the folder loses before the folder becomes a project.

**Behaviour.** The dialog title says the folder is not a Git repository. It lists the features a plain folder lacks, recommends the folder only for exploring and non-development work, and shows the path and the host it will be added on. Its buttons are Cancel (adds nothing), Continue without Git (adds a plain folder project) and Initialize Git here (adds the folder, then makes it a git project on its host). Initialize Git here appears only when the host supports it. Its note says the step runs `git init` and creates an empty first commit, and that files are not committed.

**Rules and edge cases.**
- If the initialise step fails, the folder stays a folder project and the error shows in a toast.
- All footer buttons are disabled while the step runs.

**Core interface.** The desktop dialog adds the folder through the normal project add, then calls `repo.initGit` when the host reports the `repo.init-git.v1` capability.

**Persists.** The folder project; after a successful initialise, a git project at the same path.

**Depends on.** WS-02, WS-03, GIT-04.

**Proposal.** First version. Size S. Reason: a folder project cannot isolate delegated children, so the limit has to be stated when the folder is added.

**Owner direction for the new product (2026-11-12, child placement rules from the design session):** the No Git warning must not block delegation: in a plain folder a child may run in the current folder (placement rule 2), and a writing child then runs sequentially (rule 3). Proposal unchanged.

### WS-32 No Git marker and Initialize Git on folder projects

- Source: pv96 (unmerged). New: a folder project shows a No Git pill on desktop and phone; the pill opens the limits and, on hosts that support it, an Initialize Git action.

**Behaviour.** On desktop, a "No Git" pill follows the project name in the sidebar. Clicking it opens a popover with the limits and, when the host supports it, Initialize Git. On the phone, the pill sits in the repo section header; tapping it opens a sheet with the same limits, the init note, Initialize Git and Close. A successful initialise removes the marker on both clients.

**Rules and edge cases.**
- Git projects never show the pill and never run the support check.
- The support check runs on the first open and is cached per host. A rejected check counts as unsupported, so the action stays hidden.
- On the phone, a failed initialise closes the sheet and shows the host's message in the list's banner.

**Core interface.** `repo.initGit` on the host that owns the folder, gated by the `repo.init-git.v1` capability.

**Persists.** The project kind, on the host.

**Depends on.** WS-02, WS-03, WS-31, GIT-04.

**Proposal.** Later. Size M. Reason: a convenient path to a git project; a plain `git init` in a terminal works in the first version.

### WS-33 One-line worktree sidebar (default layout)

- Source: pv96 (unmerged). New: the sidebar's default card is one line per branch and one line per session, replacing the multi-line card as the default.

**Behaviour.**
- A branch takes one line: a branch icon (a folder icon for a folder workspace, a commit icon with the short SHA for a detached HEAD), its label (the manual row name when set, otherwise the branch without the user's own prefix), and a PR or CI icon on desktop.
- A host or project chip appears only on a same-branch collision, or when the host is disconnected.
- Under a branch, each session takes one line: agent icon, title and one trailing status mark.
- Five root sessions show. Needs-human, failed, working, monitoring and unread sessions never fold. "Show N more" and "Show less" reveal or hide the rest.
- A folded branch or project header shows the worst status of the sessions it hides. Needs-human is an orange icon; red is only for failure; done and seen show nothing.
- A disconnected host dims its rows and reads as unconfirmed, never as exited.
- Environment details (worktree, host, account and path) move to the branch hover card.

**Rules and edge cases.**
- The one-line layout applies to sidebar rows only. The folder panel and the kanban card keep the Compact and Detailed layouts.
- On phones the same rows show generated names until a manual row name is set.
- A host that predates the new fields shows the branch name where a field is missing.

**Core interface.** Built from the worktree catalog and the agent status feed. The host adds optional fields (terminal handle, detached head, own branch prefix, manual row name); older hosts omit them.

**Persists.** The `worktreeCardLayout` setting (WS-45).

**Depends on.** WS-19, WS-29, agents domain.

**Proposal.** First version. Size M. Reason: it is the default workspace list, and needs-human sessions must stay visible in daily work.

### WS-34 Session row actions and hover card

- Source: pv96 (unmerged). New: a session row has its own menu and a hover card that shows its environment or answers a waiting prompt in place.

**Behaviour.**
- Desktop session menu: Rename, Mark Read, Copy Title, Copy Path, Close. Rename changes a chat's label or a terminal's tab title. Close closes the tab.
- Rename and Close are omitted for subagent children, for retained rows, and for children that share their parent's tab. Mark Read is omitted for subagent children. A child that runs in another worktree shows only Copy Title and Copy Path.
- Phone session sheet: Copy Path always; Rename and Close only when the row has a terminal handle, is not a cross-worktree child, and does not share its parent's tab.
- Desktop hover: a needs-human session with a structured prompt shows that question or approval card, answered in place. Other sessions show the Environment section.
- Phone: no hover. A waiting prompt appears once the live status has loaded.

**Rules and edge cases.**
- Renaming a session does not change the row label, which comes from the session's first message or state.
- Per-session unread is not on the wire. While a worktree is unread, every finished session counts as unread.

**Core interface.** Existing tab rename and close commands; the status feed carries the prompt.

**Persists.** The chat label or tab title set by the rename.

**Depends on.** WS-33, agents domain.

**Proposal.** Later. Size M. Reason: convenience on top of the list; the chat and the terminal already offer most of these actions.

### WS-35 Child sessions listed under their parent across worktrees

- Source: pv96 (unmerged). New: a session that runs in another worktree is listed once under its parent, with a branch icon; the child's own branch row stays listed.

**Behaviour.** A session whose parent sits in another worktree leaves its own worktree's list and appears under that parent, with a branch icon whose tooltip reads "Runs on branch <name>". Clicking it focuses the session in its own worktree. The child's own worktree row stays listed. A child with no listed parent stays a root row in its own worktree. Placement follows the lineage parent rules over live and retained sessions, so chains resolve and a cycle lists each session once. A folder or detached-HEAD child has no branch to name and gets no icon; a child that writes nothing and runs in its parent's worktree gets none either. Only the one-line layout places children across worktrees. Dimming and the unconfirmed state follow the host of the worktree the child runs in.

**Rules and edge cases.**
- Closing a child's last tab closes only the tab (WS-16).
- The Agents dashboard still lists such a child in its own worktree (known gap).

**Core interface.** The parent link comes from the host's delegation record; paired clients receive it on the status summary (`delegatedParent`).

**Persists.** Nothing new beyond the delegation record.

**Depends on.** WS-21, WS-33, agents domain (delegation).

**Proposal.** First version. Size M. Reason: Owner direction (2026-11-12): orchestration is in the first version, so child sessions run in their own worktrees and must show under their parent.

**Owner direction for the new product (2026-11-12, child placement rules from the design session):** with the child placement rules, a child normally runs in its own worktree (rule 1) and is listed under its parent as described here; a child that uses the current worktree (rule 2: read-only, plain folder, or picked by hand) stays in the parent's own list. Proposal unchanged.

### WS-36 Checklist card in agent chats

- Source: pv96 (unmerged). New: an agent chat has a shared checklist that the agent writes and the person ticks, and the agent reads the person's ticks on its next turn.

**Behaviour.** The agent writes a fenced `orca-checklist` block in its reply, with `- [ ]` and `- [x]` items and an optional title. A chat has one checklist; the latest block's title wins. The person can tick or untick items, add a note and screenshots to an item, and add or remove items of their own. Each item shows who set its state and when, and the latest of the agent or the person wins. Changes the agent has not seen go with the person's next message as one update block; "Send now" sends them alone. On desktop the card sits in the chat summary under Plan, with an inline Checklist row where the block was. On the phone there is an inline row and a Checklist sheet under Plan.

**Rules and edge cases.**
- Limits: 100 items, 500 characters per item, a 120-character title, a 2000-character note and four screenshots per item.
- Screenshots reach the agent as host paths in the update text, not as image blocks.
- Check and note only; there is no failed state.
- The block format is part of the instructions given to structured chat agents (Claude, Codex, ACP agents, Antigravity and Gentle Shell).

**Core interface.** `agentSessionChecklist.read` and `agentSessionChecklist.setItem`, behind the `agent-session.checklist.v1` capability. An older host answers method-not-found, and the card is read-only.

**Persists.** The person's marks per chat, kept on the host that owns the chat.

**Depends on.** agents domain (chat instructions and send), WS-29.

**Proposal.** Later. Size M. Reason: the owner asked for it for QA tracking, but it is not in the first-version list, so the owner should confirm.

### WS-37 Checklist tab in the floating workspace

- Source: pv96 (unmerged). New: the floating workspace has a Checklist tab that shows one chat's checklist beside the work.

**Behaviour.** A Checklist entry in the floating "+" menu (shortcut Mod+Shift+H) opens a tab that shows the same card and host store as the chat, live in both directions. A session picker lists every session that has a checklist, grouped like the Workspaces sidebar, with search and progress such as "3/7". On Prototipos the default is the session that generated the set in focus; otherwise the chat in focus; otherwise the most recently active session with a checklist. The tab follows focus until a session is picked, and a picked session stays pinned until "Follow focus". A Gentle Shell session is named Gentle Shell, never Pi.

**Rules and edge cases.** A session tab in the floating workspace does not change which checklist the Checklist tab shows (known limit).

**Core interface.** The checklist calls of WS-36.

**Persists.** Nothing new.

**Depends on.** WS-36, terminals domain (floating panel).

**Proposal.** Later. Size M. Reason: a convenience on top of the checklist; the card in the chat works without it.

### WS-38 Open an existing session in the floating workspace

- Source: pv96 (unmerged). New: "Open a session" in the floating "+" menu opens any Workspaces session in the floating panel, by reference.

**Behaviour.** The session opens as a tab of the focused floating group. It is shown only while its tab is active; its owner workspace shows a "shown elsewhere" placeholder meanwhile. Closing the tab gives the session back. A session started from the panel's New Session tab becomes its tab once created. One session is never live in two places. The start screen offers Checklist and Open a session buttons.

**Rules and edge cases.**
- A reference is dormant while the panel is closed or its tab is not active, so the owner workspace shows the session normally.
- A session tab shows a generic icon and the label it had when it was opened (known limit).

**Core interface.** Floating tab kinds for session references and for the New Session tab, in the workspace tab schema.

**Persists.** The reference, in the workspace tab schema.

**Depends on.** WS-33, WS-37, terminals domain.

**Proposal.** Later. Size M. Reason: a parallel-work convenience; the sidebar already opens sessions.

### WS-39 Host selector in the floating workspace

- Source: pv96 (unmerged). New: the floating header has a host selector, and new terminals and chats start on the selected host.

**Behaviour.** A select left of the agent button lists This computer and each paired runtime host, each with a status dot. New terminals and chats start on the selected host, in that host's workspace in focus or its last used workspace, and show in the floating workspace by reference. The default is the host of the focused Workspaces session, else the last used host. Closing a floating terminal ends its host shell; sessions opened with Open a session are handed back, not ended.

**Rules and edge cases.** SSH-only hosts are not listed (known gap).

**Core interface.** Host-aware create calls for terminals and chats on the chosen host.

**Persists.** Not verified.

**Depends on.** WS-38, server and pairing.

**Proposal.** Later. Size M. Reason: a remote creation convenience; paired hosts are already usable from the sidebar.

### WS-40 Start a session in an existing branch or worktree from the create page

- Source: pv96 (unmerged). New: the create page can start a session in an existing branch or worktree, and a workspace with no session shows the create page.

**Behaviour.** The create page places host, project and branch or worktree in one section above the chat composer, so a new session can use a branch or worktree that already exists. A Chat or Terminal switch sits beside Access; Chat is the default, and Terminal shows a note that some features are limited. A workspace with no session shows this page instead of a lone "+ Create" button. The Tasks page opens the same create screen for a task or a session.

**Rules and edge cases.**
- Drafts survive moving between the create page, the Tasks page and Workspaces sessions, through the native chat draft store.
- Built and tested in components; not yet seen in the running app.

**Core interface.** Existing worktree and session create calls; the request carries the chosen branch or worktree.

**Persists.** Drafts in the native chat draft store.

**Depends on.** WS-05, WS-33, agents domain.

**Proposal.** First version. Size M. Reason: a second session in an existing worktree is daily work, and one create page covers it.

### WS-41 Prototypes in the Workspaces side panel

- Source: pv96 (unmerged). New: the Workspaces side panel has a Prototipos entry that lists prototypes.

**Behaviour.** A Prototipos entry sits in the Workspaces side panel and lists prototypes. A prototype is not copied outside its request folder.

**Depends on.** The prototypes feature, not reviewed in this pass.

**Proposal.** Later. Size S. Reason: outside the workspace flow; the prototypes feature decides its own scope.

**Owner direction (consistency pass, 2026-11-12):** Proposal: first version. Owner direction (2026-11-12): the Workspaces Sidebar shows a Prototypes entry with its count (design D-WS-01; prototypes core, decided on the owner's behalf, pending review).

### WS-42 Show bot teams filter in Workspaces

- Source: pv96 (unmerged). New: a Show bot teams filter, off by default, controls whether bot team folders appear in the Workspaces list.

**Behaviour.** While the filter is off, bot team folders stay out of the Workspaces list. Turning it on shows the team folders group. Session history marks bot team sessions.

**Depends on.** bot-teams domain (`bot-teams.md`), WS-11.

**Proposal.** Later. Size S. Reason: belongs to the bot-teams feature, whose scope is decided there.

### WS-43 CLI: list and close a worktree's agent sessions

- Source: pv96 (unmerged). New: `orca worktree sessions` lists a worktree's agent sessions, and `orca worktree session-close` closes one.

**Behaviour.** `orca worktree sessions --worktree <selector> [--json]` lists the chats and terminal agents under a worktree. `orca worktree session-close --worktree <selector> --session <id> [--json]` closes one, using the id the listing prints.

**Depends on.** WS-33, agents domain, CLI domain.

**Proposal.** Later. Size S. Reason: a scripting convenience; the sidebar does the same job.

### WS-44 CLI: folder workspace commands

- Source: pv96 (unmerged). New: `orca folder-workspace` lists, adds, changes, stops tracking and checks folder workspaces.

**Behaviour.** `orca folder-workspace list`, `add` (into a project group), `rename`, `move`, `archive`, `pin`, `comment`, `rm --workspace <id>` (stops tracking; the folder stays on disk) and `check` (confirms that a folder workspace, project group or path still exists as a folder).

**Depends on.** WS-09, WS-11, CLI domain.

**Proposal.** First version. Size S. Reason: the Phase 1 exit needs the CLI to manage workspaces for worktrees and folders.

### WS-45 Sidebar card layout choice

- Source: pv96 (unmerged). New: the sidebar card layout can be one line, detailed or compact; an unset preference shows one line.

**Behaviour.** The `worktreeCardLayout` setting takes one-line, detailed or compact. An unset value shows one line. A profile with `compactWorktreeCards` on keeps the compact layout. Before settings load, the sidebar shows the detailed card. The experimental card style still wins. The folder panel and the kanban card read the same resolved layout.

**Persists.** `worktreeCardLayout`, and the older `compactWorktreeCards`.

**Depends on.** WS-33.

**Proposal.** Later. Size S. Reason: a display preference; the one-line default covers the first version.
