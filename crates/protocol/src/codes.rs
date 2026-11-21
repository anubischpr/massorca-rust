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
pub const BASE_NOT_FOUND: &str = "base.not_found";
pub const GIT_UNAVAILABLE: &str = "git.unavailable";
pub const GIT_FAILED: &str = "git.failed";
pub const SELECTOR_AMBIGUOUS: &str = "selector.ambiguous";
pub const CONFIG_UNKNOWN_KEY: &str = "config.unknown_key";
pub const CONFIG_INVALID_VALUE: &str = "config.invalid_value";
/// The store was written by a newer build; it is left untouched (decision 0005).
pub const STORE_NEWER_SCHEMA: &str = "store.newer_schema";

/// Reserved namespace for extension error codes; core never defines codes under it.
pub const EXTENSION_PREFIX: &str = "extension.";
