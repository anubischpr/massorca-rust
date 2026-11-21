# Source-control providers and issue trackers: functional inventory

This inventory describes what the current MassOrca app does for hosted code review and issue tracking. It covers pull and merge requests on GitHub, GitLab, Bitbucket Cloud, Azure DevOps and Gitea/Forgejo, and issues or work items on GitHub, GitLab, Jira and Linear. GitHub and GitLab run through their own command-line tools (`gh` and `glab`) and reuse the login those tools already hold. No OAuth flow exists for any provider. GitHub sign-in can start in the app through gh's device-code login (pv96). Bitbucket Cloud can be connected in the app or through environment variables. Azure DevOps and Gitea take a token and base URL saved in the Marketplace on the host, or environment variables (pv96; environment variables only before). Jira and Linear take a personal token or API key entered in the app. GitHub is by far the deepest integration: about 20,000 lines in the source directories, covering per-repository account binding, fork and upstream resolution, check rollups, review threads, stacked pull requests, a GitHub Projects board and a rate-limit budget. The other providers offer branch lookup and pull request creation only. No provider supports submitting a review verdict (approve or request changes).

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits). GitLab, Bitbucket, Jira and Linear have no changed files in pv96. The pv96 changes in this domain are GitHub account handling, Azure DevOps and Gitea credentials, feedback reports, and the Tasks page.

## Provider coverage at a glance

| Provider | Connection | Branch review lookup | Create review | Checks | PR/MR comments | Merge and state | Issues | Self-hosted |
|---|---|---|---|---|---|---|---|---|
| GitHub | gh CLI login, or in-app device-code sign-in (pv96); per-repo account binding and host-wide login switch | Yes, with fork and merged-history rules | Yes: drafts, templates, base, stacks | Check runs and legacy statuses; rerun; job logs | Yes: conversation, inline threads, resolve, reactions | Merge, close, reopen, draft, title, auto-merge, merge queue | Browse, edit, comment; Projects board | GitHub Enterprise Server |
| GitLab | glab CLI login; env tokens for diagnostics | Yes (merge requests) | Yes: drafts, default template, fork MR | Pipelines, bridges, child pipelines, job traces, retry | Yes: discussions, resolve, inline notes | Merge (squash or rebase flags), close, reopen, draft | Browse, edit, comment on issues and MRs | Self-managed hosts, ports kept |
| Bitbucket Cloud | In-app token or email plus API token; or env | Yes | Yes, no drafts | Build status only | No | No | No | Cloud only |
| Azure DevOps | Token saved in the Marketplace on the host (pv96), or env token (PAT or access token) | Yes | Yes | Commit status | No | Mergeable state shown only | No | Cloud and Azure DevOps Server |
| Gitea and Forgejo | Token and API base URL saved in the Marketplace (pv96), or env | Yes (shared scan cache) | Yes | Commit status | No | Mergeable state shown only | No | Any host, URL subpaths |
| Jira | In-app per site: Cloud email plus token, or self-hosted PAT or user and password | No | No | No | Not applicable (see SCM-20) | No | Browse, search, edit, transitions, create, attachments | Server and Data Center |
| Linear | In-app personal API key per workspace | No | No | No | Not applicable (see SCM-21) | No | Browse, search, edit, create, relations, projects | No (SaaS) |

### Shared and provider-specific parts

- **Shared.** One dispatch surface (`hostedReview` requests: branch lookup, create, create stacked, creation eligibility) routes to a single provider union: github, gitlab, bitbucket, azure-devops, gitea, unsupported. Shared vocabularies cover PR state (open, closed, merged, draft), check status (pending, success, failure, neutral), mergeable state (mergeable, conflicting, unknown) and review decision (approved, changes requested, review required). Workspace creation from an item uses one source vocabulary (GitHub PR or issue, GitLab MR or issue, branch, Linear issue, Jira issue).
- **Shared rule.** A merged PR or MR matched only by branch name is history, not review context, and stays hidden unless the worktree HEAD is part of it. A non-open branch match on the default branch is discarded. GitHub, GitLab, Azure DevOps and Gitea all carry this guard.
- **Shared failure rule.** On rate limits, outages or partial reads the UI keeps the last known status with a banner. Hard auth or permission failures show empty-state copy that tells the user what to fix.
- **Provider-specific.** GitHub: account binding, host-wide login switch and removal, in-app sign-in, Projects, stacks, auto-merge, Actions logs, reactions, rate-limit buckets. GitLab: pipelines and bridges, discussions, approval state (read only). Jira: Atlassian document format conversion, transitions, attachment media. Linear: relations, custom views, agent command surface.

## Source directories read (repository-relative, reference repo)

- `src/main/github/` (all files and `client/` subtree: rule comments, exports, auth, rate limit, error classification, project-view files by listing)
- `src/main/gitlab/` (all files: rule comments, exports, auth and rate-limit headers, mapping and creation headers)
- `src/main/bitbucket/`, `src/main/azure-devops/`, `src/main/gitea/` (file lists, rule comments, auth configuration, exported names)
- `src/main/jira/` (file list, rule comments, exported names, token storage, auth header)
- `src/main/linear/` (file list, rule comments, exported names, token storage, MCP list header)
- `src/shared/github/`, `src/shared/linear/` (file lists and type names; not read in full)
- `src/preload/gitlab.ts` (header and channel list)
- `src/main/issue-command-file.ts` (issue command read and write)
- `src/main/ipc/` (channel names by grep; `hosted-review.ts` head; `bitbucket.ts`, `github-account-handlers.ts`, `linear.ts` heads; GitHub, GitLab, Jira and Linear handler file names)
- `src/main/runtime/` (RPC method names by grep; `rpc/methods/linear-agent-access.ts`; `runtime-repository-issue-command.ts` comments)
- `src/main/source-control/` (file names and the provider type union)
- `src/main/git/command-runner/` (gh and host fallback comments only)
- `src/shared/new-workspace/workspace-source.ts`, `src/shared/pull-request-generation.ts` (heads)
- `docs/site/content/docs/review/` (github, linear, jira, commit-push), `docs/site/content/docs/model/orca-yaml.mdx` (issue command section), `docs/site/content/docs/github-errors.mdx` (sections), `docs/site/content/docs/ssh.mdx` (grep), `docs/site/content/docs/cli/reference.mdx` (headings and Linear section), `README.md` (grep)

Method: CodeGraph has no index in the reference repository. It was not initialized there because the brief forbids writes to that tree, so exploration used Grep, Read and Glob. Test files were not read; only their file counts (153 in scope) were noted.

### Added for pv96 (delta pass)

- Odd docs at `d3c6311a531`: `odd/tasks/desktop-simplification.md` (the Tasks page part) and `odd/tasks/worktree-one-line-sidebar.md` (the PR and CI icon only).
- `src/main/github/` (`gh-auth-sign-in.ts`, `gh-account-switch.ts`, `gh-account-logout.ts`, `gh-account-remote-client.ts`, `issue-create.ts`): names, header comments and contract types; the sign-in and relay bodies were not read in full.
- `src/main/gitea/` (`client.ts`, `pull-request-creation.ts`) and `src/main/azure-devops/` (`azure-devops-api-request.ts`, `pull-request-creation.ts`): credential source lines and the changed helpers.
- `src/main/marketplace/marketplace-service-credentials.ts` and the vault header (`marketplace-vault.ts`): stored token and base URL rules only.
- `src/main/marketplace/host-programs/host-program-catalog.ts`: program ids and install methods.
- `src/main/ipc/github-account-handlers.ts` (channel names), `src/main/ipc/feedback.ts`, `src/shared/massorca-distribution.ts` (the repository constant), `src/shared/github/auth-types.ts`, `src/shared/rpc-contract/github-account-binding-params.ts`, and `src/preload/api/github-account-api.ts` (method names).
- RPC method names diffed across `src/main/runtime/rpc/methods` and `src/main/ipc`: added `github.switchActiveAccount` and `github.removeAccount`; none removed.
- Unchanged in pv96 (no changed files): `src/main/gitlab/`, `src/main/bitbucket/`, `src/main/jira/`, `src/main/linear/`, `src/main/source-control/`.

## Not read or low confidence

**Not read, or sampled only through comments and names**
- GitHub: GraphQL query text, mapper bodies, branch-lookup state machines, stacked PR creation and merge internals (`github-pr-stack*`, `stacked-pr-creation`), the PR refresh coordinator bodies (names and pacing constants only), conflict-summary bodies beyond comments, gh process execution internals.
- GitLab: bodies of merge request creation, lookup and state mutations; approval and reviewer bodies (signatures only); pipeline job mutations (retry only).
- Bitbucket, Azure DevOps, Gitea: only auth, lookup, creation and mapping names and comments.
- Jira: search and listing bodies, Atlassian document format conversion, attachment image pipeline and transition logic (names and comments only).
- Linear: the roughly 20 `runtime-linear-*` command files (names only), issue listing and mutation bodies, custom views, projects, legacy viewer migration (names and comments only), MCP list (header only).
- Runtime and IPC handler bodies, except the heads named above. GitHub work item and project-view handlers were used by channel name only.
- `src/shared/github/`, `src/shared/linear/`, `src/main/source-control/`: names and type unions only.
- Renderer and mobile UI: outside the brief. Their behaviour is taken from the product docs.
- Docs not read: `review/diff-viewer`, `review/annotate-ai-diff`, `review/attribution` (diff-level, outside this domain), most of `github-errors`, most of `cli/reference`, most of `ssh`.

**Low confidence**
- Whether the bound GitHub account also sets the commit author identity, or only pins the project (the code says "pins a project to a GitHub account").
- Behaviour of the Gitea client when no token is set (status reports whether a token is configured).
- Purpose of the GitLab to-do call (`gitlab:todos`): an IPC channel exists, with no user documentation found.
- Jira transition ordering and board-based status order.
- Whether PR-based workspace starts apply to folder workspaces (no branch exists there).
- Whether Bitbucket or Gitea creation supports templates.
- The exact option set of the per-repo issue source preference. Only "origin" (origin-only), "upstream" and "auto" are implied by code comments.
- Whether any CLI command covers GitHub, GitLab or Jira. The CLI reference headings were checked, and only Linear appeared.

**pv96 delta, not read or low confidence**
- The Tasks page body (from the `desktop-simplification.md` notes only); the approvals list and the gated WorkSpaces or Bot Team switch were not traced in code.
- The GitHub sign-in body and the device-code parsing (header comments read); the relay client bodies (header and contract only).
- The Marketplace GitHub page, the feedback form and the mobile GitHub account screens (renderer and mobile not read).
- The Azure DevOps and Gitea pull request bodies (credential lines only); whether the creation flow changed beyond the credential source.
- Whether the feedback dialog's remaining crash-report path changed: `src/main/ipc/crash-reporting-submission.ts` changed, but its body was not read.

## Features

Features with a proposed first-version mark: SCM-01, SCM-03, SCM-07, SCM-08, SCM-09 and SCM-22 (owner direction). Everything else is later; SCM-30 and SCM-32 are dropped. Sizes and marks are in the feature table.

### SCM-01 GitHub connection and diagnostics

- Source: pv96 (unmerged). GitHub sign-in can start from the app through gh's device-code login (`gh auth login --web`), and the GitHub CLI can be installed from the Marketplace; MassOrca still keeps no token.

**What the user does and what happens**
- The user can sign in from the app (pv96). MassOrca starts `gh auth login --web`, reads only the one-time code and verification address from gh's output, and reports them to the renderer, which opens the address. gh keeps the token. The user can cancel, and a sign-in not finished within 10 minutes ends. The terminal route (`gh auth login`) still works.
- The app shows the signed-in login (viewer) and a diagnosis of the gh auth state, including which accounts gh knows for each host.
- The diagnosis is scoped to a host when the caller names one (for example a GitHub Enterprise origin). Project queries that fail with a missing scope get a canned explanation of the scope needed.
- If gh is not found, the app reports the GitHub CLI as unavailable. The fix is to install gh for the same environment MassOrca runs in (WSL or native Windows), then restart so PATH matches.

**Rules and edge cases**
- MassOrca does not persist GitHub tokens. For a bound account it runs `gh auth token` for that login and host, keeps the result in memory for 5 minutes (30 seconds when empty), and never logs it.
- For bound calls, ambient `GH_TOKEN`, `GH_HOST` and `GH_REPO` are stripped so a shell-level login cannot override the bound account. Docs list an exported `GITHUB_TOKEN` or `GH_TOKEN` in the shell profile as the most common auth footgun.
- Prompts are disabled on gh child processes (`GH_PROMPT_DISABLED`) so gh never blocks on input.
- Typical auth failures named in the docs: expired or revoked token, organisation SAML SSO not authorised, missing scope.
- Windows: the token and the prompt flag are passed into a WSL child through WSLENV. Auth state is per execution host (native, each WSL distro, SSH relay).

**Interface and data**
- Clients ask: `gh:viewer`, `gh:diagnoseAuth` (optionally with a host), `gh:rateLimit`, and for sign-in `gh:startSignIn` and `gh:cancelSignIn` (IPC only; progress arrives on `gh:signInEvent`). RPC equivalents exist under `github.*`.
- Persists: nothing for auth. Caches are in memory only.
- Depends on: none.

### SCM-02 Per-repository GitHub account binding

- Source: pv96 (unmerged). The per-repository binding rules are unchanged; host-wide switching and removal of GitHub logins are new and listed as SCM-31.

**What the user does and what happens**
- The user pins a project to one of the GitHub logins that gh knows on the repository's host. The app lists bindable accounts per host, validates the chosen binding against gh's inventory, and lists orgs for an account.
- Account inference suggests the login whose organisations cover the origin's owner.
- The pin is also written to repository-local git config so the user's terminal `gh` wrapper reads the same account.

**Rules and edge cases**
- A binding is a host plus a login. Hosts are compared in lower case. Logins may contain dots or underscores (GitHub Enterprise and LDAP names).
- When the gh on that host is too old for multi-account support, binding creation is not offered.
- Inference reads the origin through the same runner. An SSH relay answers with the remote's origin, and only logins on the remote's host are candidates.
- The binding applies per execution host. A local repository and an SSH repository at the same path do not share auth state.
- Reading and writing the git-config pin runs on the repository's own host (local, WSL or SSH relay).

**Interface and data**
- Clients ask: `gh:listBindableAccounts`, `gh:listHostBindableAccounts`, `gh:inferRepoAccount`, `gh:validateAccountBinding`, `gh:listAccountOrgs`, `gh:applyAccountGitConfig`, `gh:readAccountGitConfig`.
- Persists: per-project binding (host and login) in the project record; a repo-local git config key that pins the account.
- Depends on: SCM-01.

### SCM-03 Hosting provider detection and repository identity

**What the user does and what happens**
- From the repository's git remotes (origin and, when present, upstream) the app works out which provider hosts the repository and its owner and repo name.
- Recognised hosts: github.com and GitHub Enterprise; gitlab.com and self-managed GitLab hosts that glab knows (port-aware); bitbucket.org; dev.azure.com, `*.visualstudio.com`, `ssh.dev.azure.com` and Azure DevOps Server; Gitea and Forgejo (matching rules low confidence). Anything else is "unsupported".
- Issues have a per-repository source preference. Origin, upstream and auto are the options implied by the code. Only an explicit origin preference restricts results to origin.

**Rules and edge cases**
- Fork checkouts: pull requests live on the upstream parent, not on origin. Lookups probe every candidate repository (origin and upstream) before giving up.
- Hosted-review creation targets the remote that owns the unqualified head branch and keeps that remote's host.
- SSH aliases: the effective host is found by asking ssh for the resolved configuration. A failed ssh probe is not treated as "not GitHub". Only stable "no such remote" misses are held for minutes.
- GitHub Enterprise over SSH has no API port in the remote URL. The host is identified only if it is unique in gh's auth inventory. SSH push to Enterprise on port 443 uses the `ssh.<host>` form.
- GitLab: "not GitLab" holds only until origin changes or the user logs in. Probes are cached per host and per execution host, and a stale probe cannot overwrite a newer answer.
- Azure DevOps Server remotes are identified only by the `_git` path convention.
- Port rules: an HTTP port identifies a GitHub Enterprise web/API endpoint. An SSH or git port does not.

**Interface and data**
- Clients ask: `gh:repoSlug`, `gh:repoUpstream`, `gitlab:projectSlug`, `hostedReview:forBranch` (which resolves the provider).
- Persists: the per-repository issue source preference. Other results are in-memory caches.
- Depends on: none.

### SCM-04 Connect GitLab, Bitbucket Cloud, Azure DevOps and Gitea/Forgejo

- Source: pv96 (unmerged). Azure DevOps and Gitea take a token and base URL saved in the Marketplace on this host before the environment variables, and a saved Azure DevOps token outranks an environment access token.

**What the user does and what happens**
- GitLab: connection comes from `glab` login. The app reads the viewer through the API user endpoint and reports auth diagnostics. Diagnostics show whether `GITLAB_TOKEN` or `GLAB_TOKEN` is set in the environment.
- Bitbucket Cloud: the user connects in-app from the Source Control pane using "Email and API token" or an "Access token". The app verifies the credential before saving. Environment variables `ORCA_BITBUCKET_*` win over a saved credential.
- Azure DevOps: a token and base URL saved in the Marketplace on this host win (pv96), and the environment is the fallback. A PAT (`ORCA_AZURE_DEVOPS_TOKEN` or `_PAT`, with an optional `_USERNAME`) is sent as Basic auth. An access token (`ORCA_AZURE_DEVOPS_ACCESS_TOKEN`) is sent as Bearer. `ORCA_AZURE_DEVOPS_API_BASE_URL` points to an on-premises server.
- Gitea/Forgejo: a token and base URL saved in the Marketplace on this host win (pv96), and the environment is the fallback. `ORCA_GITEA_TOKEN` is sent as a token header and `ORCA_GITEA_API_BASE_URL` gets `/api/v1` appended when missing.

**Rules and edge cases**
- Bitbucket credential failure: only credential-bearing HTTP rejections mark the saved credential as bad. Timeouts, DNS failures and 5xx responses do not, so a network blip cannot discard a good login.
- Bitbucket credential storage: secret encrypted through the OS secret store; plain metadata kept beside it. A denied keychain prompt leaves no usable credential.
- Azure DevOps and Gitea have no connection form of their own in the code; the Marketplace on the host is where a token and base URL can be saved from the app (pv96). A saved Azure DevOps token is sent as a PAT and outranks an environment access token.
- No provider offers OAuth.

**Interface and data**
- Clients ask: `bitbucket:status`, `bitbucket:disconnect`, `bitbucket:connect`; GitLab `gitlab:diagnoseAuth`, `gitlab:viewer`, `gitlab:rateLimit`.
- Persists: Bitbucket credential metadata and an encrypted secret. Azure and Gitea hold nothing when only the environment is used; a saved token and base URL sit in the host's sealed Marketplace vault (pv96). GitLab holds nothing (CLI).
- Depends on: SCM-03.

### SCM-05 Jira site connection

**What the user does and what happens**
- In the Tasks view the user picks Jira and clicks Connect. The dialog offers Cloud (site URL, Atlassian email, API token) or Self-hosted (base URL including any path, then either a personal access token or username and password).
- The app verifies the credentials and loads the sites. Several sites can be connected; a site picker offers "All sites" to combine issues.
- Validation: a missing email or token, or a missing PAT for the PAT option, is refused with a specific message.
- Disconnect removes the site, its token and its cached attachment images.

**Rules and edge cases**
- Authorization: Cloud uses Basic with email and token. A self-hosted site with no username is treated as a PAT sent as Bearer. Basic with a PAT in the password slot is a known source of 401 errors.
- Credentials are sent only to the configured site. Attachment downloads never forward Jira credentials to other hosts, because attachment metadata is supplied by the provider.
- An authentication error clears the site's token and removes the site from the connected list. Some 403 responses are treated as project or permission gaps rather than auth failures, per code comments.
- Storage: site list in a file with owner-only permissions. Token encrypted through the OS secret store. When the store is unavailable (typical on some Linux setups), the token is written as plaintext with a logged warning.
- HTTPS is recommended in the dialog because credentials travel in the Authorization header.

**Interface and data**
- Clients ask: `jira:connect`, `jira:disconnect`, `jira:selectSite`, `jira:status`, `jira:readStatus`, `jira:testConnection`.
- Persists: site list and encrypted token per site; attachment image cache in memory with eviction.
- Depends on: none.

### SCM-06 Linear workspace connection

**What the user does and what happens**
- The user opens Settings, Integrations, Linear and pastes a personal API key from Linear's API settings. The app checks the key with a Linear call, takes the workspace identity from the viewer, and then the user picks teams to show.
- Several workspaces can be connected. A selection of "all" combines them. Under "all", one workspace whose secret cannot be decrypted does not block the others.
- Status sync is optional per team: moving an issue to "In Progress" when a worktree is created.

**Rules and edge cases**
- A 401 from Linear clears the stored token and prompts for re-authentication.
- Failure messages are sanitised: raw SDK errors are not shown, and bearer tokens are redacted.
- Startup warms only plaintext metadata. Tokens are decrypted when needed.
- Storage: workspace metadata in a registry; token per workspace, encrypted through the OS secret store. The same plaintext fallback as Jira applies when the store is unavailable.
- A legacy viewer file is migrated and then kept as loaded-and-empty once removed (low confidence on details).

**Interface and data**
- Clients ask: `linear:connect` (API key), `linear:status`, `linear:selectWorkspace`, `linear:disconnect`, `linear:testConnection`.
- Persists: workspace registry and encrypted token per workspace; team selection.
- Depends on: none.

### SCM-07 Branch review lookup and sidebar status

- Source: pv96 (unmerged). The one-line sidebar row shows the PR or MR state and a failed-check mark as one tinted icon on desktop, and the phone keeps the `#N` badge.

**What the user does and what happens**
- Each worktree's branch is matched to its pull or merge request on GitHub, GitLab, Bitbucket, Azure DevOps or Gitea. The sidebar card shows state (open, draft, merged, closed) and, on GitHub, a red chip when Actions checks fail.
- The branch context row shows a compact "Open review page in browser" link for every provider that gives a URL.
- Per PR actions on GitHub: copy the review link, close, or reopen after confirming the state change.
- Detached HEAD: branch lookup is impossible, but an explicitly linked PR number is still queried.

**Rules and edge cases**
- An explicit link (a stored PR number) is the source of truth. If the linked PR was deleted or belongs to another repository, the app falls back to branch discovery. GitHub and GitLab can also fall back by number on a cheap exact lookup.
- Merged history: a merged PR matched only by branch name is hidden. A merged PR stays visible when the worktree HEAD is its head commit or one of its commits, including on secondary worktrees. Merged-commit membership answers are definitive and cached.
- Abandoned Azure DevOps PRs with newer close dates do not hide a usable branch PR.
- Upstream tracking: the tracked remote branch identifies the real head, even when a local branch name matches another PR. Fork owner can identify the head.
- On a fork checkout, the PR can live on upstream; lookups probe both (SCM-03).
- Lookup prefers the cheap REST listing for identity and the exact view for review and merge-queue state.
- Failure: rate limits and outages keep the last known state with a banner. Missing remotes give no lookup. Failed lookups are cached with a cooldown.
- SSH and WSL: tracked-upstream state cannot be inspected cheaply on the remote, so its cache has a short TTL. While an SSH host reconnects, the sidebar can show persisted metadata.
- Folder workspaces have no branch and so no review lookup (inferred from the branch model).

**Interface and data**
- Clients ask: `hostedReview:forBranch`, `gh:prForBranch`, `gitlab:mrForBranch`; `gh:reportVisiblePRRefreshCandidates` tells the core which cards are on screen.
- The core pushes back refresh events for changed review state (`gh:workItemMutated`, PR refresh events).
- Persists: the explicit PR or MR link on the workspace (medium confidence, inferred from linked-item lifetime tests). Everything else is in-memory and bounded.
- Depends on: SCM-01, SCM-03, SCM-08.

### SCM-08 Status refresh pacing and rate-limit handling

**What the user does and what happens**
- PR status for all worktrees refreshes in the background within a budget. Visible worktrees come first.
- Settings, Git, GitHub API Budget shows the remaining REST, Search and GraphQL counts, with a manual refresh. On a remote Orca server the budget view shows the server's own gh identity.
- When a limit is hit, the PR and Checks panels keep the last known status and show a banner. Hard auth or permission failures show empty-state copy instead.

**Rules and edge cases**
- Background refresh: at most 20 background refreshes in any 5-minute window and at least 10 seconds apart. Active worktrees get at most 3 refreshes in a 30-second burst. The queue growth is bounded.
- GitHub rate-limit buckets are core, search and GraphQL. The snapshot is cached for 30 seconds, and failures are cached too, so a host that rejects the probe does not spawn gh repeatedly. A primary 403 opens a short breaker for that bucket. One exempt probe then refines the reset time.
- A snapshot from before its reset is treated as expired (fail open).
- Quota scopes are separate: github.com (native), each GitHub Enterprise host and each WSL runtime keep their own snapshot and breaker.
- GitHub search budget is 30 requests per minute. Result counts are cached for 120 seconds and stop when the budget is exhausted, because counts are decorative.
- GitLab rate-limit snapshots are cached for 30 seconds per host, for up to 64 hosts. GitLab omits budget headers inconsistently, so a missing header is recorded as unknown.
- Comments are cached for 60 seconds on normal loads. An explicit refresh bypasses the cache.
- Gitea uses a pull request scan cache: the list scan is cached long enough to absorb a push-event burst, and failures get a short cooldown.

**Interface and data**
- Clients ask: `gh:rateLimit` (with force), `gitlab:rateLimit`, `gh:enqueuePRRefresh`, `gh:refreshPRNow`.
- Persists: nothing; all state is in memory.
- Depends on: SCM-01, SCM-03.

### SCM-09 Create a pull or merge request from a branch

**What the user does and what happens**
- After the branch is pushed, the Source Control panel opens a hosted-review dialog. The user confirms base, title, description and draft state. The branch context row keeps the current branch visible.
- "Generate pull request details with AI" can fill title, description and draft (SCM-17).
- If a follow-up commit fails (for example a hook rejects it), the dialog shows the hook output and offers "Fix with AI" or a manual retry.
- Creation is a remote action; the app records no local PR object. The link is set from the lookup afterwards.

**Rules and edge cases**
- Templates: GitHub reads the first existing file among the `.github` and root variants of `pull_request_template.md`, and `docs/` variants. GitLab reads `.gitlab/merge_request_templates/Default.md`.
- Drafts: GitHub and GitLab support them. Bitbucket Cloud has no drafts, so the toggle is hidden. GitLab draft state comes from either a boolean field or a title prefix, depending on the API generation.
- GitHub stacking appears when the chosen base already has an open PR (SCM-10).
- Eligibility is evaluated before submit, including for the sidebar. A dedicated eligibility call says whether creation is blocked and why.
- Ambiguous failures: Bitbucket and Azure DevOps say "creation may have completed" and refresh the branch review state rather than report failure.
- Azure DevOps Server rejects versioned requests without the preview suffix; the app retries with the preview version and remembers the origin.
- Per the docs, the worktree creation flow checks Azure DevOps and Gitea pull requests for remote conflicts before pushing (exact trigger not traced).
- Fork PRs: creation targets the origin owning the head branch and keeps its host.
- Error classification distinguishes existing-PR and validation failures (exact wording not traced).

**Interface and data**
- Clients ask: `hostedReview:create`, `hostedReview:getCreationEligibility`; GitHub, GitLab, Bitbucket and Azure keep their own creation paths underneath.
- Persists: nothing beyond the link the lookup later finds.
- Depends on: SCM-01, SCM-03, SCM-07. Git push and branch state belong to the git domain.

### SCM-10 Stacked GitHub pull requests

**What the user does and what happens**
- When the chosen base already has an open PR, the create composer offers "Stack this PR above #N". Checking it creates a GitHub Stack or extends the parent's stack. The submit label changes (for example "Create PR in stack").
- A preview shows the parent PR and the new branch.
- The PR sidebar shows a collapsible "Stack #N" map: position, stack size, base branch, and each layer's status (open, draft, checks pending or failed, review needed, conflicts, merged, closed). Clicking a layer navigates to it.
- Stack-aware merge: the merge label becomes "Merge through #N, M PRs" (or "Queue through" with a merge queue), covering the PR and every PR below it. The confirmation lists the numbers when stack metadata is complete.

**Rules and edge cases**
- GitHub only. Offered only when the executing host supports stacked creation. GitLab and Bitbucket stay single-PR.
- Atomic merge: if any layer cannot merge, none merge.
- Submission is asynchronous. A transient poll failure after submission does not make the merge fail, because the merge was already submitted.
- Ordinary dependent chains without GitHub stack metadata keep the single-PR merge flow.
- The stack summary cache keeps a failed probe as a failure with a short cooldown, so it is never read as "no stack".

**Interface and data**
- Clients ask: `hostedReview:createStacked`; GitHub stack reads and merge calls run through the GitHub client.
- Persists: nothing locally. Stack membership lives on GitHub.
- Depends on: SCM-09, SCM-15, SCM-01.

### SCM-11 Checks and CI status details

**What the user does and what happens**
- GitHub: the Checks panel lists check runs and legacy status contexts with per-check status and annotations. Failed job logs open inline. "Fix broken checks" hands the failed check names and links to an agent.
- GitLab: the Checks panel shows pipeline jobs, including bridge and child-pipeline jobs. Expanding a job loads its trace where available. Retry is available for real jobs.
- Bitbucket, Azure DevOps and Gitea: a derived build or commit status only, with no logs and no reruns.

**Rules and edge cases**
- GitHub rollup: uses the rich check-run query first, then falls back to the older `gh pr checks` output when the rollup is unavailable. Zero check runs is an empty state, not an error. Legacy status contexts sit beside check runs; name collisions keep the check-run metadata.
- GitHub reruns are not idempotent. The result reports how many reruns already started, so a retry is never blind.
- Job logs: only the tail is kept, and each entry holds up to 16 KB. The log cache is bounded. Log fetches are capped so failed-job details stay a bounded follow-up.
- GitLab: child pipelines expand to at most 20 per MR, with at most 4 concurrent fetches. Bridges have no real job id, so they cannot be retried. A blocked pipeline on a human gate reads as manual. Skipped and canceled pipelines read as neutral. An empty job list means nothing to report.
- GitLab over SSH: each job fetch runs glab on the remote (SCM-29).
- Status vocabulary: pending, success, failure, neutral.

**Interface and data**
- Clients ask: `gh:prChecks`, `gh:prCheckDetails`, `gh:rerunPRChecks`, `gitlab:jobTrace`, `gitlab:retryJob`.
- Persists: nothing; caches are bounded and in memory.
- Depends on: SCM-07, SCM-08.

### SCM-12 PR and MR comments and review threads

**What the user does and what happens**
- The user reads the conversation: issue comments, review summaries (approve, request changes, general) and inline review threads. Sections group comments newest-first. The timeline stays oldest-first.
- The user adds a conversation comment, adds an inline review comment, replies to any comment in a thread, and resolves or reopens a thread.
- On GitHub: react with one of eight reactions. Existing reactions toggle. Edit or delete issue comments.
- GitLab: add a discussion note or an inline note with position, and resolve or reopen the whole discussion (not a single note).

**Rules and edge cases**
- GitHub comment loading is partial by design: each endpoint is fetched independently, and a failing source contributes nothing rather than blanking the view.
- Review thread IDs and resolution state are GraphQL-only. REST omits them.
- Outdated inline comments (after a force push) lose their current line numbers, so the original line is kept.
- GitLab: system-generated activity notes are skipped. Detail views show a bounded recent snapshot. Notes do not include a per-note web URL, so the app builds it.
- Bitbucket, Azure DevOps and Gitea: no comment or thread support in the code.

**Interface and data**
- Clients ask: `gh:prComments`, `gh:addIssueComment`, `gh:addPRReviewComment`, `gh:addPRReviewCommentReply`, `gh:resolveReviewThread`, `gh:setPRCommentReaction`, `gitlab:addMRComment`, `gitlab:addMRInlineComment`, `gitlab:resolveMRDiscussion`.
- Persists: nothing locally.
- Depends on: SCM-07.

### SCM-13 PR and MR file contents and viewed state

**What the user does and what happens**
- Review of a pull request shows file contents for the changed files. On GitHub the user marks files as viewed or unviewed. The state is VIEWED, UNVIEWED or DISMISSED.
- GitLab diffs are fetched per file.

**Rules and edge cases**
- The viewed toggle is the highest-volume GraphQL caller on GitHub. It fires once per file while the user reads a diff.
- GitLab deprecated the all-in-one changes endpoint in favour of per-file diffs. The diff parser treats `---` and `+++` as headers only before the first hunk.
- The diff viewer itself is outside this domain.

**Interface and data**
- Clients ask: `gh:prFileContents`, `gh:setPRFileViewed`.
- Persists: viewed state lives on GitHub.
- Depends on: SCM-07.

### SCM-14 Reviewers and review state

**What the user does and what happens**
- On GitHub and GitLab, the user requests reviewers or removes them. The app shows the reviewer list (including bot-only reviewers) and the review decision.
- GitHub review decision values: approved, changes requested, review required.
- GitLab approval state is read only.

**Rules and edge cases**
- GitHub reviewer data comes from GraphQL merge metadata. A REST fallback drops review detail and would blank bot-only reviewer lists, so the app does not fall through to REST for reviewers.
- No provider offers submitting an approve, comment-only or request-changes verdict. The only write paths are reviewer requests and thread actions.

**Interface and data**
- Clients ask: `gh:requestPRReviewers`, `gh:removePRReviewers`, `gitlab:updateMRReviewers`.
- Persists: nothing locally.
- Depends on: SCM-07.

### SCM-15 Merge, close, reopen, draft and title edits, conflict summary

**What the user does and what happens**
- Merge a PR or MR using the repository's allowed method (GitHub: squash, merge commit, rebase; GitLab: squash or rebase flags, default regular merge commit).
- Close and reopen. Mark a draft ready for review. Edit title and body.
- Before merging, the app checks for conflicts and shows a conflict summary when available.

**Rules and edge cases**
- GitHub merge methods shown are limited to what the repository allows, and the default follows the repository.
- GitHub reopen uses gh's reopen flow, because a REST state change can fail with 422 on reopen.
- GitLab: a close of an already-closed MR is treated as success.
- Conflict preflight runs local git, which is skipped for SSH repositories until that helper is routed through the relay. A preflight failure does not block the merge.
- Conflict summary: a pure function of the two commit IDs, so it is cached by that key. The base-ref fetch is capped at 10 seconds. Conflict listing uses merge-tree; Git before 2.38 cannot give a reliable list and is treated as degraded.
- Title and body edits: a title-only path and a title-plus-body path. Base branch is not edited on this path (low confidence that no other path exists).
- Azure DevOps and Gitea show mergeable state only. Bitbucket has no merge action.
- Users can always fall back to "Open review page in browser" (SCM-07).

**Interface and data**
- Clients ask: `gh:mergePR`, `gh:updatePRState`, `gh:updatePRTitle`, `gh:markPRReadyForReview`, `gitlab:mergeMR`, `gitlab:closeMR`, `gitlab:reopenMR`, `gitlab:updateMR`.
- Persists: nothing; conflict summaries are cached in memory with bounded maps.
- Depends on: SCM-07.

### SCM-16 GitHub auto-merge and merge queue

**What the user does and what happens**
- In the PR view the merge button offers "Enable auto-merge". GitHub merges once requirements such as checks and reviews pass. The method follows the repository default.
- "Disable auto-merge" cancels a pending request from the same control.
- When the base uses a merge queue, the control reads "Merge when ready" and queues the PR.

**Rules and edge cases**
- Hidden for PRs GitHub reports as draft, closed, conflicting or unstable, and for repositories that do not allow auto-merge.
- GitHub rejects auto-merge on a PR already in a clean state. The app shows an actionable message instead of the raw error.
- The mutation only creates the request, so branch rules gate the actual merge. The variant that merges immediately is not used.

**Interface and data**
- Clients ask: `gh:setPRAutoMerge`.
- Persists: nothing locally.
- Depends on: SCM-15.

### SCM-17 AI-drafted PR title and description

**What the user does and what happens**
- In the create dialog, "Generate pull request details with AI" drafts the title, description and draft state from the branch diff, commits and current title and body. The selected base is kept.
- Generated copy aims for short problem and solution sections. With a linked GitHub issue, it suggests "Fixes" or "Refs".
- Empty results are rejected with a clear error.

**Rules and edge cases**
- Runs through a Source Control "action recipe" that picks the agent, the CLI arguments and the prompt template. Recipes can be global or per repository, and repository overrides are flagged in settings.
- Template variables include base branch, current title and body, commit summary, changed files and patch. The patch is truncated for the prompt, and the model's JSON response has structure limits.

**Interface and data**
- Clients ask: filesystem git pull-request generation handlers (names only).
- Persists: action recipes in settings.
- Depends on: SCM-09, and the agent runtime (another domain).

### SCM-18 GitHub and GitLab issue and MR browse

**What the user does and what happens**
- The task drawer lists GitHub issues and PRs for the chosen repository, or across all repositories. For GitLab it lists issues and MRs for the project.
- Filters: open, closed, all; assigned to me; author; labels; text search. For GitLab the only assignee filter exposed is "assigned to me".
- Pages and totals: GitHub uses search pages and approximate counts. GitLab uses 1-based pages with header fallbacks.

**Rules and edge cases**
- Issue-side and PR-side errors are reported separately. A failure on one side is never shown as an empty list, and an availability failure on both sides is shown as an outage rather than no data.
- Across repositories, one repository's failure does not zero the others. The result counts the failed repositories.
- An unresolved source gives an empty list. The app never runs an unscoped search, which would return other public repositories' issues.
- PR listing uses GitHub's rich PR list sorted by creation, because the search API omits PR fields the Tasks columns need.
- GitHub "closed" excludes merged PRs, so closed means closed without merge.
- GitLab issues have only open and closed states. The MR list uses the same preference resolver as issues, so fork workflows share plumbing.
- When GitLab omits total headers, totals are approximate. Proxies can strip the headers, so one extra page is probed.
- GitLab to-do list (`gitlab:todos`): read-only IPC, purpose low confidence.

**Interface and data**
- Clients ask: `gh:listWorkItems`, `gh:listIssues`, `gh:countWorkItems`, `gh:workItem`, `gh:workItemByOwnerRepo`, `gitlab:listWorkItems`, `gitlab:listIssues`, `gitlab:listMRs`, `gitlab:workItemByPath`, `gitlab:todos`.
- Persists: the per-repository issue source preference; search results are in memory (count cache 120 seconds).
- Depends on: SCM-01, SCM-03, SCM-08.

### SCM-19 GitHub and GitLab issue details, edits and comments

**What the user does and what happens**
- The issue detail dialog shows body, comments, labels, assignees and, on GitHub, issue type. The GitHub Activity section interleaves comments with timeline events: assignments, mentions, cross-references, state changes and project column moves.
- Edit title, body, state, labels, assignees and (GitHub) issue type. Create an issue.
- Comments: add, edit and delete on GitHub. Add on GitLab notes.
- Copy an issue link from the linked-issue menu without opening the drawer.

**Rules and edge cases**
- GitHub assignable users are paginated through all pages.
- GitHub body size limits are handled on create (exact behaviour not traced).
- GitLab member lookup handles inherited members (low detail).
- The Issue.parent capability is probed per owner, item type and host (SCM-27).

**Interface and data**
- Clients ask: `gh:createIssue`, `gh:updateIssue`, `gh:addIssueComment`, `gh:workItemDetails`, `gh:listLabels`, `gh:listAssignableUsers`, `gitlab:createIssue`, `gitlab:updateIssue`, `gitlab:addIssueComment`, `gitlab:listLabels`, `gitlab:listAssignableUsers`.
- Persists: nothing locally.
- Depends on: SCM-18.

### SCM-20 Jira issue browse, detail and edit

**What the user does and what happens**
- The task drawer shows Jira issues once a site is connected. Search runs by text or by a JQL-style query, with cancellable requests. The user can pick a site or "All sites".
- A detail side drawer shows the description, comments, metadata and attachments.
- Edit status through the transitions available for the issue, plus priority, assignee and custom fields. Add a comment from the composer.
- Create an issue from a dialog built from the project's create-field metadata (issue types, priorities, allowed values). Title and description survive an accidental dismissal until restart.

**Rules and edge cases**
- Descriptions are converted from the provider's document format to Markdown. Media nodes are resolved against attachments. Images are downloaded and inlined with per-image and total caps. A placeholder replaces media that cannot be downloaded, so one bad attachment does not blank the description.
- Media IDs are not attachment IDs; pairing uses attachment IDs and sizes, and each attachment is used once.
- All Jira requests go through a shared queue. Queueing for an abandoned search is avoided so it does not keep occupying the shared pool.
- Atlassian's XSRF filter rejects POST and PUT calls that carry a browser user agent, so requests are sent in a form that avoids it.
- Transitions: an issue lists its available transitions. Board and status-order detail is low confidence.
- A 403 is a permission gap, not a disconnect (SCM-05).

**Interface and data**
- Clients ask: `jira:searchIssues`, `jira:listIssues`, `jira:getIssue`, `jira:issueComments`, `jira:addIssueComment`, `jira:updateIssue`, `jira:listTransitions`, `jira:createIssue`, `jira:listCreateFields`, `jira:listIssueTypes`, `jira:listPriorities`, `jira:listProjects`, `jira:searchUsers`, `jira:listAssignableUsers`, `jira:lookupIssueSummary`, `jira:cancelSearchIssues`.
- Persists: attachment image cache in memory, with eviction and clearing on disconnect.
- Depends on: SCM-05.

### SCM-21 Linear issue browse, detail and edit

**What the user does and what happens**
- Linear issues appear in the task drawer next to GitHub and Jira. Layouts (list or board), group-by, order-by, visible columns and attribute filters persist across restarts. Attribute filters are stored per workspace.
- "Has Workspace" mode lists only issues already linked to a worktree or folder workspace. Rows with a workspace open it, and a new workspace can still be started from the same issue.
- Detail view: status, assignee, priority (with Linear's own icons), labels, estimate, due date, project, description with inline images, comments, sub-issues, relations and activity.
- Create issues and projects from dialogs. Edit fields. Add comments and attach links.
- Custom views and projects can be browsed, and teams expose their states, labels and members.
- Copy the suggested branch name from the issue menu. Worktrees created from an issue use Linear's suggested branch name when one exists.

**Rules and edge cases**
- Linear caps connection pages at 50, so larger reads walk pages. Load-more is available in the drawer.
- "All workspaces" is a globally sorted list: one bounded page per workspace, merged.
- Attribute facets merge into Linear's filter before the first-N cut.
- Search returns Linear's relevance ranking, which the app does not re-sort.
- Label updates are full-replace fields. The code notes a race if another client changes labels in between.
- Relation creation has no caller-supplied ID, so a duplicate can come from a concurrent add (exact handling not read).
- The write path includes a confirmation step (`confirmLinearWrite`; behaviour not read in detail).
- Drafts of new issues or projects survive accidental dismissal within a session and are cleared after success.

**Interface and data**
- Clients ask: `linear:listIssues`, `linear:searchIssues`, `linear:getIssue`, `linear:issueComments`, `linear:addIssueComment`, `linear:createIssue`, `linear:updateIssue`, `linear:listProjects`, `linear:getProject`, `linear:createProject`, `linear:listTeams`, `linear:teamStates`, `linear:teamLabels`, `linear:teamMembers`, `linear:listCustomViews`, `linear:getCustomView`, `linear:listCustomViewIssues`, `linear:listCustomViewProjects`, `linear:listProjectIssues`.
- Persists: layout choices and attribute filters per workspace; drafts in session only.
- Depends on: SCM-06.

### SCM-22 Unified task drawer and source picker

- Source: pv96 (unmerged). The Tasks page opens a create screen for a task or a session and lists tasks waiting for the person's approval; the WorkSpaces or Bot Team switch is still gated, and the older board stays reachable.

**What the user does and what happens**
- A source picker switches between GitHub, Linear and Jira (and GitLab in GitLab repositories). Sources can be hidden in Settings, Tasks. Jira sits in the picker even before credentials are saved.
- The drawer combines the enabled sources into one list with a shared filter model.
- The last-used source is remembered per repository, so a Jira-driven repository opens on Jira.

**Rules and edge cases**
- Jira appears in the picker even before credentials are saved (docs).
- Linear layout choices and attribute filters persist per workspace (docs).

**Interface and data**
- Clients ask: the issue list RPC methods for each source (SCM-18, SCM-20, SCM-21).
- Persists: last-used source per repository; hidden-source settings.
- Depends on: SCM-18, SCM-20, SCM-21.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the Tasks page opens from the activity rail and is one list with a source filter, covering tracker items and the Bot Teams Boards (D-WS-24, D-SH-11). Proposal: first version, with team Boards as the first source; tracker sources (SCM-18 to SCM-21) stay later.

### SCM-23 Agent access to Linear (CLI and RPC)

**What the user does and what happens**
- Agents read and write Linear through `orca linear` (read: issue with comments, children, relations, activity and full context; search across workspaces; list with filters and cursors; team, state and label lookups; project list. Write: save an issue with MCP-style fields, add and remove relations, set status, assignee, priority, estimate, due date and labels, add comments, attach links, create issues).
- Output is JSON on request. `--current` resolves the issue linked to the worktree; explicit IDs are accepted.
- Writes accept a `--write-id` UUID, which the code uses to recognise repeated writes (exact semantics not read).

**Rules and edge cases**
- The linear RPC methods carry a workspace permission.
- Errors redact authorization headers and do not expose raw SDK messages.
- Only Linear has an agent-facing command surface in the reference CLI. GitHub, GitLab and Jira have none.

**Interface and data**
- Clients ask: `linear.saveIssue`, `linear.agentSearchIssues`, `linear.issueContext`, `linear.agentIssueList`, `linear.agentProjectList`, `linear.agentTeamList`, `linear.agentTeamStates`, `linear.agentTeamLabels`, `linear.agentTeamMembers`, `linear.issueAddComment`, `linear.issueRelationWrite`, `linear.issueSetState`, `linear.issueUpdateTask`, `linear.issueAttachLink`, `linear.issueCreate`, `linear.mcpListIssues`, `linear.resolveCurrentIssue`.
- Persists: nothing beyond the Linear workspace state.
- Depends on: SCM-06, SCM-21.

### SCM-24 Create a workspace from an issue or PR

**What the user does and what happens**
- Starting a workspace from a GitHub issue or PR, a GitLab issue or MR, a Linear issue or a Jira issue opens the interactive create composer. It is not a silent background create, so the issue command template, SSH targets and folder workspaces all work.
- The composer pre-fills the task name and links the item. The branch uses Linear's suggested branch name when available, otherwise a slug of identifier and title.
- The user can paste an item URL into the name field. A pasted Jira URL is matched against connected sites by origin. If no site matches, the app says the site is not connected. If several match, the user chooses.
- A PR start checks out the PR head branch. Fork PRs are disabled in the Start-from picker, because the head owner differs from the selected repository.

**Rules and edge cases**
- Fork heads: the PR head ref is fetched into a local branch named after the contributor's branch, and the resolver can fall back to the pull-head ref when fork metadata is missing or the fork is deleted. Entry paths for this resolver are low confidence.
- A missing fork metadata field can make a fork PR look like a same-repository PR. The resolver handles that case.
- A pasted link lookup is restricted to the pasted host, so a current-directory remote cannot redirect it.
- Folder workspaces can carry a linked item. Whether PR starts apply to folder workspaces is low confidence.
- SSH hosts are supported by the composer (docs).

**Interface and data**
- Clients ask: `workspace` creation with a source selection (`github-pr`, `github-issue`, `gitlab-mr`, `gitlab-issue`, `branch`, `linear`, `jira`), and the item lookups listed under SCM-18, SCM-20 and SCM-21.
- Persists: the linked item on the new workspace (provider, number or key, title, Linear workspace and branch identifiers).
- Depends on: SCM-03, SCM-18, SCM-20, SCM-21, SCM-26 (optional).

### SCM-25 Link and relink a workspace to a task

**What the user does and what happens**
- In Edit Worktree Details, the Issue field takes a provider chip (GitHub, Linear, Jira) or a pasted URL. Saving a new link replaces the previous one.
- The worktree card shows the key and summary, with "View on Jira" or the provider's own link. The sidebar's linked item menu copies the link.
- Relinking does not change the branch and does not recreate the worktree.

**Rules and edge cases**
- GitHub and Linear share one link field. A workspace has at most one link at a time.
- Only the GitHub issue number feeds the `{linkedIssue}` placeholder in PR and commit templates. Pure Linear, GitLab or Jira links leave it empty.

**Interface and data**
- Clients ask: the workspace update call that carries the linked item (names only).
- Persists: the link on the workspace.
- Depends on: SCM-18, SCM-20, SCM-21.

### SCM-26 Issue command template

**What the user does and what happens**
- A repository can define an issue command template in its shared project config (`issueCommand`). The create composer uses it for linked GitHub and GitLab items. Templates support `{{artifact_url}}` (the item's URL) and the legacy `{{issue}}` (its number).
- Each user can override the template locally in a git-ignored file under the repository's `.orca` folder. Non-empty override content wins. Clearing it restores the shared template.

**Rules and edge cases**
- Shared YAML commands go through Orca's approval flow, and saved trust can be reused. Local overrides are treated as user-authored and do not trigger the shared-command prompt.
- The composer decides whether to use the template. With no template, it can still supply its built-in agent prompt.
- Over SSH, the override file is written on the execution host through the relay, and ignore rules are checked there.

**Interface and data**
- Clients ask: `repo` issue-command read and write (names only).
- Persists: the local override file and the shared project config field.
- Depends on: SCM-24.

### SCM-27 GitHub Projects board (Tasks)

**What the user does and what happens**
- The Tasks entry lists GitHub Projects (v2) cards across repositories and owners. Cards can be filtered by source repository and show draft PR status.
- The user can create a worktree from any card (SCM-24), update item fields (single select and others) or clear them, and edit the underlying issue or PR (title, labels, assignees, issue type).
- Project discovery: the user's own projects plus organisation projects, with failures reported in a banner rather than aborting discovery.

**Rules and edge cases**
- Discovery is bounded to keep quota use low: an organisation cap, with no per-organisation continuation loop. Users past the cap paste a project URL instead.
- Capability probes: whether an owner supports issue parent links is probed per owner, item type and host. A gap for one owner does not affect others.
- GitHub Enterprise owners form a separate namespace and quota.
- Errors are classified; raw stderr is not shown to the user.
- Caches are bounded per owner, with in-flight requests shared.

**Interface and data**
- Clients ask: `gh:listAccessibleProjects`, `gh:resolveProjectRef`, `gh:listProjectViews`, `gh:getProjectViewTable`, `gh:updateProjectItemField`, `gh:clearProjectItemField`, `gh:updateIssueBySlug`, `gh:updatePullRequestBySlug`, and the comment and label calls by slug.
- Persists: nothing; caches are in memory.
- Depends on: SCM-01, SCM-03, SCM-24.

### SCM-28 Self-hosted instance support

**What the user does and what happens**
- GitHub Enterprise Server: works when gh has the host logged in. PR and issue calls are scoped to that host. Pasted Enterprise project URLs are accepted.
- Self-managed GitLab: the host is discovered through `glab auth status --hostname` probes on each execution host. Ports are kept. Mapping stays per host.
- Gitea and Forgejo: set an API base URL. Hosts may sit under a URL subpath.
- Azure DevOps Server: identified by the `_git` path convention, with an optional API base URL.
- Jira Server or Data Center: base URL including any path; personal access token or username and password.
- Bitbucket: Cloud only. An API base override exists, but no Data Center support was found.

**Rules and edge cases**
- `--hostname` is accepted only by api and auth subcommands, not by MR or issue subcommands, so the host is passed another way for those.
- Forgejo can take about 5 seconds per pull request page on large instances, so caches are long.
- Each GitHub Enterprise host and each GitLab host has its own rate-limit scope and cache bounds.
- Azure DevOps Server rejects versioned requests without the preview suffix. The app retries with preview and remembers the origin.

**Interface and data**
- Clients ask: the provider-specific auth and status calls (SCM-01, SCM-04, SCM-05).
- Persists: per-host probe caches in memory; the API base URL settings for Gitea and Azure (environment).
- Depends on: SCM-01, SCM-03, SCM-04, SCM-05.

### SCM-29 Remote and WSL execution of provider calls

- Source: pv96 (unmerged). GitHub logins on an SSH host are now listed, switched and removed through that host's own relay (SCM-31), so the host's gh is the one changed; provider calls still run as described here.

**What the user does and what happens**
- Repositories run on execution hosts: native, each WSL distribution, or an SSH relay connection. Provider status and actions follow the repository's host.
- GitHub runs gh on the desktop (native host) even for SSH repositories. The identity comes from the origin read through the relay. There is no current-directory fallback.
- GitLab runs glab on the repository's host through the SSH relay, one remote process per call.
- Bitbucket, Azure DevOps, Gitea, Jira and Linear calls are HTTP requests from the desktop process. Their credentials stay on the desktop.

**Rules and edge cases**
- The two providers already differ. Which host runs provider calls for remote repositories is a decision for the owner (see Open decisions).
- A reconnect can replace the execution host under the same connection ID. A provider generation number fences stale answers.
- The SSH relay's read-only git channel rejects fetch, so PR head fetches for SSH repositories use a separate path. Conflict summaries use local git and are skipped for SSH repositories.
- WSL: gh inside a distribution has its own binary, login and quota scope. WSL repositories use a UNC current directory. Host gh is used as a fallback only for commands with an explicit repository or API placeholder. Tokens reach the WSL child through WSLENV.
- Windows: the WSL distribution and the native Windows environment each need their own gh or glab installation and login.

**Interface and data**
- Clients ask: the provider calls above, which take the repository's execution host implicitly.
- Persists: per-connection auth caches in memory.
- Depends on: SCM-01, SCM-03.

### SCM-30 Star Orca on GitHub (upstream promotion)

**What the user does and what happens**
- Several prompts (a landing button, a settings section, and a usage threshold) offer to star the upstream Orca repository on GitHub using the user's gh login. The app checks whether the repository is already starred.
- Starring records an analytics event with the source and a cohort.

**Rules and edge cases**
- The action is promotion of the upstream project and sends analytics. It is not part of the user's work.

**Interface and data**
- Clients ask: `gh:checkOrcaStarred`, `gh:starOrca` (with a source tag).
- Persists: nothing locally; analytics event.
- Depends on: SCM-01.

### SCM-31 Host-wide GitHub logins: list, switch active, remove

- Source: pv96 (unmerged). New: the GitHub logins that gh knows on each host can be listed, one can be made active, and one can be removed, from the app, on an SSH host through its relay, and from the CLI.

**What the user does and what happens**
- The Marketplace's GitHub page lists the logins gh knows on each host. The user can make one the active login or remove one.
- On an SSH host the same actions run through that host's own relay, so that host's gh is the one changed.
- The CLI has `orca marketplace github accounts list`, `switch` and `rm`.

**Rules and edge cases**
- Switching runs `gh auth switch` and removing runs `gh auth logout`, both for one host and one login, and both need gh 2.40 or later. An older gh is reported as unsupported; gh's raw output does not cross IPC.
- Removing a login does not revoke its token.
- A remote host that needs an update answers that it needs one, instead of a partial list.
- The token cache is cleared after a switch or a removal.

**Interface and data**
- Clients ask: `gh:listHostBindableAccounts` (existing), and the new `gh:switchActiveAccount` and `gh:removeAccount` (IPC); `github.switchActiveAccount` and `github.removeAccount` (RPC, new in pv96).
- Persists: nothing new; gh holds the logins.
- Depends on: SCM-01, SCM-02, SCM-29.

**Proposal (new in pv96).** Later. Size M. Reason: whether the first version needs it depends on the owner's open choice between one login and per-repository binding (SCM-02).

### SCM-32 Feedback reports published as GitHub issues

- Source: pv96 (unmerged). New: a feedback report becomes an issue in the MassOrca project's own GitHub repository, created with gh's active login.

**What the user does and what happens**
- The user writes a report in the sidebar's feedback dialog and submits it. The report becomes an issue in the MassOrca repository, created with the login gh has active on this computer.
- The earlier feedback endpoint is no longer used for this path, and the form's image attachments were removed in this pass.

**Rules and edge cases**
- The renderer supplies the report body only; the target repository is fixed in the main process, because the renderer is not trusted.
- The title is the first non-empty line of the report after a client environment footer is stripped; when it is longer than 80 characters it is cut and prefixed with "Feedback: ".
- The target is pinned, so a fork's upstream cannot be picked.

**Interface and data**
- Clients ask: `feedback:submit` (IPC).
- Persists: nothing locally.
- Depends on: SCM-01 (gh login).

**Proposal (new in pv96).** Drop (owner to confirm). Reason: the reports go to the current project's own tracker; the new product needs its own feedback channel, decided later.

## Open decisions for the owner

- Which host runs provider calls for SSH and remote repositories: the core's host (where the repository lives) or the desktop. The current app does both (gh on the desktop, glab on the remote).
- Whether the first version needs one GitHub login or per-repository account binding (SCM-02).
- Whether to keep plaintext token fallback for Jira and Linear when the OS secret store is unavailable. The current app allows it with a warning.
- Whether a feedback channel is needed for the new product (SCM-32 is proposed as dropped).
