use std::path::PathBuf;

use massorca_profile::{Profile, ProfileError, ensure_dir, local_socket_name};

fn no_env(_: &str) -> Option<String> {
    None
}

/// An environment lookup over fixed pairs; it owns its data, so no process state is touched.
fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let owned: Vec<(String, String)> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    move |key| {
        owned
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, value)| value.clone())
    }
}

/// The directory ADR 0004 names for this platform.
fn expected_default_home() -> PathBuf {
    let app_dir = if cfg!(target_os = "linux") {
        "massorca"
    } else {
        "MassOrca"
    };
    directories::BaseDirs::new()
        .expect("base directories")
        .data_dir()
        .join(app_dir)
}

fn resolve_in(home: &std::path::Path, name: Option<&str>) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), name.map(str::to_owned), no_env)
        .expect("profile resolves")
}

#[test]
fn default_name_is_default_when_nothing_is_set() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(resolve_in(home.path(), None).name(), "default");
}

#[test]
fn environment_names_the_profile_without_an_argument() {
    let home = tempfile::tempdir().unwrap();
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        None,
        env(&[("MASSORCA_PROFILE", "clean-test")]),
    )
    .unwrap();
    assert_eq!(profile.name(), "clean-test");
}

#[test]
fn argument_beats_environment_for_the_name() {
    let home = tempfile::tempdir().unwrap();
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        Some("from-arg".to_owned()),
        env(&[("MASSORCA_PROFILE", "from-env")]),
    )
    .unwrap();
    assert_eq!(profile.name(), "from-arg");
}

#[test]
fn empty_environment_name_counts_as_unset() {
    let home = tempfile::tempdir().unwrap();
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        None,
        env(&[("MASSORCA_PROFILE", "")]),
    )
    .unwrap();
    assert_eq!(profile.name(), "default");
}

#[test]
fn invalid_names_are_rejected_from_arguments_and_environment() {
    let home = tempfile::tempdir().unwrap();
    let too_long = "a".repeat(33);
    let invalid = [
        "Default",
        "-lead",
        "_lead",
        "a/b",
        "a b",
        "a.b",
        "..",
        "é",
        too_long.as_str(),
    ];
    for name in invalid {
        let from_arg = Profile::resolve_with(
            Some(home.path().to_path_buf()),
            Some(name.to_owned()),
            no_env,
        );
        assert!(
            matches!(from_arg, Err(ProfileError::InvalidName { .. })),
            "argument {name:?}"
        );
        let from_env = Profile::resolve_with(
            Some(home.path().to_path_buf()),
            None,
            env(&[("MASSORCA_PROFILE", name)]),
        );
        assert!(
            matches!(from_env, Err(ProfileError::InvalidName { .. })),
            "environment {name:?}"
        );
    }
    let explicit_empty =
        Profile::resolve_with(Some(home.path().to_path_buf()), Some(String::new()), no_env);
    assert!(matches!(
        explicit_empty,
        Err(ProfileError::InvalidName { .. })
    ));
}

#[test]
fn valid_names_are_accepted() {
    let home = tempfile::tempdir().unwrap();
    let longest = "a".repeat(32);
    for name in ["a", "0", "work-2", "a_b", longest.as_str()] {
        assert_eq!(resolve_in(home.path(), Some(name)).name(), name);
    }
}

#[test]
fn environment_home_is_used_and_argument_wins() {
    let env_home = tempfile::tempdir().unwrap();
    let arg_home = tempfile::tempdir().unwrap();
    let env_path = env_home.path().to_str().unwrap();
    let from_env = Profile::resolve_with(None, None, env(&[("MASSORCA_HOME", env_path)])).unwrap();
    assert_eq!(from_env.home(), env_home.path());
    let from_arg = Profile::resolve_with(
        Some(arg_home.path().to_path_buf()),
        None,
        env(&[("MASSORCA_HOME", env_path)]),
    )
    .unwrap();
    assert_eq!(from_arg.home(), arg_home.path());
}

#[test]
fn default_home_is_the_platform_data_directory() {
    let profile = Profile::resolve_with(None, None, no_env).unwrap();
    assert_eq!(profile.home(), expected_default_home().as_path());
}

#[test]
fn empty_home_environment_counts_as_unset() {
    let profile = Profile::resolve_with(None, None, env(&[("MASSORCA_HOME", "")])).unwrap();
    assert_eq!(profile.home(), expected_default_home().as_path());
}

#[cfg(windows)]
#[test]
fn default_home_is_appdata_massorca_on_windows() {
    let appdata = std::env::var_os("APPDATA").expect("APPDATA is set on Windows");
    let profile = Profile::resolve_with(None, None, no_env).unwrap();
    assert_eq!(
        profile.home(),
        PathBuf::from(appdata).join("MassOrca").as_path()
    );
}

#[test]
fn relative_home_is_made_absolute() {
    let profile =
        Profile::resolve_with(Some(PathBuf::from("relative-home")), None, no_env).unwrap();
    assert!(profile.home().is_absolute());
    assert!(profile.home().ends_with("relative-home"));
}

#[test]
fn empty_home_argument_is_rejected() {
    let error = Profile::resolve_with(Some(PathBuf::new()), None, no_env).unwrap_err();
    assert!(matches!(error, ProfileError::InvalidHome { .. }));
}

#[test]
fn layout_paths_live_under_the_profile_directory() {
    let home = tempfile::tempdir().unwrap();
    let profile = resolve_in(home.path(), Some("work"));
    let dir = home.path().join("profiles").join("work");
    assert_eq!(profile.home(), home.path());
    assert_eq!(profile.dir(), dir.as_path());
    assert_eq!(profile.lock_path(), dir.join("service.lock"));
    assert_eq!(profile.info_path(), dir.join("service.json"));
    assert_eq!(profile.store_path(), dir.join("store.sqlite3"));
    assert_eq!(profile.logs_dir(), dir.join("logs"));
    assert_eq!(profile.extensions_dir(), dir.join("extensions"));
    assert_eq!(profile.socket_path(), dir.join("service.sock"));
}

#[test]
fn ensure_dir_creates_profile_and_logs_but_not_extensions() {
    let home = tempfile::tempdir().unwrap();
    let profile = resolve_in(home.path(), Some("work"));
    ensure_dir(&profile).unwrap();
    assert!(profile.dir().is_dir());
    assert!(profile.logs_dir().is_dir());
    assert!(!profile.extensions_dir().exists());
}

#[test]
fn pipe_name_is_stable_for_a_known_profile_directory() {
    #[cfg(windows)]
    let (home, expected) = (
        r"C:\massorca-test-home",
        "massorca-default-df9aa463cd15c96b",
    );
    #[cfg(not(windows))]
    let (home, expected) = ("/massorca-test-home", "massorca-default-aeac31973ed16dc3");
    let profile = Profile::resolve_with(Some(PathBuf::from(home)), None, no_env).unwrap();
    assert_eq!(profile.pipe_name(), expected);
}

#[test]
fn different_homes_give_different_pipe_names() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let name_first = resolve_in(first.path(), None).pipe_name();
    let name_second = resolve_in(second.path(), None).pipe_name();
    assert_ne!(name_first, name_second);
    assert_eq!(name_first, resolve_in(first.path(), None).pipe_name());
    assert!(name_first.starts_with("massorca-default-"));
    assert_eq!(name_first.len(), "massorca-default-".len() + 16);
}

#[test]
fn endpoint_name_is_available_for_a_short_home() {
    let home = tempfile::tempdir().unwrap();
    let profile = resolve_in(home.path(), None);
    assert!(local_socket_name(&profile).is_ok());
}

#[cfg(unix)]
#[test]
fn unix_socket_path_must_fit_sun_path() {
    // The socket path is the home plus "/profiles/default/service.sock" (30 bytes).
    // 103 bytes is the longest path that still leaves room for the NUL terminator.
    let fits = Profile::resolve_with(
        Some(PathBuf::from(format!("/{}", "h".repeat(72)))),
        None,
        no_env,
    )
    .unwrap();
    assert_eq!(fits.socket_path().as_os_str().len(), 103);
    assert!(local_socket_name(&fits).is_ok());

    let too_long = Profile::resolve_with(
        Some(PathBuf::from(format!("/{}", "h".repeat(73)))),
        None,
        no_env,
    )
    .unwrap();
    assert_eq!(too_long.socket_path().as_os_str().len(), 104);
    let error = local_socket_name(&too_long).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("104 bytes"), "{error}");
}
