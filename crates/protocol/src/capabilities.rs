//! Named, versioned feature groups. The server offers them in `welcome`; a client calls only the methods of offered groups.

pub const SERVICE: &str = "service/1";
/// Developer tools on the service's host: `service.tools` (PLAT-08, decision 0116).
pub const SERVICE_TOOLS: &str = "service-tools/1";
pub const PROJECTS: &str = "projects/1";
/// `project.init_git` (decision 0115): turning a folder project into a git project.
pub const PROJECT_INIT_GIT: &str = "project-init-git/1";
/// `project.update` (decision 0091): changing a project's icon.
pub const PROJECT_UPDATE: &str = "project-update/1";
/// The colour of a Lucide project icon (`ProjectIcon::Lucide` `color`): an older service would drop it silently.
pub const PROJECT_ICON_COLOR: &str = "project-icon-color/1";
/// Per-script trust of a project's `orca.yaml` scripts (`project.scripts`, `project.trust_script`, decision 0095).
pub const SCRIPT_TRUST: &str = "script-trust/1";
pub const WORKSPACES: &str = "workspaces/1";
/// Renaming a workspace and setting its comment (`workspace.update`).
pub const WORKSPACE_UPDATE: &str = "workspace-update/1";
/// Checking a workspace's folder and its git registration (`workspace.check`).
pub const WORKSPACE_CHECK: &str = "workspace-check/1";
/// The containing-workspace lookup (`workspace.current`), offered apart so a client knows the service answers it.
pub const WORKSPACE_CURRENT: &str = "workspace-current/1";
/// `unique_name` on `workspace.create`: a taken name, branch or folder moves on to the next free `name-N`.
pub const WORKSPACE_UNIQUE_NAME: &str = "workspace-unique-name/1";
/// `workspace.rename` (decision 0111): replacing a name the service chose, and its branch.
pub const WORKSPACE_RENAME: &str = "workspace-rename/1";
/// `in_place` on `workspace.create` (decision 0112): the project's own checkout becomes its workspace, and removal only forgets it.
pub const WORKSPACE_IN_PLACE: &str = "workspace-in-place/1";
pub const EVENTS: &str = "events/1";
pub const CONFIG: &str = "config/1";
/// Paired devices and pairing offers (decision 0041).
pub const DEVICES: &str = "devices/1";
/// The network listener's state (decision 0040).
pub const NETWORK: &str = "network/1";
/// `addresses` on `pairing.create`: the offer lists the addresses the caller gives instead of the bound listeners.
pub const PAIRING_ADDRESS: &str = "pairing-address/1";
/// `pairing.cancel` and `offer_id` on a created offer: retiring a pending offer before it expires.
pub const PAIRING_CANCEL: &str = "pairing-cancel/1";
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
/// Archive and delete of an agent session (decision 0163): `agent.archive_session`, the session's `archived`,
/// `include_archived` on `agent.list`, `remove_workspace` and `force` on `agent.forget`, and `agent.forget_preview`.
pub const AGENT_ARCHIVE: &str = "agent-archive/1";
/// `agent.start` without a `workspace`: the session runs in the conversations folder, in a folder of its own (decision
/// 0094).
pub const AGENT_NO_PROJECT: &str = "agent-no-project/1";
/// `model` and `effort` on `agent.start` (decision 0094): passed to the flags the kind declares for them.
pub const AGENT_OPTIONS: &str = "agent-options/1";
/// `attachments` on `agent.prompt` and `agent.queue` (decision 0172): files of the session's workspace sent with a prompt,
/// typed as mentions on a terminal run and sent with a chat run.
pub const AGENT_PROMPT_ATTACHMENTS: &str = "agent-prompt-attachments/1";
/// `agent.set_options` (decision 0171): a session's `model`, `effort` and `fast` change while it runs, typed into the run
/// where its kind declares a live command for them, and kept for the session's next resume otherwise.
pub const AGENT_OPTIONS_LIVE: &str = "agent-options-live/1";
/// `fast` on `agent.start` and `fast` on the kinds, models and sessions (decision 0165): fast mode passed to the settings the
/// kind declares for it.
pub const AGENT_FAST: &str = "agent-fast/1";
/// `agent.fork` (decision 0168): a new session from a copy of an exited session's conversation, by the kind's fork form.
pub const AGENT_FORK: &str = "agent-fork/1";
/// Structured chat sessions (`agent.chat.*`, decision 0101): `agent.chat.start`, the transcript stream, history paging,
/// approvals and interrupt, and `chat` on `agent.kinds`. Not offered until the service runs chat sessions.
pub const AGENT_CHAT: &str = "agent-chat/1";
/// Import source detection (decision 0097): `agent.accounts.import_candidates`, and `agent.accounts.import` without
/// `from`, which copies each declared file from where the kind's CLI keeps it.
pub const AGENT_IMPORT_DETECT: &str = "agent-import-detect/1";
/// `agent.models`: the models a kind's picker offers for an account, declared and listed by its vendor CLI (decision 0166).
pub const AGENT_MODELS: &str = "agent-models/1";
/// Chat titles (`agent.generate_title`, decision 0167): a short title named from a chat's first prompt and reply by a
/// one-shot run of the kind.
pub const AGENT_TITLE: &str = "agent-title/1";
pub const GIT: &str = "git/1";
/// `project` on `git.branches`: a git project's own checkout, listed with no workspace.
pub const GIT_PROJECT_BRANCHES: &str = "git-project-branches/1";
/// The hosting identity of a repository's remotes (`git.remotes`, decision 0114).
pub const GIT_REMOTE_IDENTITY: &str = "git-remote-identity/1";
/// The secret vault (`secret.*`, decision 0071).
pub const SECRETS: &str = "secrets/1";
/// Bot Teams methods (`team.*`, `task.*`) and the `team.changed` and `task.changed` topics (decision 0072).
pub const TEAMS: &str = "teams/1";
/// A team's Conference room (`team.room_list`, `team.room_post`) and the `team.room_changed` topic (decision 0102).
pub const TEAM_ROOM: &str = "team-room/1";
/// Choice cards in a team's room: the optional `card` field of a room line and `team.room_answer` (decision 0121). A
/// host that has it also keeps `team-room/1`.
pub const TEAM_ROOM_CARDS: &str = "team-room/2";
/// Conversation replies in a team's room: `ask` and `reply_to` of `team.room_post`, and the `quoted` and `typing` fields
/// of `team.room_list` (decision 0122). A host that has it also keeps `team-room/1` and `team-room/2`.
pub const TEAM_ROOM_REPLIES: &str = "team-room/3";
/// Extension install and management (`extensions.*`, decision 0062).
pub const EXTENSIONS: &str = "extensions/1";
/// Rolling a package back to the version its update replaced (`extensions.rollback`).
pub const EXTENSIONS_ROLLBACK: &str = "extensions-rollback/1";
/// Updating a host program a tool package installed (`extensions.program_update_review`, `extensions.program_update`,
/// decision 0151).
pub const EXTENSIONS_PROGRAMS: &str = "extensions-programs/1";
/// What each owner registered on the tool, skill, view, theme and status-rule points (`extensions.points`, decision 0153).
pub const EXTENSION_POINTS: &str = "extension-points/1";
/// Usage and limits of each agent account, read on the host that runs the agent (`usage.list`, `usage.refresh` and the
/// `usage.changed` topic, decision 0154).
pub const USAGE: &str = "usage/1";
/// The run log of a package (`extensions.log`).
pub const EXTENSIONS_LOG: &str = "extensions-log/1";
/// Skill discovery (`skills.discover`, SKL-01): the skills agent tools and registered folders keep on this host.
pub const SKILLS: &str = "skills/1";
/// Skill placement (`skills.place`, `skills.unplace`, `skills.managed`, SKL-02): copies of skills in agent tools' folders.
pub const SKILLS_PLACE: &str = "skills-place/1";
/// The skills MassOrca ships (`skills.bundled.list`, `skills.bundled.install`, `skills.bundled.refresh`, SKL-04).
pub const SKILLS_BUNDLED: &str = "skills-bundled/1";
/// Hosted-service connections (`marketplace.service.*`, SKL-35, decision 0097).
pub const SERVICES: &str = "services/1";
/// Figures read on the host that runs the service: `host.memory` (decision 0081).
pub const HOST: &str = "host/1";
/// Port forwards to loopback ports on the host (`host.forward*`, decision 0131): needs `binary-frames/1` too.
pub const PORT_FORWARD: &str = "port-forward/1";
/// The command table: `commands.describe`, which lists every command with its method, permission and parameters.
pub const COMMANDS: &str = "commands/1";
/// Scheduled automation definitions (`automation.*`, decision 0123). Not offered by a service that predates them.
pub const AUTOMATIONS: &str = "automations/1";
/// The Calendar (`calendar.occurrences`, decision 0124). Not offered by a service that predates it.
pub const CALENDAR: &str = "calendar/1";
