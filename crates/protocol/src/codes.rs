//! Stable error codes. Clients branch on these strings; `message` is for humans only.

pub const PROTOCOL_UNSUPPORTED_VERSION: &str = "protocol.unsupported_version";
pub const PROTOCOL_HANDSHAKE: &str = "protocol.handshake";
pub const METHOD_NOT_FOUND: &str = "method.not_found";
pub const CAPABILITY_NOT_NEGOTIATED: &str = "method.capability_not_negotiated";
pub const PERMISSION_DENIED: &str = "permission.denied";
pub const REQUEST_INVALID_PARAMS: &str = "request.invalid_params";
pub const INTERNAL: &str = "internal";
pub const SERVICE_SHUTTING_DOWN: &str = "service.shutting_down";
/// CLI only: the service predates the command table, so it does not offer `commands/1`.
pub const SERVICE_CAPABILITY_MISSING: &str = "service.capability_missing";
/// `massorca open` found no desktop app beside the CLI, nor at `MASSORCA_DESKTOP_EXECUTABLE`.
pub const DESKTOP_NOT_FOUND: &str = "desktop.not_found";
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
/// The folder is already a git repository, or inside one, or already a git project (decision 0115).
pub const PROJECT_ALREADY_GIT: &str = "project.already_git";
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
/// Delegated children run in workspaces of their own under this one; removal needs `with_children` (decision 0110).
/// `data.children` names them.
pub const WORKSPACE_HAS_CHILDREN: &str = "workspace.has_children";
/// Skill placement (SKL-02): a folder for the skill already holds one MassOrca did not place.
pub const SKILL_NAME_TAKEN: &str = "skill.name_taken";
/// Skill placement (SKL-02): the skill changed since MassOrca placed it; remove the placed copies first.
pub const SKILL_CONTENT_CHANGED: &str = "skill.content_changed";
/// Skill placement (SKL-02): `skills.unplace` names a skill MassOrca did not place.
pub const SKILL_NOT_PLACED: &str = "skill.not_placed";
/// Skill placement (SKL-02): no skill with that id is found on this host.
pub const SKILL_NOT_FOUND: &str = "skill.not_found";
/// Skill placement (SKL-02): the skill cannot be copied (a link inside it, a bundled or plugin skill, or too large).
pub const SKILL_NOT_PLACEABLE: &str = "skill.not_placeable";
/// Skill placement (SKL-02): the placement record on this host cannot be read, so nothing is removed.
pub const SKILL_RECORD_INVALID: &str = "skill.record_invalid";
/// A workspace has a live terminal or agent run, or is being removed: a removal whose runs did not stop stays
/// `removing`, and a project with such a workspace cannot be forgotten.
pub const WORKSPACE_BUSY: &str = "workspace.busy";
/// `workspace.rename` refused a name a person typed; only a name the service chose is renamed (decision 0111).
pub const WORKSPACE_NAME_NOT_GENERATED: &str = "workspace.name_not_generated";
/// The worktree is no longer on the branch the workspace records, or the one the caller expected.
pub const WORKSPACE_BRANCH_CHANGED: &str = "workspace.branch_changed";
/// The workspace's branch has an upstream or a remote copy, so renaming it would split it from what others see.
pub const WORKSPACE_BRANCH_PUBLISHED: &str = "workspace.branch_published";
/// A workspace's archive script failed or timed out; nothing was stopped or removed. `allow_failed_archive` waives it.
pub const WORKSPACE_ARCHIVE_FAILED: &str = "workspace.archive_failed";
/// The project's `orca.yaml` defines no script of the kind asked for.
pub const SCRIPT_NOT_FOUND: &str = "script.not_found";
/// The script's content no longer has the hash the caller asked to trust.
pub const SCRIPT_CHANGED: &str = "script.changed";
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
/// The host's git is older than the 2.25 baseline, so the command was refused (ADR 0014).
pub const GIT_TOO_OLD: &str = "git.too_old";
pub const GIT_FAILED: &str = "git.failed";
pub const SELECTOR_AMBIGUOUS: &str = "selector.ambiguous";
/// A git method named a folder workspace, or a path git does not treat as a repository.
pub const GIT_NOT_A_REPOSITORY: &str = "git.not_a_repository";
/// A request path is absolute, has a `..` segment or a NUL, so it could leave the workspace.
pub const GIT_PATH_OUTSIDE_WORKTREE: &str = "git.path_outside_worktree";
/// A caller without `workspace` permission asked for a git read that could not be made to start no program the
/// repository names, such as a hooks folder that is not empty (decision 0078).
pub const GIT_UNSAFE_FOR_CALLER: &str = "git.unsafe_for_caller";
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
/// An address given for a pairing offer is not one a device could dial (a wildcard, port 0, a path, credentials).
pub const PAIRING_INVALID_ADDRESS: &str = "pairing.invalid_address";
/// `pairing.cancel` named an offer that is not pending: unknown, used or expired.
pub const PAIRING_OFFER_NOT_FOUND: &str = "pairing.offer_not_found";
pub const DEVICE_NOT_FOUND: &str = "device.not_found";
/// `device.forget` on a device that is still active; revoke it first.
pub const DEVICE_NOT_REVOKED: &str = "device.not_revoked";
/// `device.grant` on a revoked device; pair it again instead.
pub const DEVICE_REVOKED: &str = "device.revoked";
pub const TERMINAL_NOT_FOUND: &str = "terminal.not_found";
/// Input or resize sent to a terminal whose process has already ended.
pub const TERMINAL_EXITED: &str = "terminal.exited";
/// The OS error text travels in the error `message`.
pub const TERMINAL_SPAWN_FAILED: &str = "terminal.spawn_failed";
pub const TERMINAL_INPUT_TOO_LARGE: &str = "terminal.input_too_large";
pub const TERMINAL_INVALID_SIZE: &str = "terminal.invalid_size";
pub const TERMINAL_LIMIT_REACHED: &str = "terminal.limit_reached";
/// The terminal was closed while a `terminal.wait` was on it, so its exit is not known.
pub const TERMINAL_GONE: &str = "terminal.gone";
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
/// The kind's package is available or disabled, not enabled (decision 0059); the data names the kind, the package and its
/// state.
pub const AGENT_KIND_NOT_ENABLED: &str = "agent.kind_not_enabled";
/// The kind is declared but none of its program candidates resolves, or its declaration has problems.
pub const AGENT_NOT_INSTALLED: &str = "agent.not_installed";
/// The workspace already holds a live writing session; `data.session` names it.
pub const AGENT_WORKSPACE_BUSY: &str = "agent.workspace_busy";
pub const AGENT_NOT_LIVE: &str = "agent.not_live";
pub const AGENT_BUSY: &str = "agent.busy";
/// A prompt over the size limit or with a NUL.
pub const AGENT_PROMPT_INVALID: &str = "agent.prompt_invalid";
pub const AGENT_RESUME_UNAVAILABLE: &str = "agent.resume_unavailable";
/// The kind keeps a saved conversation per agent ref, and none exists for the session's ref: a resume would fail.
pub const AGENT_RESUME_NOT_FOUND: &str = "agent.resume_not_found";
/// `agent.fork` cannot fork the session: its kind declares no fork form, or its agent ref is not known (decision 0168).
pub const AGENT_FORK_UNAVAILABLE: &str = "agent.fork_unavailable";
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
/// The kind declares no one-shot form, so it cannot name a chat (decision 0167).
pub const AGENT_ONESHOT_UNSUPPORTED: &str = "agent.oneshot_unsupported";
/// A one-shot run that names a chat did not finish within its time limit (decision 0167).
pub const AGENT_ONESHOT_TIMEOUT: &str = "agent.oneshot_timeout";
/// A one-shot run that names a chat exited with a failure, or printed no usable title (decision 0167).
pub const AGENT_ONESHOT_FAILED: &str = "agent.oneshot_failed";
/// The run's vault secrets could not be given to it; the message names the cause and variable names, never a value.
pub const AGENT_SECRETS_UNAVAILABLE: &str = "agent.secrets_unavailable";
/// The kind declares no permission mode of that id, or none at all; the data names the kind, the mode and the known ids.
pub const AGENT_MODE_UNKNOWN: &str = "agent.mode_unknown";
/// The mode has no launch arguments and its live switch cannot be sent at start, because the kind reports no idle state.
pub const AGENT_MODE_UNAVAILABLE: &str = "agent.mode_unavailable";
/// The kind declares no flag for a `model` or `effort` the start asked for (decision 0094); the message names the kind.
pub const AGENT_OPTION_UNSUPPORTED: &str = "agent.option_unsupported";
/// The session's kind has no status mechanism, so it has no prompt queue (decision 0084).
pub const AGENT_QUEUE_UNAVAILABLE: &str = "agent.queue_unavailable";
/// The session's queue already holds the most prompts it may (decision 0084).
pub const AGENT_QUEUE_FULL: &str = "agent.queue_full";
/// No queued prompt has the id (decision 0084).
pub const AGENT_QUEUE_NOT_FOUND: &str = "agent.queue_not_found";
/// The request id belongs to a prompt the session already delivered, so it is not typed again (decision 0084).
pub const AGENT_QUEUE_ALREADY_DELIVERED: &str = "agent.queue_already_delivered";
/// The output of a released session could not be archived, so its terminal was kept (`agent.release`, decision 0090).
pub const AGENT_ARCHIVE_FAILED: &str = "agent.archive_failed";
/// `agent.chat.start` refused: the kind has no chat support, or its CLI version is outside the supported range. The
/// error's `data` is the kind's `KindChat` (decision 0101).
pub const AGENT_CHAT_UNSUPPORTED: &str = "agent.chat_unsupported";
/// A chat method was called on a session that does not run as a chat (decision 0101).
pub const AGENT_NOT_CHAT: &str = "agent.not_chat";
/// `agent.chat.approval.answer` names a request the session never had, or one that is no longer pending (decision 0101).
/// The error's `data` is the approval item when it exists.
pub const AGENT_APPROVAL_SETTLED: &str = "agent.approval_settled";
/// `agent.chat.approval.answer` names a `seq` the approval item no longer has: the CLI sent the request again, with new
/// input, since the person saw it (decision 0101). The error's `data` is the item as it is now.
pub const AGENT_APPROVAL_CHANGED: &str = "agent.approval_changed";
/// `agent.chat.history` names a cursor from another journal epoch; the client resubscribes (decision 0101).
pub const AGENT_CURSOR_STALE: &str = "agent.cursor_stale";

/// A team, bot or team project is named by a selector that matches none, or none of its own team.
pub const TEAM_NOT_FOUND: &str = "team.not_found";
/// Another team has this name.
pub const TEAM_NAME_IN_USE: &str = "team.name_in_use";
pub const TEAM_PROJECT_NOT_FOUND: &str = "team.project_not_found";
/// `task.start` found the Bot teams folder already added as a project that is not a folder project.
pub const TEAM_FOLDER_TAKEN: &str = "team.folder_taken";
pub const TEAM_PROJECT_NAME_IN_USE: &str = "team.project_name_in_use";
/// The team has Workspaces off, so its projects cannot be connected to code (decision 0120).
pub const TEAM_WORKSPACES_OFF: &str = "team.workspaces_off";
/// The team has no memory line with this id.
pub const TEAM_MEMORY_NOT_FOUND: &str = "team.memory_not_found";
pub const TEAM_ROUTINE_NOT_FOUND: &str = "team.routine_not_found";
/// The service stopped after a routine run was claimed and before its start recorded an outcome. It is not retried.
pub const TEAM_ROUTINE_RUN_INTERRUPTED: &str = "team.routine_run_interrupted";
/// Other calls kept changing the routine while this edit was made; read it again and retry.
pub const TEAM_ROUTINE_CHANGED: &str = "team.routine_changed";
/// `team.room_answer` names no line of the team's room, or a line that is not a choice card.
pub const TEAM_ROOM_CARD_NOT_FOUND: &str = "team.room_card_not_found";
/// `team.room_answer` on a choice card that was answered already; a card takes one answer.
pub const TEAM_ROOM_CARD_ANSWERED: &str = "team.room_card_answered";
/// `team.room_post`'s `reply_to` names no line of the team's room.
pub const TEAM_ROOM_LINE_NOT_FOUND: &str = "team.room_line_not_found";
pub const BOT_NOT_FOUND: &str = "bot.not_found";
/// Another bot of the team has this name.
pub const BOT_NAME_IN_USE: &str = "bot.name_in_use";
/// The team already has a lead bot; clear its lead first.
pub const BOT_LEAD_TAKEN: &str = "bot.lead_taken";
/// The bot owns open tasks; it can be removed only with `unassign`.
pub const BOT_HAS_TASKS: &str = "bot.has_tasks";
/// No longer sent: hosts before decision 0072's 2026-11-23 amendment refused `task.start` for a bot working on another
/// task. Kept so a client still names that answer from an older host.
pub const BOT_BUSY: &str = "bot.busy";
pub const TASK_NOT_FOUND: &str = "task.not_found";
/// The task's agent session still runs; stop it first.
pub const TASK_RUNNING: &str = "task.running";
/// `task.stop` on a task whose agent session does not run.
pub const TASK_NOT_RUNNING: &str = "task.not_running";
/// `task.start` on a task that already has a session or an outcome; reopen it first.
pub const TASK_STARTED: &str = "task.started";
/// `task.start` on a task whose last three starts failed in a row; reopen it to try again (AUTO-16).
pub const TASK_CIRCUIT_BROKEN: &str = "task.circuit_broken";
/// `task.start` on a task without a bot.
pub const TASK_NO_BOT: &str = "task.no_bot";
/// The task changed between the call reading it and writing it; read it again and retry.
pub const TASK_CHANGED: &str = "task.changed";

/// `marketplace.service.*` names a service this build does not know.
pub const SERVICE_UNKNOWN: &str = "marketplace.service_unknown";
/// The service cannot do this from here: a status-only row, GitLab without `glab`, or a GitLab disconnect.
pub const SERVICE_UNAVAILABLE: &str = "marketplace.service_unavailable";
/// The service's program refused the token. The message is a fixed sentence, never the program's output.
pub const SERVICE_REJECTED: &str = "marketplace.service_rejected";
/// The service's program could not be run or did not finish in time.
pub const SERVICE_FAILED: &str = "marketplace.service_failed";

/// Reserved namespace for extension error codes; core never defines codes under it.
pub const EXTENSION_PREFIX: &str = "extension.";

// Extension codes (decision 0062). The package-format ones come from decision 0061.
pub const EXTENSION_INVALID_MANIFEST: &str = "extension.invalid_manifest";
pub const EXTENSION_INVALID_HASH: &str = "extension.invalid_hash";
pub const EXTENSION_HASH_MISMATCH: &str = "extension.hash_mismatch";
pub const EXTENSION_INVALID_ARCHIVE: &str = "extension.invalid_archive";
pub const EXTENSION_UNSAFE_PATH: &str = "extension.unsafe_path";
pub const EXTENSION_UNSAFE_ENTRY: &str = "extension.unsafe_entry";
pub const EXTENSION_LIMIT_EXCEEDED: &str = "extension.limit_exceeded";
pub const EXTENSION_MISSING_FILE: &str = "extension.missing_file";
/// The source is not `catalogue:<id>`, `dir:<path>` or `file:<path>`.
pub const EXTENSION_INVALID_SOURCE: &str = "extension.invalid_source";
/// No catalogue entry has that id.
pub const EXTENSION_NOT_FOUND: &str = "extension.not_found";
pub const EXTENSION_NOT_INSTALLED: &str = "extension.not_installed";
pub const EXTENSION_ALREADY_INSTALLED: &str = "extension.already_installed";
/// Another package already installed the agent kind.
pub const EXTENSION_KIND_CONFLICT: &str = "extension.kind_conflict";
/// A package archive needs its SHA-256.
pub const EXTENSION_HASH_REQUIRED: &str = "extension.hash_required";
/// An install without the fingerprint of an accepted review.
pub const EXTENSION_CONSENT_REQUIRED: &str = "extension.consent_required";
/// The package or its install plan changed since the review.
pub const EXTENSION_CONSENT_MISMATCH: &str = "extension.consent_mismatch";
/// The install runs a vendor installer and `run_installer` was not set.
pub const EXTENSION_INSTALLER_CONSENT_REQUIRED: &str = "extension.installer_consent_required";
pub const EXTENSION_INSTALLER_FAILED: &str = "extension.installer_failed";
pub const EXTENSION_VERIFY_FAILED: &str = "extension.verify_failed";
pub const EXTENSION_ASSET_NOT_LISTED: &str = "extension.asset_not_listed";
pub const EXTENSION_ASSET_TOO_LARGE: &str = "extension.asset_too_large";
pub const EXTENSION_INVALID_LOCKFILE: &str = "extension.invalid_lockfile";
pub const EXTENSION_IO: &str = "extension.io";
/// An installed package's files no longer match its lockfile hash; nothing they name runs.
pub const EXTENSION_TAMPERED: &str = "extension.tampered";
/// A package from outside the built-in catalogue claims publisher `massorca` or a `massorca.*` id.
pub const EXTENSION_RESERVED_PUBLISHER: &str = "extension.reserved_publisher";
/// Another install or removal of the same package is running; try again when it has finished.
pub const EXTENSION_BUSY: &str = "extension.busy";
/// A package from outside the built-in catalogue declares a vendor command (install, verify or uninstall).
pub const EXTENSION_VENDOR_COMMAND_REFUSED: &str = "extension.vendor_command_refused";
/// A vendor installer or uninstaller needs a person: an agent run may not start one.
pub const EXTENSION_NEEDS_PERSON: &str = "extension.needs_person";
/// `extensions.update` found the installed package already in place: nothing changed.
pub const EXTENSION_UP_TO_DATE: &str = "extension.up_to_date";
/// `extensions.rollback` found no version kept for the package: nothing changed.
pub const EXTENSION_NO_PREVIOUS_VERSION: &str = "extension.no_previous_version";
/// No automation has the id or name given (`automation.*`, decision 0123).
pub const AUTOMATION_NOT_FOUND: &str = "automation.not_found";
/// An automation's cadence, time, weekday, cron expression or grace is refused, such as a cron that can never occur.
pub const AUTOMATION_SCHEDULE_INVALID: &str = "automation.schedule_invalid";
/// An automation's time zone is unknown, or the host's zone cannot be read so the automation needs one.
pub const AUTOMATION_ZONE_INVALID: &str = "automation.zone_invalid";
/// A calendar window that ends before it starts, or spans more than 62 days.
pub const CALENDAR_WINDOW_INVALID: &str = "calendar.window_invalid";
