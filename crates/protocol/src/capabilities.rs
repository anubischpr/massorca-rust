//! Named, versioned feature groups. The server offers them in `welcome`; a client calls only the methods of offered groups.

pub const SERVICE: &str = "service/1";
pub const PROJECTS: &str = "projects/1";
pub const WORKSPACES: &str = "workspaces/1";
pub const EVENTS: &str = "events/1";
pub const CONFIG: &str = "config/1";
