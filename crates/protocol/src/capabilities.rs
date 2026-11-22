//! Named, versioned feature groups. The server offers them in `welcome`; a client calls only the methods of offered groups.

pub const SERVICE: &str = "service/1";
pub const PROJECTS: &str = "projects/1";
pub const WORKSPACES: &str = "workspaces/1";
pub const EVENTS: &str = "events/1";
pub const CONFIG: &str = "config/1";
/// Terminal methods (`terminal.*`) and the attach stream.
pub const TERMINALS: &str = "terminals/1";
/// Binary frame kinds beyond JSON; terminal output rides on frame kind `0x02`.
pub const BINARY_FRAMES: &str = "binary-frames/1";
pub const GIT: &str = "git/1";
