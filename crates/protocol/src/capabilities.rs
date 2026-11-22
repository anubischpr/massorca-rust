//! Named, versioned feature groups. The server offers them in `welcome`; a client calls only the methods of offered groups.

pub const SERVICE: &str = "service/1";
pub const PROJECTS: &str = "projects/1";
pub const WORKSPACES: &str = "workspaces/1";
pub const EVENTS: &str = "events/1";
pub const CONFIG: &str = "config/1";
/// Paired devices and pairing offers (decision 0041).
pub const DEVICES: &str = "devices/1";
/// The network listener's state (decision 0040).
pub const NETWORK: &str = "network/1";
/// Terminal methods (`terminal.*`) and the attach stream.
pub const TERMINALS: &str = "terminals/1";
/// Binary frame kinds beyond JSON; terminal output rides on frame kind `0x02`.
pub const BINARY_FRAMES: &str = "binary-frames/1";
/// Agent methods (`agent.*`) and the agent watch stream (decision 0057).
pub const AGENTS: &str = "agents/1";
pub const GIT: &str = "git/1";
/// Bot Teams methods (`team.*`, `task.*`) and the `team.changed` and `task.changed` topics (decision 0072).
pub const TEAMS: &str = "teams/1";
