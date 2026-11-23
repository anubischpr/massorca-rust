//! `massorca service ...`: running the service in the foreground, controlling a running one, and printing its unit.

pub mod control;
pub mod foreground;
pub mod parent_watch;
pub mod unit;
