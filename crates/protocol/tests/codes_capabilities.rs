use massorca_protocol::{capabilities, codes};
use std::collections::HashSet;

/// Every error code with its wire value. Clients branch on these strings, so they are pinned here.
const ERROR_CODES: &[(&str, &str)] = &[
    (
        codes::PROTOCOL_UNSUPPORTED_VERSION,
        "protocol.unsupported_version",
    ),
    (codes::PROTOCOL_HANDSHAKE, "protocol.handshake"),
    (codes::METHOD_NOT_FOUND, "method.not_found"),
    (
        codes::CAPABILITY_NOT_NEGOTIATED,
        "method.capability_not_negotiated",
    ),
    (codes::PERMISSION_DENIED, "permission.denied"),
    (codes::REQUEST_INVALID_PARAMS, "request.invalid_params"),
    (codes::INTERNAL, "internal"),
    (codes::SERVICE_SHUTTING_DOWN, "service.shutting_down"),
    (codes::STREAM_NOT_FOUND, "stream.not_found"),
    (codes::STREAM_OVERFLOW, "stream.overflow"),
    (codes::STREAM_ITEM_TOO_LARGE, "stream.item_too_large"),
    (
        codes::STREAM_BINARY_FRAMES_REQUIRED,
        "stream.binary_frames_required",
    ),
    (codes::RESPONSE_TOO_LARGE, "response.too_large"),
    (codes::PATH_NOT_UTF8, "path.not_utf8"),
    (codes::PATH_NOT_FOUND, "path.not_found"),
    (codes::PATH_NOT_DIRECTORY, "path.not_directory"),
    (codes::PROJECT_NOT_FOUND, "project.not_found"),
    (codes::PROJECT_EXISTS, "project.exists"),
    (codes::PROJECT_NOT_GIT, "project.not_git"),
    (codes::PROJECT_HAS_WORKSPACES, "project.has_workspaces"),
    (codes::WORKSPACE_NOT_FOUND, "workspace.not_found"),
    (codes::WORKSPACE_NAME_TAKEN, "workspace.name_taken"),
    (codes::WORKSPACE_NAME_INVALID, "workspace.name_invalid"),
    (codes::WORKSPACE_PATH_TAKEN, "workspace.path_taken"),
    (codes::WORKSPACE_DIRTY, "workspace.dirty"),
    (codes::WORKSPACE_LOCKED, "workspace.locked"),
    (codes::WORKSPACE_MAIN_WORKTREE, "workspace.main_worktree"),
    (codes::BRANCH_EXISTS, "branch.exists"),
    (codes::BRANCH_NOT_FOUND, "branch.not_found"),
    (codes::BASE_NOT_FOUND, "base.not_found"),
    (codes::GIT_UNAVAILABLE, "git.unavailable"),
    (codes::GIT_FAILED, "git.failed"),
    (codes::SELECTOR_AMBIGUOUS, "selector.ambiguous"),
    (codes::CONFIG_UNKNOWN_KEY, "config.unknown_key"),
    (codes::CONFIG_INVALID_VALUE, "config.invalid_value"),
    (codes::STORE_NEWER_SCHEMA, "store.newer_schema"),
    (codes::TERMINAL_NOT_FOUND, "terminal.not_found"),
    (codes::TERMINAL_EXITED, "terminal.exited"),
    (codes::TERMINAL_SPAWN_FAILED, "terminal.spawn_failed"),
    (codes::TERMINAL_INPUT_TOO_LARGE, "terminal.input_too_large"),
    (codes::TERMINAL_INVALID_SIZE, "terminal.invalid_size"),
    (codes::TERMINAL_LIMIT_REACHED, "terminal.limit_reached"),
    (codes::AGENT_NOT_FOUND, "agent.not_found"),
    (codes::AGENT_KIND_NOT_FOUND, "agent.kind_not_found"),
    (codes::AGENT_NOT_INSTALLED, "agent.not_installed"),
    (codes::AGENT_WORKSPACE_BUSY, "agent.workspace_busy"),
    (codes::AGENT_NOT_LIVE, "agent.not_live"),
    (codes::AGENT_BUSY, "agent.busy"),
    (codes::AGENT_PROMPT_INVALID, "agent.prompt_invalid"),
    (codes::AGENT_RESUME_UNAVAILABLE, "agent.resume_unavailable"),
    (codes::AGENT_HOOK_REFUSED, "agent.hook_refused"),
    (codes::AGENT_DELEGATE_NEEDS_GIT, "agent.delegate_needs_git"),
    (codes::AGENT_DEPTH_EXCEEDED, "agent.depth_exceeded"),
    (codes::AGENT_PARENT_BUSY, "agent.parent_busy"),
    (codes::AGENT_PARENT_DIRTY, "agent.parent_dirty"),
    (codes::AGENT_MERGE_CONFLICT, "agent.merge_conflict"),
    (
        codes::AGENT_ALREADY_BROUGHT_BACK,
        "agent.already_brought_back",
    ),
    (
        codes::AGENT_CREDENTIALS_UNSUPPORTED,
        "agent.credentials_unsupported",
    ),
    (codes::AGENT_CREDENTIALS_EXIST, "agent.credentials_exist"),
    (
        codes::AGENT_CREDENTIALS_INVALID,
        "agent.credentials_invalid",
    ),
    (codes::EXTENSION_PREFIX, "extension."),
];

#[test]
fn error_codes_match_the_wire_contract() {
    for &(constant, expected) in ERROR_CODES {
        assert_eq!(constant, expected);
    }
}

#[test]
fn error_code_values_are_unique() {
    let values: HashSet<&str> = ERROR_CODES.iter().map(|&(_, value)| value).collect();
    assert_eq!(values.len(), ERROR_CODES.len());
}

#[test]
fn capability_names_match_the_contract() {
    assert_eq!(capabilities::SERVICE, "service/1");
    assert_eq!(capabilities::PROJECTS, "projects/1");
    assert_eq!(capabilities::WORKSPACES, "workspaces/1");
    assert_eq!(capabilities::EVENTS, "events/1");
    assert_eq!(capabilities::CONFIG, "config/1");
    assert_eq!(capabilities::TERMINALS, "terminals/1");
    assert_eq!(capabilities::BINARY_FRAMES, "binary-frames/1");
    assert_eq!(capabilities::AGENTS, "agents/1");
}
