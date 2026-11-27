//! Shells in pseudo-terminals that keep running while clients come and go (docs/architecture/terminals.md).
//!
//! No protocol and no async runtime: the service maps requests onto this crate.

pub mod attach;
pub mod credential_guard;
pub mod manager;
pub mod process_tree;
pub mod pty;
pub mod queries;
pub mod ring;
pub mod screen;
pub mod shell;

/// An environment value of a run: zeroed when dropped, because it may be a vault secret (decision 0071).
pub type EnvValue = zeroize::Zeroizing<String>;

pub use attach::{AttachItem, AttachMode, Attachment, QueryAnswers};
pub use manager::{
    CreateSpec, ExitReason, ExitRecord, ExitWait, ManagerConfig, ShellChoice, TerminalError,
    TerminalInfo, TerminalManager, TerminalMembers, TerminalSample, TerminalState, TerminalTheme,
};
