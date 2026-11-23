//! The harness refuses a profile that a test wrote into before the profile folder was created private (ADR 0073).

mod harness;

use std::fs;

use harness::Sandbox;

#[test]
#[should_panic(expected = "call `Sandbox::prepare_profile` first")]
fn a_profile_written_before_its_first_command_is_refused() {
    let sandbox = Sandbox::new();
    let dir = sandbox.profile_dir("default");
    fs::create_dir_all(&dir).expect("the profile folder");
    // Not owner-only on Unix whatever the umask is; on Windows the folder inherits the home's access.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("the folder mode");
    }
    fs::write(dir.join("notes.txt"), "written too early\n").expect("the file");

    let _ = sandbox.command("default");
}

#[test]
fn a_profile_prepared_before_its_first_write_runs_normally() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    fs::write(
        sandbox.profile_dir("default").join("notes.txt"),
        "written after prepare_profile\n",
    )
    .expect("the file");

    sandbox.run("default", &["service", "info"]).success();
}

#[test]
fn a_profile_the_service_created_passes_the_guard_on_its_next_command() {
    let sandbox = Sandbox::new();
    sandbox.run("default", &["service", "info"]).success();
    sandbox.run("default", &["service", "info"]).success();
}
