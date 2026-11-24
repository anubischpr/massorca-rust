//! The desktop's local log as the app starts it: in the profile's logs folder, at the level `MASSORCA_LOG` names. This
//! file has one test because starting the log installs the process's logger.

use std::fs;

use massorca_desktop::local_log::{self, FILTER_ENV};
use massorca_profile::Profile;
use tempfile::TempDir;

#[test]
fn the_app_writes_its_log_into_the_profile_logs_folder_at_the_configured_level() {
    let temp = TempDir::new().expect("a temporary folder");
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a private home");
    let profile = Profile::resolve_with(Some(home), Some("default".to_owned()), |_| None)
        .expect("the profile resolves");

    let guard = local_log::start(&profile, |key| {
        (key == FILTER_ENV).then(|| "debug".to_owned())
    })
    .expect("the local log starts");
    tracing::debug!("debug line");
    tracing::trace!("trace line");
    drop(guard);

    let mut text = String::new();
    for entry in fs::read_dir(profile.logs_dir()).expect("the logs folder") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("desktop.") && name.ends_with(".log") {
            text.push_str(&fs::read_to_string(entry.path()).expect("the log file"));
        }
    }
    assert!(text.contains("debug line"), "{text}");
    assert!(!text.contains("trace line"), "{text}");
}
