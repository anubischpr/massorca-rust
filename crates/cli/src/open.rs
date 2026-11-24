//! `massorca open`: starts the MassOrca desktop app on the profile, detached, and returns while it runs.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use massorca_profile::Profile;
use massorca_protocol::codes;
use serde_json::json;

use crate::error::{CliError, exit};
use crate::output::Output;

/// Points `massorca open` at a desktop app kept elsewhere, such as a build under test.
const DESKTOP_OVERRIDE_ENV: &str = "MASSORCA_DESKTOP_EXECUTABLE";

/// Starts the desktop app on `profile`. `autostart` is off when the CLI runs with `--no-autostart`, and the app then
/// keeps the service from starting as well.
pub fn run(profile: &Profile, autostart: bool, output: Output) -> Result<(), CliError> {
    let cli = std::env::current_exe().map_err(|error| {
        CliError::new(
            "io.failed",
            format!("cannot locate the massorca executable: {error}"),
            exit::ERROR,
        )
    })?;
    let executable = desktop_executable(std::env::var_os(DESKTOP_OVERRIDE_ENV), &cli)?;
    let mut command = massorca_process::command(&executable);
    command.args(desktop_args(profile, autostart));
    if let Some(folder) = executable.parent() {
        // The app's own folder, so the caller's working directory stays free while the app runs.
        command.current_dir(folder);
    }
    let child = massorca_process::spawn_detached(&mut command).map_err(|error| {
        CliError::new(
            "io.failed",
            format!("cannot start {}: {error}", executable.display()),
            exit::ERROR,
        )
    })?;
    let result = json!({
        "pid": child.id(),
        "executable": executable.display().to_string(),
        "profile": profile.name(),
    });
    let line = format!(
        "Launched the MassOrca desktop app for profile {}.",
        profile.name()
    );
    output.success(&result, &[line]);
    Ok(())
}

/// The arguments that select `profile` the way the desktop app reads its own command line.
fn desktop_args(profile: &Profile, autostart: bool) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--profile"),
        OsString::from(profile.name()),
        OsString::from("--home"),
        profile.home().as_os_str().to_owned(),
    ];
    if !autostart {
        args.push(OsString::from("--no-autostart"));
    }
    args
}

/// The desktop app to start: the override when set, else the one beside `cli`. A missing one is refused with its code.
fn desktop_executable(override_path: Option<OsString>, cli: &Path) -> Result<PathBuf, CliError> {
    let requested = match override_path.filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => cli.with_file_name(beside_name()),
    };
    let absolute = std::path::absolute(&requested).map_err(|error| {
        CliError::new(
            "io.failed",
            format!("cannot resolve {}: {error}", requested.display()),
            exit::ERROR,
        )
    })?;
    if absolute.is_file() {
        return Ok(absolute);
    }
    Err(CliError::new(
        codes::DESKTOP_NOT_FOUND,
        format!(
            "the MassOrca desktop app is not at {}; reinstall MassOrca or set {DESKTOP_OVERRIDE_ENV}",
            absolute.display()
        ),
        exit::ERROR,
    ))
}

/// The desktop app's file name beside the CLI, with the platform's executable suffix.
fn beside_name() -> String {
    format!("massorca-desktop{}", std::env::consts::EXE_SUFFIX)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    use massorca_profile::Profile;
    use massorca_protocol::codes;

    use super::{desktop_args, desktop_executable};
    use crate::error::exit;

    /// The desktop app's file name beside the CLI on this platform.
    fn beside_name() -> String {
        format!("massorca-desktop{}", std::env::consts::EXE_SUFFIX)
    }

    fn cli_in(dir: &Path) -> PathBuf {
        dir.join(format!("massorca{}", std::env::consts::EXE_SUFFIX))
    }

    fn profile_in(home: &Path) -> Profile {
        Profile::resolve_with(Some(home.to_path_buf()), Some("dev".to_owned()), |_| None)
            .expect("the profile resolves")
    }

    #[test]
    fn the_arguments_select_the_profile_and_its_home_the_way_the_desktop_reads_them() {
        let home = tempfile::tempdir().expect("a temporary folder");
        let profile = profile_in(home.path());
        assert_eq!(
            desktop_args(&profile, true),
            [
                OsString::from("--profile"),
                OsString::from("dev"),
                OsString::from("--home"),
                profile.home().as_os_str().to_owned(),
            ]
        );
    }

    #[test]
    fn no_autostart_is_passed_on_only_when_the_cli_has_it_off() {
        let home = tempfile::tempdir().expect("a temporary folder");
        let profile = profile_in(home.path());
        assert_eq!(
            desktop_args(&profile, false).last(),
            Some(&OsString::from("--no-autostart"))
        );
        assert!(!desktop_args(&profile, true).contains(&OsString::from("--no-autostart")));
    }

    #[test]
    fn the_override_is_used_even_when_an_app_sits_beside_the_cli() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        fs::write(dir.path().join(beside_name()), b"").expect("the app beside the CLI");
        let build = dir.path().join("builds").join("desktop-build");
        fs::create_dir_all(build.parent().expect("a folder")).expect("the builds folder");
        fs::write(&build, b"").expect("the override build");
        assert_eq!(
            desktop_executable(Some(build.clone().into_os_string()), &cli_in(dir.path())),
            Ok(build)
        );
    }

    #[test]
    fn the_app_is_found_beside_the_cli_under_the_platform_name() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let beside = dir.path().join(beside_name());
        fs::write(&beside, b"").expect("the app beside the CLI");
        assert_eq!(desktop_executable(None, &cli_in(dir.path())), Ok(beside));
    }

    #[test]
    fn an_empty_override_means_no_override() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let beside = dir.path().join(beside_name());
        fs::write(&beside, b"").expect("the app beside the CLI");
        assert_eq!(
            desktop_executable(Some(OsString::new()), &cli_in(dir.path())),
            Ok(beside)
        );
    }

    #[test]
    fn a_missing_app_is_refused_with_its_stable_code_and_the_path_it_looked_at() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let error =
            desktop_executable(None, &cli_in(dir.path())).expect_err("no app beside the CLI");
        assert_eq!(error.code, codes::DESKTOP_NOT_FOUND);
        assert_eq!(error.exit_code, exit::ERROR);
        assert!(error.message.contains(&beside_name()), "{}", error.message);
    }
}
