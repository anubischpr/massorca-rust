//! Shells in pseudo-terminals that keep running while clients come and go (docs/architecture/terminals.md).
//!
//! No protocol and no async runtime: the service maps requests onto this crate.

pub mod process_tree;
pub mod pty;
pub mod ring;
pub mod shell;
