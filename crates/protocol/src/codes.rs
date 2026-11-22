//! Stable error codes. Clients branch on these strings; `message` is for humans only.

pub const PROTOCOL_UNSUPPORTED_VERSION: &str = "protocol.unsupported_version";
pub const PROTOCOL_HANDSHAKE: &str = "protocol.handshake";
pub const METHOD_NOT_FOUND: &str = "method.not_found";
pub const CAPABILITY_NOT_NEGOTIATED: &str = "method.capability_not_negotiated";
pub const PERMISSION_DENIED: &str = "permission.denied";
pub const REQUEST_INVALID_PARAMS: &str = "request.invalid_params";
pub const INTERNAL: &str = "internal";
pub const SERVICE_SHUTTING_DOWN: &str = "service.shutting_down";
pub const STREAM_NOT_FOUND: &str = "stream.not_found";
/// A subscriber fell too far behind; its stream ends with this error rather than buffering without limit.
pub const STREAM_OVERFLOW: &str = "stream.overflow";
/// One stream item is larger than a frame; the stream ends with this error.
pub const STREAM_ITEM_TOO_LARGE: &str = "stream.item_too_large";
/// The method sends stream-bytes frames and the client did not negotiate `binary-frames/1`.
pub const STREAM_BINARY_FRAMES_REQUIRED: &str = "stream.binary_frames_required";
/// A reply is larger than a frame; only the request that asked for it fails.
pub const RESPONSE_TOO_LARGE: &str = "response.too_large";
pub const PATH_NOT_UTF8: &str = "path.not_utf8";
pub const PATH_NOT_FOUND: &str = "path.not_found";
pub const PATH_NOT_DIRECTORY: &str = "path.not_directory";
pub const PROJECT_NOT_FOUND: &str = "project.not_found";
pub const PROJECT_EXISTS: &str = "project.exists";
pub const PROJECT_NOT_GIT: &str = "project.not_git";
pub const PROJECT_HAS_WORKSPACES: &str = "project.has_workspaces";
pub const WORKSPACE_NOT_FOUND: &str = "workspace.not_found";
pub const WORKSPACE_NAME_TAKEN: &str = "workspace.name_taken";
pub const WORKSPACE_NAME_INVALID: &str = "workspace.name_invalid";
pub const WORKSPACE_PATH_TAKEN: &str = "workspace.path_taken";
pub const WORKSPACE_DIRTY: &str = "workspace.dirty";
pub const WORKSPACE_LOCKED: &str = "workspace.locked";
pub const WORKSPACE_IGNORED_FILES: &str = "workspace.ignored_files";
/// A directory git does not recognise as its worktree; removal needs `forget_orphan`.
pub const WORKSPACE_ORPHAN: &str = "workspace.orphan";
pub const WORKSPACE_MAIN_WORKTREE: &str = "workspace.main_worktree";
pub const BRANCH_EXISTS: &str = "branch.exists";
pub const BRANCH_NOT_FOUND: &str = "branch.not_found";
/// A branch name git does not accept.
pub const BRANCH_NAME_INVALID: &str = "branch.name_invalid";
/// A branch is not merged into its upstream or HEAD; deleting it needs `force`.
pub const BRANCH_UNMERGED: &str = "branch.unmerged";
/// A branch is checked out in a worktree; it cannot be deleted, or switched to from another worktree.
pub const BRANCH_CHECKED_OUT: &str = "branch.checked_out";
/// A workspace was created on the branch; removing that workspace is the path that deletes it.
pub const BRANCH_IN_USE: &str = "branch.in_use";
pub const BASE_NOT_FOUND: &str = "base.not_found";
pub const GIT_UNAVAILABLE: &str = "git.unavailable";
pub const GIT_FAILED: &str = "git.failed";
pub const SELECTOR_AMBIGUOUS: &str = "selector.ambiguous";
/// A git method named a folder workspace, or a path git does not treat as a repository.
pub const GIT_NOT_A_REPOSITORY: &str = "git.not_a_repository";
/// A request path is absolute, has a `..` segment or a NUL, so it could leave the workspace.
pub const GIT_PATH_OUTSIDE_WORKTREE: &str = "git.path_outside_worktree";
/// A revision does not resolve to a commit.
pub const GIT_REVISION_NOT_FOUND: &str = "git.revision_not_found";
/// A discard named untracked files without `delete_untracked`; `data.paths` lists them.
pub const GIT_DISCARD_UNTRACKED: &str = "git.discard_untracked";
/// A discard named files whose only changes are staged, without `include_staged`; `data.paths` lists them.
pub const GIT_DISCARD_STAGED: &str = "git.discard_staged";
/// A discard named unmerged files; resolve or abort the operation first. `data.paths` lists them.
pub const GIT_PATH_CONFLICTED: &str = "git.path_conflicted";
/// A discard named a directory; it acts on the files status lists only. `data.paths` lists them.
pub const GIT_PATH_IS_DIRECTORY: &str = "git.path_is_directory";
/// A request named both `paths` and `all`, or neither.
pub const GIT_PATHS_REQUIRED: &str = "git.paths_required";
/// Another process holds the index lock (`index.lock`); retry once it finishes.
pub const GIT_INDEX_LOCKED: &str = "git.index_locked";
/// A commit with nothing staged, or with an empty message.
pub const GIT_NOTHING_TO_COMMIT: &str = "git.nothing_to_commit";
pub const GIT_COMMIT_MESSAGE_EMPTY: &str = "git.commit_message_empty";
/// An amend of a commit a remote-tracking branch contains, without `allow_published`.
pub const GIT_COMMIT_PUBLISHED: &str = "git.commit_published";
/// An amend before the first commit.
pub const GIT_NO_COMMITS: &str = "git.no_commits";
/// Git has no user name or email for the commit; set `user.name` and `user.email`.
pub const GIT_IDENTITY_UNKNOWN: &str = "git.identity_unknown";
/// A checkout would overwrite local changes or untracked files; `data.paths` lists them.
pub const GIT_CHECKOUT_WOULD_OVERWRITE: &str = "git.checkout_would_overwrite";
/// A remote asked for credentials the host's git setup does not supply; authenticate in a terminal, then retry.
pub const GIT_AUTH_REQUIRED: &str = "git.auth_required";
/// SSH does not know the remote host's key; accept it once in a terminal, then retry.
pub const GIT_HOST_KEY_UNKNOWN: &str = "git.host_key_unknown";
/// The remote host could not be reached: name resolution, connection or timeout.
pub const GIT_REMOTE_UNREACHABLE: &str = "git.remote_unreachable";
/// The remote answered that the repository does not exist, or the remote is not configured.
pub const GIT_REMOTE_NOT_FOUND: &str = "git.remote_not_found";
/// The branch has no upstream to pull from.
pub const GIT_NO_UPSTREAM: &str = "git.no_upstream";
/// HEAD is not on a branch.
pub const GIT_DETACHED_HEAD: &str = "git.detached_head";
/// No merge, rebase, cherry-pick or revert is in progress.
pub const GIT_NO_OPERATION: &str = "git.no_operation";
/// A merge or rebase would overwrite local changes; commit, stash elsewhere or discard them first.
pub const GIT_LOCAL_CHANGES: &str = "git.local_changes";
/// The repository's git directory cannot be watched; the stream ends with this code.
pub const GIT_WATCH_UNAVAILABLE: &str = "git.watch_unavailable";
/// CLI only: a fast-forward-only pull found local and remote commits on both sides; nothing changed.
pub const GIT_DIVERGED: &str = "git.diverged";
/// CLI only: a pull's merge or rebase stopped on conflicts.
pub const GIT_CONFLICTS: &str = "git.conflicts";
/// CLI only: the remote refused the push.
pub const GIT_PUSH_REJECTED: &str = "git.push_rejected";
pub const CONFIG_UNKNOWN_KEY: &str = "config.unknown_key";
pub const CONFIG_INVALID_VALUE: &str = "config.invalid_value";
/// The store was written by a newer build; it is left untouched (decision 0005).
pub const STORE_NEWER_SCHEMA: &str = "store.newer_schema";
/// CLI only: the service refused a profile directory that holds entries another user owns; `data.paths` lists up to ten.
pub const PROFILE_FOREIGN_ENTRIES: &str = "profile.foreign_entries";
/// CLI only: other users could change the profile directory or entries in it; `data.paths` lists up to ten.
pub const PROFILE_WRITABLE_BY_OTHERS: &str = "profile.writable_by_others";
/// CLI only: a folder from the home down to the profile is a link, another user's, or changeable by others; `data.path`.
pub const PROFILE_UNSAFE_LOCATION: &str = "profile.unsafe_location";
/// Other users could read a profile folder that holds secrets, such as a device key; `data.paths` lists it.
pub const PROFILE_READABLE_BY_OTHERS: &str = "profile.readable_by_others";
/// A network connection without a client certificate or `hello.auth` (decision 0041).
pub const AUTH_REQUIRED: &str = "auth.required";
/// The client certificate belongs to no paired device.
pub const AUTH_UNKNOWN_DEVICE: &str = "auth.unknown_device";
pub const AUTH_DEVICE_REVOKED: &str = "auth.device_revoked";
/// A wrong, unknown or already used pairing secret.
pub const PAIRING_INVALID: &str = "pairing.invalid";
pub const PAIRING_EXPIRED: &str = "pairing.expired";
/// Too many failed pairing attempts from this address, or overall; retry after the window.
pub const PAIRING_RATE_LIMITED: &str = "pairing.rate_limited";
/// The service has no network listener, so an offer would name no address.
pub const PAIRING_UNAVAILABLE: &str = "pairing.unavailable";
pub const PAIRING_TOO_MANY_OFFERS: &str = "pairing.too_many_offers";
pub const DEVICE_NOT_FOUND: &str = "device.not_found";
pub const TERMINAL_NOT_FOUND: &str = "terminal.not_found";
/// Input or resize sent to a terminal whose process has already ended.
pub const TERMINAL_EXITED: &str = "terminal.exited";
/// The OS error text travels in the error `message`.
pub const TERMINAL_SPAWN_FAILED: &str = "terminal.spawn_failed";
pub const TERMINAL_INPUT_TOO_LARGE: &str = "terminal.input_too_large";
pub const TERMINAL_INVALID_SIZE: &str = "terminal.invalid_size";
pub const TERMINAL_LIMIT_REACHED: &str = "terminal.limit_reached";
/// The vault key will not open; the ciphertext is kept (decision 0071).
pub const SECRET_VAULT_LOCKED: &str = "secret.vault_locked";
pub const SECRET_NOT_FOUND: &str = "secret.not_found";
/// The name is already owned or linked in this scope.
pub const SECRET_NAME_IN_USE: &str = "secret.name_in_use";
/// The name in this scope is a link; change it at its owning scope, named in the message.
pub const SECRET_LINKED: &str = "secret.linked";
/// Other scopes link to this secret; they are named in the message. `include_links` removes them too.
pub const SECRET_HAS_LINKS: &str = "secret.has_links";
pub const SECRET_INVALID_NAME: &str = "secret.invalid_name";
pub const SECRET_INVALID_SCOPE: &str = "secret.invalid_scope";
pub const SECRET_TOO_LARGE: &str = "secret.too_large";
pub const SECRET_INVALID_REASON: &str = "secret.invalid_reason";
pub const SECRET_INVALID_SESSION: &str = "secret.invalid_session";
/// A secret, named in the data, does not open with the vault key; nothing was marked used.
pub const SECRET_UNREADABLE: &str = "secret.unreadable";
/// `secret.reset_key` while the vault still holds secrets; the count is in the data.
pub const SECRET_VAULT_NOT_EMPTY: &str = "secret.vault_not_empty";
/// Too many pending or recent requests from one requester; see decision 0071 for the limits.
pub const SECRET_REQUEST_LIMIT: &str = "secret.request_limit";
/// Names stored before case clashes were refused would reach one process together; the data names each one.
pub const SECRET_NAME_CLASH: &str = "secret.name_clash";
/// The name loads code, redirects lookups or swaps an endpoint (decision 0071); the data names it.
pub const SECRET_RESERVED_NAME: &str = "secret.reserved_name";
pub const SECRET_REQUEST_NOT_FOUND: &str = "secret.request_not_found";
/// The request is no longer pending.
pub const SECRET_REQUEST_RESOLVED: &str = "secret.request_resolved";

pub const AGENT_NOT_FOUND: &str = "agent.not_found";
pub const AGENT_KIND_NOT_FOUND: &str = "agent.kind_not_found";
/// The kind is declared but none of its program candidates resolves, or its declaration has problems.
pub const AGENT_NOT_INSTALLED: &str = "agent.not_installed";
/// The workspace already holds a live writing session; `data.session` names it.
pub const AGENT_WORKSPACE_BUSY: &str = "agent.workspace_busy";
pub const AGENT_NOT_LIVE: &str = "agent.not_live";
pub const AGENT_BUSY: &str = "agent.busy";
/// A prompt over the size limit or with a NUL.
pub const AGENT_PROMPT_INVALID: &str = "agent.prompt_invalid";
pub const AGENT_RESUME_UNAVAILABLE: &str = "agent.resume_unavailable";
pub const AGENT_HOOK_REFUSED: &str = "agent.hook_refused";
pub const AGENT_DELEGATE_NEEDS_GIT: &str = "agent.delegate_needs_git";
pub const AGENT_DEPTH_EXCEEDED: &str = "agent.depth_exceeded";
pub const AGENT_PARENT_BUSY: &str = "agent.parent_busy";
pub const AGENT_PARENT_DIRTY: &str = "agent.parent_dirty";
pub const AGENT_MERGE_CONFLICT: &str = "agent.merge_conflict";
pub const AGENT_ALREADY_BROUGHT_BACK: &str = "agent.already_brought_back";
pub const AGENT_CREDENTIALS_UNSUPPORTED: &str = "agent.credentials_unsupported";
pub const AGENT_CREDENTIALS_EXIST: &str = "agent.credentials_exist";
pub const AGENT_CREDENTIALS_INVALID: &str = "agent.credentials_invalid";
pub const AGENT_ACCOUNT_NOT_FOUND: &str = "agent.account_not_found";
/// The run's vault secrets could not be given to it; the message names the cause and variable names, never a value.
pub const AGENT_SECRETS_UNAVAILABLE: &str = "agent.secrets_unavailable";

/// A team, bot or team project is named by a selector that matches none, or none of its own team.
pub const TEAM_NOT_FOUND: &str = "team.not_found";
/// Another team has this name.
pub const TEAM_NAME_IN_USE: &str = "team.name_in_use";
pub const TEAM_PROJECT_NOT_FOUND: &str = "team.project_not_found";
pub const TEAM_PROJECT_NAME_IN_USE: &str = "team.project_name_in_use";
pub const BOT_NOT_FOUND: &str = "bot.not_found";
/// Another bot of the team has this name.
pub const BOT_NAME_IN_USE: &str = "bot.name_in_use";
/// The team already has a lead bot; clear its lead first.
pub const BOT_LEAD_TAKEN: &str = "bot.lead_taken";
/// The bot owns open tasks; it can be removed only with `unassign`.
pub const BOT_HAS_TASKS: &str = "bot.has_tasks";
pub const TASK_NOT_FOUND: &str = "task.not_found";
/// The task's agent session still runs; stop it first.
pub const TASK_RUNNING: &str = "task.running";

/// Reserved namespace for extension error codes; core never defines codes under it.
pub const EXTENSION_PREFIX: &str = "extension.";
