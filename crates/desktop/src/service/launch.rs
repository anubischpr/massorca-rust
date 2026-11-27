//! How the desktop picks its profile and whether it may start the service. The profile follows the
//! CLI's rule (`--home` and `--profile`, then `MASSORCA_HOME` and `MASSORCA_PROFILE`), and so does
//! autostart (on unless `--no-autostart` or `MASSORCA_NO_AUTOSTART`). The service is run by the
//! `massorca` executable that sits next to this one (decision 0028).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use massorca_client::Autostart;
use massorca_profile::Profile;

use crate::launch_options::DebugPage;

const CLI_NAME: &str = if cfg!(windows) {
    "massorca.exe"
} else {
    "massorca"
};
const NO_AUTOSTART_ENV: &str = "MASSORCA_NO_AUTOSTART";

/// What the command line asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    /// Run on sample data and never touch a service.
    pub sample: bool,
    pub profile: Option<String>,
    pub home: Option<PathBuf>,
    pub no_autostart: bool,
    /// Debug only: the page or dialog to open once its data has arrived (`--page`).
    pub page: Option<DebugPage>,
}

impl LaunchOptions {
    /// Reads the arguments after the program name. `env` answers `MASSORCA_NO_AUTOSTART`.
    pub fn parse(
        args: impl IntoIterator<Item = OsString>,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, String> {
        let mut options = Self {
            no_autostart: env(NO_AUTOSTART_ENV).is_some_and(|value| truthy(&value)),
            ..Self::default()
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let arg = arg.to_string_lossy().into_owned();
            match arg.as_str() {
                "--sample" => options.sample = true,
                "--no-autostart" => options.no_autostart = true,
                "--profile" => options.profile = Some(value_of(&arg, args.next())?),
                "--home" => options.home = Some(PathBuf::from(value_of(&arg, args.next())?)),
                "--page" => options.page = Some(value_of(&arg, args.next())?.parse()?),
                other => return Err(format!("unknown argument {other}")),
            }
        }
        Ok(options)
    }

    /// The profile this run talks to.
    pub fn resolve_profile(&self) -> Result<Profile, String> {
        Profile::resolve(self.home.clone(), self.profile.clone()).map_err(|e| e.to_string())
    }

    /// The service launch the client may use, or `None` when autostart is off or no `massorca`
    /// executable sits next to the desktop.
    pub fn autostart(&self, desktop_exe: &Path) -> Option<Autostart> {
        if self.no_autostart {
            return None;
        }
        autostart_next_to(desktop_exe)
    }
}

/// `massorca` beside `desktop_exe`, when it exists.
pub fn autostart_next_to(desktop_exe: &Path) -> Option<Autostart> {
    let exe = desktop_exe.parent()?.join(CLI_NAME);
    exe.is_file().then(|| Autostart {
        exe,
        // Lets the service repoint a stale `massorca` command at itself after an update (decision 0113).
        args_extra: vec!["--repair-command".to_owned()],
    })
}

fn value_of(flag: &str, value: Option<OsString>) -> Result<String, String> {
    value
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{flag} needs a value"))
}

/// The CLI reads this variable with clap's falsey rule: these spellings mean off.
pub(crate) fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off" | "n" | "f"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str], env: &[(&str, &str)]) -> Result<LaunchOptions, String> {
        LaunchOptions::parse(args.iter().map(OsString::from), |key| {
            env.iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn no_arguments_use_the_client_and_may_autostart() {
        assert_eq!(parse(&[], &[]), Ok(LaunchOptions::default()));
    }

    #[test]
    fn the_flags_select_the_source_profile_and_autostart() {
        let options = parse(
            &[
                "--sample",
                "--profile",
                "dev",
                "--home",
                "/tmp/h",
                "--no-autostart",
            ],
            &[],
        )
        .expect("parses");
        assert!(options.sample && options.no_autostart);
        assert_eq!(options.profile.as_deref(), Some("dev"));
        assert_eq!(options.home, Some(PathBuf::from("/tmp/h")));
    }

    #[test]
    fn the_environment_switches_autostart_off_like_the_cli() {
        assert!(
            parse(&[], &[("MASSORCA_NO_AUTOSTART", "1")])
                .expect("ok")
                .no_autostart
        );
        assert!(
            !parse(&[], &[("MASSORCA_NO_AUTOSTART", "0")])
                .expect("ok")
                .no_autostart
        );
        assert!(
            !parse(&[], &[("MASSORCA_NO_AUTOSTART", "false")])
                .expect("ok")
                .no_autostart
        );
    }

    #[test]
    fn unknown_arguments_and_missing_values_are_refused() {
        assert!(parse(&["--nope"], &[]).is_err());
        assert!(parse(&["--profile"], &[]).is_err());
    }

    #[test]
    fn the_page_flag_selects_a_debug_page() {
        let options = parse(&["--page", "remove-secret"], &[]).expect("parses");
        assert_eq!(options.page, Some(DebugPage::RemoveSecret));
        assert_eq!(parse(&[], &[]).expect("parses").page, None);
    }

    #[test]
    fn an_unknown_page_is_refused_with_the_valid_names() {
        let error = parse(&["--page", "nope"], &[]).expect_err("refused before any window");
        assert!(error.starts_with("unknown page \"nope\""), "{error}");
        assert!(error.ends_with(&DebugPage::names()), "{error}");
    }

    #[test]
    fn the_page_flag_needs_a_value() {
        assert_eq!(
            parse(&["--page"], &[]),
            Err("--page needs a value".to_owned())
        );
    }

    #[test]
    fn autostart_runs_the_massorca_next_to_the_desktop_and_only_when_it_exists() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let desktop = dir.path().join("massorca-desktop");
        let options = LaunchOptions::default();
        assert_eq!(options.autostart(&desktop), None, "no massorca beside it");
        std::fs::write(dir.path().join(CLI_NAME), b"").expect("a stand-in executable");
        let autostart = options.autostart(&desktop).expect("found it");
        assert_eq!(autostart.exe, dir.path().join(CLI_NAME));
        assert_eq!(autostart.args_extra, ["--repair-command"]);
        let off = LaunchOptions {
            no_autostart: true,
            ..LaunchOptions::default()
        };
        assert_eq!(off.autostart(&desktop), None);
    }
}
