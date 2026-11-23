//! Shells in pseudo-terminals that keep running while clients come and go (docs/architecture/terminals.md).
//!
//! No protocol and no async runtime: the service maps requests onto this crate.

pub mod attach;
pub mod manager;
pub mod process_tree;
pub mod pty;
pub mod ring;
pub mod screen;
pub mod shell;

pub use attach::{AttachItem, AttachMode, Attachment, QueryAnswers};
pub use manager::{
    CreateSpec, ExitReason, ExitRecord, ManagerConfig, ShellChoice, TerminalError, TerminalInfo,
    TerminalManager, TerminalMembers, TerminalSample, TerminalState,
};
