//! Where the stable `massorca` command lives on each platform, and what a Windows command file says (decision 0088).

use std::path::{Path, PathBuf};

/// The command's name without an extension.
pub const COMMAND_NAME: &str = "massorca";

/// First line of a command file: cmd prints nothing but what the binary prints.
const ECHO_OFF: &str = "@echo off";
/// Marks a command file as one `massorca cli install` wrote; uninstall removes only files that carry it.
const MARKER: &str = "REM massorca cli install manages this file";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Unix,
    Windows,
}

impl Platform {
    /// The platform this build runs on.
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

/// The folder a stable command lives in, and the command's own path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub platform: Platform,
    pub bin_dir: PathBuf,
    pub command_path: PathBuf,
}

impl Layout {
    /// `~/.local/bin/massorca`: a link in a folder any user may write to, so no administrator is needed.
    pub fn unix(home: &Path) -> Self {
        let bin_dir = home.join(".local").join("bin");
        let command_path = bin_dir.join(COMMAND_NAME);
        Self {
            platform: Platform::Unix,
            bin_dir,
            command_path,
        }
    }

    /// `%LOCALAPPDATA%\Programs\MassOrca\bin\massorca.cmd`: a per-user folder, as the reference keeps its command.
    pub fn windows(local_app_data: &Path) -> Self {
        let bin_dir = local_app_data.join("Programs").join("MassOrca").join("bin");
        let command_path = bin_dir.join(format!("{COMMAND_NAME}.cmd"));
        Self {
            platform: Platform::Windows,
            bin_dir,
            command_path,
        }
    }
}

/// The text of a Windows command file that runs `target` with every argument, or `None` when cmd could misread the
/// path: a quote or percent sign would end the quoted path or be expanded, and a line break would split the line.
pub fn command_file_text(target: &Path) -> Option<String> {
    let target = target.to_str()?;
    if target.contains(['"', '%', '\r', '\n']) {
        return None;
    }
    Some(format!("{ECHO_OFF}\r\n{MARKER}\r\n\"{target}\" %*\r\n"))
}

/// The binary a command file runs, when the text is one `command_file_text` wrote for a massorca binary.
pub fn command_file_target(text: &str) -> Option<PathBuf> {
    let mut lines = text.split('\n').map(|line| line.trim_end_matches('\r'));
    if lines.next()? != ECHO_OFF || lines.next()? != MARKER {
        return None;
    }
    let target = lines.next()?.strip_prefix('"')?.strip_suffix("\" %*")?;
    if !lines.all(str::is_empty) {
        return None;
    }
    let path = PathBuf::from(target);
    is_massorca_binary(&path).then_some(path)
}

/// Whether two paths name the same place: Windows ignores case and whether separators are `\` or `/`.
pub fn same_path(platform: Platform, left: &Path, right: &Path) -> bool {
    match platform {
        Platform::Unix => left == right,
        Platform::Windows => windows_key(left) == windows_key(right),
    }
}

fn windows_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Whether the path's file name is the massorca binary.
pub fn is_massorca_binary(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("massorca" | "massorca.exe")
    )
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn unix_command_is_a_link_in_the_home_local_bin_folder() {
        let layout = Layout::unix(Path::new("home-ana"));
        assert_eq!(layout.platform, Platform::Unix);
        assert_eq!(
            layout.bin_dir,
            Path::new("home-ana").join(".local").join("bin")
        );
        assert_eq!(layout.command_path, layout.bin_dir.join("massorca"));
    }

    #[test]
    fn windows_command_is_a_cmd_file_in_the_per_user_programs_folder() {
        let layout = Layout::windows(Path::new("local-ana"));
        assert_eq!(layout.platform, Platform::Windows);
        assert_eq!(
            layout.bin_dir,
            Path::new("local-ana")
                .join("Programs")
                .join("MassOrca")
                .join("bin")
        );
        assert_eq!(layout.command_path, layout.bin_dir.join("massorca.cmd"));
    }

    #[test]
    fn command_file_runs_the_binary_with_every_argument_and_prints_nothing_itself() {
        let text =
            command_file_text(Path::new("opt/massorca/massorca.exe")).expect("usable target");
        assert_eq!(
            text,
            "@echo off\r\nREM massorca cli install manages this file\r\n\"opt/massorca/massorca.exe\" %*\r\n"
        );
    }

    #[test]
    fn command_file_text_refuses_a_target_that_cmd_would_misread() {
        for target in [
            "opt/a%b/massorca.exe",
            "opt/a\"b/massorca.exe",
            "opt/a\r\nb/massorca.exe",
        ] {
            assert_eq!(command_file_text(Path::new(target)), None, "{target:?}");
        }
    }

    #[test]
    fn command_file_target_reads_back_what_command_file_text_wrote() {
        let text =
            command_file_text(Path::new("opt/massorca/massorca.exe")).expect("usable target");
        assert_eq!(
            command_file_target(&text),
            Some(PathBuf::from("opt/massorca/massorca.exe"))
        );
    }

    #[test]
    fn command_file_target_refuses_text_massorca_did_not_write() {
        let marked =
            command_file_text(Path::new("opt/massorca/massorca.exe")).expect("usable target");
        let other_program = "@echo off\r\nREM massorca cli install manages this file\r\n\"opt/tool/tool.exe\" %*\r\n";
        assert_eq!(
            command_file_target("@echo off\r\ncall tool.bat %*\r\n"),
            None
        );
        assert_eq!(command_file_target(other_program), None);
        assert_eq!(
            command_file_target(&format!("{marked}echo extra\r\n")),
            None
        );
    }

    #[test]
    fn windows_paths_match_regardless_of_case_and_separator_style() {
        assert!(same_path(
            Platform::Windows,
            Path::new(r"C:\Apps\MassOrca\massorca.exe"),
            Path::new("c:/apps/massorca/MASSORCA.EXE"),
        ));
        assert!(same_path(
            Platform::Windows,
            Path::new(r"C:\Apps\bin\"),
            Path::new(r"C:\Apps\bin"),
        ));
        assert!(!same_path(
            Platform::Windows,
            Path::new(r"C:\Apps\massorca.exe"),
            Path::new(r"C:\Apps\other.exe"),
        ));
    }

    #[test]
    fn unix_paths_match_exactly() {
        assert!(same_path(
            Platform::Unix,
            Path::new("/opt/massorca"),
            Path::new("/opt/massorca"),
        ));
        assert!(!same_path(
            Platform::Unix,
            Path::new("/opt/Massorca"),
            Path::new("/opt/massorca"),
        ));
    }

    #[test]
    fn only_the_massorca_binary_name_counts_as_ours() {
        assert!(is_massorca_binary(Path::new("opt/massorca/massorca")));
        assert!(is_massorca_binary(Path::new("massorca.exe")));
        assert!(!is_massorca_binary(Path::new("usr/bin/orca")));
    }
}
