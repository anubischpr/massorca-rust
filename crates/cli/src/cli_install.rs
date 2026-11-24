//! `massorca cli install|uninstall|status`: puts the `massorca` command on this user's PATH and nothing else (decision 0088).

pub mod command_file;
pub mod layout;
pub mod path_entries;
pub mod plan;
pub mod user_path;
#[cfg(windows)]
pub mod windows_user_path;
