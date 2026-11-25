//! Named, versioned feature groups. The server offers them in `welcome`; a client calls only the methods of offered groups.

pub const SERVICE: &str = "service/1";
pub const PROJECTS: &str = "projects/1";
/// `project.update` (decision 0091): changing a project's icon.
pub const PROJECT_UPDATE: &str = "project-update/1";
pub const WORKSPACES: &str = "workspaces/1";
/// Renaming a workspace and setting its comment (`workspace.update`).
pub const WORKSPACE_UPDATE: &str = "workspace-update/1";
/// Checking a workspace's folder and its git registration (`workspace.check`).
pub const WORKSPACE_CHECK: &str = "workspace-check/1";
/// The containing-workspace lookup (`workspace.current`), offered apart so a client knows the service answers it.
pub const WORKSPACE_CURRENT: &str = "workspace-current/1";
pub const EVENTS: &str = "events/1";
pub const CONFIG: &str = "config/1";
/// Paired devices and pairing offers (decision 0041).
pub const DEVICES: &str = "devices/1";
/// The network listener's state (decision 0040).
pub const NETWORK: &str = "network/1";
/// Terminal methods (`terminal.*`) and the attach stream.
pub const TERMINALS: &str = "terminals/1";
/// `terminal.wait`: a bounded wait for a terminal's exit (decision 0087).
pub const TERMINAL_WAIT: &str = "terminal-wait/1";
/// Binary frame kinds beyond JSON; terminal output rides on frame kind `0x02`.
pub const BINARY_FRAMES: &str = "binary-frames/1";
/// Agent methods (`agent.*`) and the agent watch stream (decision 0057).
pub const AGENTS: &str = "agents/1";
/// Agent permission modes (decision 0083): `permission_mode` on `agent.start`, `agent.delegate` and `agent.resume`, the
/// session's `permission_mode` and `mode_confirmed`, and `agent.set_mode`.
pub const AGENT_MODES: &str = "agent-modes/1";
/// The prompt queue (`agent.queue*`, decision 0084): queued prompts that the service delivers when the agent is ready.
pub const AGENT_QUEUE: &str = "agent-queue/1";
/// Seen state of an agent's attention states: `seen_at_ms` on a session's activity, and `agent.mark_seen`.
pub const AGENT_SEEN: &str = "agent-seen/1";
/// Release and retain of an agent session, and its archived output (`agent.release`, `agent.retain`, `agent.archive`,
/// decision 0090).
pub const AGENT_RELEASE: &str = "agent-release/1";
/// `agent.start` without a `workspace`: the session runs in the conversations folder, in a folder of its own (decision
/// 0094).
pub const AGENT_NO_PROJECT: &str = "agent-no-project/1";
/// `model` and `effort` on `agent.start` (decision 0094): passed to the flags the kind declares for them.
pub const AGENT_OPTIONS: &str = "agent-options/1";
pub const GIT: &str = "git/1";
/// The secret vault (`secret.*`, decision 0071).
pub const SECRETS: &str = "secrets/1";
/// Bot Teams methods (`team.*`, `task.*`) and the `team.changed` and `task.changed` topics (decision 0072).
pub const TEAMS: &str = "teams/1";
/// Extension install and management (`extensions.*`, decision 0062).
pub const EXTENSIONS: &str = "extensions/1";
/// Skill discovery (`skills.discover`, SKL-01): the skills agent tools and registered folders keep on this host.
pub const SKILLS: &str = "skills/1";
/// Figures read on the host that runs the service: `host.memory` (decision 0081).
pub const HOST: &str = "host/1";
/// The command table: `commands.describe`, which lists every command with its method, permission and parameters.
pub const COMMANDS: &str = "commands/1";
