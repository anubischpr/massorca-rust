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
    (codes::GIT_NOT_A_REPOSITORY, "git.not_a_repository"),
    (
        codes::GIT_PATH_OUTSIDE_WORKTREE,
        "git.path_outside_worktree",
    ),
    (codes::GIT_REVISION_NOT_FOUND, "git.revision_not_found"),
    (codes::GIT_DISCARD_UNTRACKED, "git.discard_untracked"),
    (codes::GIT_DISCARD_STAGED, "git.discard_staged"),
    (codes::GIT_PATH_CONFLICTED, "git.path_conflicted"),
    (codes::GIT_PATH_IS_DIRECTORY, "git.path_is_directory"),
    (codes::GIT_PATHS_REQUIRED, "git.paths_required"),
    (codes::GIT_INDEX_LOCKED, "git.index_locked"),
    (codes::GIT_NOTHING_TO_COMMIT, "git.nothing_to_commit"),
    (codes::GIT_COMMIT_MESSAGE_EMPTY, "git.commit_message_empty"),
    (codes::GIT_COMMIT_PUBLISHED, "git.commit_published"),
    (codes::GIT_NO_COMMITS, "git.no_commits"),
    (codes::GIT_IDENTITY_UNKNOWN, "git.identity_unknown"),
    (
        codes::GIT_CHECKOUT_WOULD_OVERWRITE,
        "git.checkout_would_overwrite",
    ),
    (codes::GIT_AUTH_REQUIRED, "git.auth_required"),
    (codes::GIT_HOST_KEY_UNKNOWN, "git.host_key_unknown"),
    (codes::GIT_REMOTE_UNREACHABLE, "git.remote_unreachable"),
    (codes::GIT_REMOTE_NOT_FOUND, "git.remote_not_found"),
    (codes::GIT_NO_UPSTREAM, "git.no_upstream"),
    (codes::GIT_DETACHED_HEAD, "git.detached_head"),
    (codes::GIT_NO_OPERATION, "git.no_operation"),
    (codes::GIT_LOCAL_CHANGES, "git.local_changes"),
    (codes::GIT_WATCH_UNAVAILABLE, "git.watch_unavailable"),
    (codes::WORKSPACE_NOT_FOUND, "workspace.not_found"),
    (codes::WORKSPACE_NAME_TAKEN, "workspace.name_taken"),
    (codes::WORKSPACE_NAME_INVALID, "workspace.name_invalid"),
    (codes::WORKSPACE_PATH_TAKEN, "workspace.path_taken"),
    (codes::WORKSPACE_DIRTY, "workspace.dirty"),
    (codes::WORKSPACE_LOCKED, "workspace.locked"),
    (codes::WORKSPACE_BUSY, "workspace.busy"),
    (codes::WORKSPACE_MAIN_WORKTREE, "workspace.main_worktree"),
    (codes::BRANCH_EXISTS, "branch.exists"),
    (codes::BRANCH_NOT_FOUND, "branch.not_found"),
    (codes::BRANCH_NAME_INVALID, "branch.name_invalid"),
    (codes::BRANCH_UNMERGED, "branch.unmerged"),
    (codes::BRANCH_CHECKED_OUT, "branch.checked_out"),
    (codes::BRANCH_IN_USE, "branch.in_use"),
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
    (codes::AGENT_KIND_NOT_ENABLED, "agent.kind_not_enabled"),
    (codes::AGENT_NOT_INSTALLED, "agent.not_installed"),
    (codes::AGENT_WORKSPACE_BUSY, "agent.workspace_busy"),
    (codes::AGENT_NOT_LIVE, "agent.not_live"),
    (codes::AGENT_BUSY, "agent.busy"),
    (codes::AGENT_PROMPT_INVALID, "agent.prompt_invalid"),
    (codes::AGENT_RESUME_UNAVAILABLE, "agent.resume_unavailable"),
    (codes::AGENT_RESUME_NOT_FOUND, "agent.resume_not_found"),
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
    (codes::AGENT_MODE_UNKNOWN, "agent.mode_unknown"),
    (codes::AGENT_MODE_UNAVAILABLE, "agent.mode_unavailable"),
    (codes::AGENT_OPTION_UNSUPPORTED, "agent.option_unsupported"),
    (
        codes::AGENT_ONESHOT_UNSUPPORTED,
        "agent.oneshot_unsupported",
    ),
    (codes::AGENT_ONESHOT_TIMEOUT, "agent.oneshot_timeout"),
    (codes::AGENT_ONESHOT_FAILED, "agent.oneshot_failed"),
    (codes::AGENT_QUEUE_UNAVAILABLE, "agent.queue_unavailable"),
    (codes::AGENT_QUEUE_FULL, "agent.queue_full"),
    (codes::AGENT_QUEUE_NOT_FOUND, "agent.queue_not_found"),
    (
        codes::AGENT_QUEUE_ALREADY_DELIVERED,
        "agent.queue_already_delivered",
    ),
    (codes::AGENT_ARCHIVE_FAILED, "agent.archive_failed"),
    (codes::TEAM_NOT_FOUND, "team.not_found"),
    (codes::TEAM_NAME_IN_USE, "team.name_in_use"),
    (codes::TEAM_PROJECT_NOT_FOUND, "team.project_not_found"),
    (codes::TEAM_PROJECT_NAME_IN_USE, "team.project_name_in_use"),
    (codes::TEAM_ROUTINE_NOT_FOUND, "team.routine_not_found"),
    (
        codes::TEAM_ROUTINE_RUN_INTERRUPTED,
        "team.routine_run_interrupted",
    ),
    (codes::TEAM_ROUTINE_CHANGED, "team.routine_changed"),
    (codes::BOT_NOT_FOUND, "bot.not_found"),
    (codes::BOT_NAME_IN_USE, "bot.name_in_use"),
    (codes::BOT_LEAD_TAKEN, "bot.lead_taken"),
    (codes::BOT_HAS_TASKS, "bot.has_tasks"),
    (codes::TASK_NOT_FOUND, "task.not_found"),
    (codes::TASK_RUNNING, "task.running"),
    (codes::EXTENSION_UP_TO_DATE, "extension.up_to_date"),
    (codes::AUTOMATION_NOT_FOUND, "automation.not_found"),
    (
        codes::AUTOMATION_SCHEDULE_INVALID,
        "automation.schedule_invalid",
    ),
    (codes::AUTOMATION_ZONE_INVALID, "automation.zone_invalid"),
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
    assert_eq!(capabilities::GIT, "git/1");
    assert_eq!(capabilities::EVENTS, "events/1");
    assert_eq!(capabilities::CONFIG, "config/1");
    assert_eq!(capabilities::TERMINALS, "terminals/1");
    assert_eq!(capabilities::BINARY_FRAMES, "binary-frames/1");
    assert_eq!(capabilities::AGENTS, "agents/1");
    assert_eq!(capabilities::AGENT_MODES, "agent-modes/1");
    assert_eq!(capabilities::AGENT_SEEN, "agent-seen/1");
    assert_eq!(capabilities::AGENT_QUEUE, "agent-queue/1");
    assert_eq!(capabilities::AGENT_RELEASE, "agent-release/1");
    assert_eq!(capabilities::AGENT_ARCHIVE, "agent-archive/1");
    assert_eq!(capabilities::AGENT_NO_PROJECT, "agent-no-project/1");
    assert_eq!(capabilities::AGENT_OPTIONS, "agent-options/1");
    assert_eq!(capabilities::AGENT_FAST, "agent-fast/1");
    assert_eq!(capabilities::AGENT_CHAT, "agent-chat/1");
    assert_eq!(capabilities::AGENT_IMPORT_DETECT, "agent-import-detect/1");
    assert_eq!(capabilities::AGENT_TITLE, "agent-title/1");
    assert_eq!(capabilities::TEAMS, "teams/1");
    assert_eq!(capabilities::AUTOMATIONS, "automations/1");
}
