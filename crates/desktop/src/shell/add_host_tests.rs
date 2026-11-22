//! Element tests of the Add host dialog's SSH path: the host is checked before anything is installed, and an
//! add goes out only for the target that passed, with a program. Each call the dialog sends is answered by
//! hand, so no window opens and no service runs.

use std::path::PathBuf;

use super::settings_tests::{Rig, click, draw, paired_host, settings_rig};
use crate::model::add_host::Check;
use crate::service::{Command, Refusal, SshAdded, SshFailed};
use gpui::{TestAppContext, VisualTestContext};
use massorca_core_remote::bootstrap::RemoteMachine;

/// Opens Settings, Hosts and the dialog from its button.
fn open_dialog(cx: &mut VisualTestContext) {
    click(cx, "settings-nav-Hosts");
    click(cx, "settings-add-host");
    draw(cx);
}

/// Types the address into the dialog's address field.
fn type_address(rig: &Rig, cx: &mut VisualTestContext, address: &str) {
    let field = rig.shell.read_with(cx, |shell, _| {
        shell
            .add_host
            .as_ref()
            .expect("the dialog is open")
            .address
            .clone()
    });
    field.update_in(cx, |field, window, cx| field.set_text(address, window, cx));
}

fn check_of(rig: &Rig, cx: &mut VisualTestContext) -> Check {
    rig.shell.read_with(cx, |shell, _| {
        shell
            .add_host
            .as_ref()
            .expect("the dialog is open")
            .check
            .clone()
    })
}

#[gpui::test]
fn the_host_is_checked_before_anything_is_added(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_dialog(cx);
    type_address(&rig, cx, "100.64.0.10");
    click(cx, "add-host-test");
    let reply = loop {
        match rig.script.next() {
            Some(Command::ProbeSshHost {
                target,
                ssh_args,
                reply,
            }) => {
                assert_eq!(target, "100.64.0.10");
                assert!(ssh_args.is_empty(), "no port typed: no ssh option");
                break reply;
            }
            Some(_) => continue,
            None => panic!("the host was not checked"),
        }
    };
    reply
        .send(Ok(RemoteMachine {
            os: "Linux".to_owned(),
            arch: "x86_64".to_owned(),
        }))
        .expect("the dialog listens");
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        check_of(&rig, cx),
        Check::Passed {
            arch: "x86_64".to_owned()
        }
    );
}

#[gpui::test]
fn a_host_that_cannot_be_checked_says_why_and_keeps_the_log(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_dialog(cx);
    type_address(&rig, cx, "100.64.0.10");
    click(cx, "add-host-test");
    let reply = loop {
        match rig.script.next() {
            Some(Command::ProbeSshHost { reply, .. }) => break reply,
            Some(_) => continue,
            None => panic!("the host was not checked"),
        }
    };
    let refusal = Refusal::Failed {
        code: "host.unreachable".to_owned(),
        message: "the host is unreachable: timed out".to_owned(),
    };
    let sentence = refusal.sentence();
    reply
        .send(Err(SshFailed {
            refusal,
            log: vec!["could not check 100.64.0.10".to_owned()],
        }))
        .expect("the dialog listens");
    cx.run_until_parked();
    assert_eq!(
        check_of(&rig, cx),
        Check::Failed {
            sentence,
            log: vec!["could not check 100.64.0.10".to_owned()],
        }
    );
}

#[gpui::test]
fn adding_sends_the_checked_host_only_with_a_program_for_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_dialog(cx);
    type_address(&rig, cx, "100.64.0.10");
    click(cx, "add-host-test");
    let reply = loop {
        match rig.script.next() {
            Some(Command::ProbeSshHost { reply, .. }) => break reply,
            Some(_) => continue,
            None => panic!("the host was not checked"),
        }
    };
    reply
        .send(Ok(RemoteMachine {
            os: "Linux".to_owned(),
            arch: "x86_64".to_owned(),
        }))
        .expect("the dialog listens");
    cx.run_until_parked();

    // No program is packaged next to the test binary, so nothing is installed yet.
    rig.shell.update(cx, |shell, cx| shell.add_add_host(cx));
    cx.run_until_parked();
    while let Some(command) = rig.script.next() {
        assert!(
            !matches!(command, Command::AddSshHost { .. }),
            "nothing is installed without a program"
        );
    }

    rig.shell.update(cx, |shell, _| {
        shell
            .add_host
            .as_mut()
            .expect("the dialog is open")
            .chosen_program = Some(PathBuf::from("massorca-linux-test"));
    });
    rig.shell.update(cx, |shell, cx| shell.add_add_host(cx));
    cx.run_until_parked();
    let (host, reply) = loop {
        match rig.script.next() {
            Some(Command::AddSshHost { host, reply }) => break (host, reply),
            Some(_) => continue,
            None => panic!("the host was not added"),
        }
    };
    assert_eq!(host.target, "100.64.0.10");
    assert_eq!(host.binary, PathBuf::from("massorca-linux-test"));
    assert_eq!(
        host.name, None,
        "no name typed: the server's own name is used"
    );
    reply
        .send(Ok(SshAdded::Paired(paired_host("vps"))))
        .expect("the dialog listens");
    cx.run_until_parked();
    draw(cx);
    assert!(
        rig.shell.read_with(cx, |shell, _| shell.add_host.is_none()),
        "a host that was added closes the dialog"
    );
}

/// Makes an `.ssh` folder with two keys and their public halves, and points the dialog's key list at it.
fn ssh_folder(rig: &Rig, cx: &mut VisualTestContext) -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for name in ["id_ed25519", "id_ed25519.pub", "work_key", "work_key.pub"] {
        std::fs::write(folder.path().join(name), b"").expect("a key file");
    }
    rig.shell
        .update(cx, |shell, _| shell.ssh_dir = folder.path().to_path_buf());
    folder
}

/// Chooses menu row `index` of the dialog's SSH key field: 0 is the ssh config's choice, then each key.
fn choose_key(rig: &Rig, cx: &mut VisualTestContext, index: usize) {
    let key = rig.shell.read_with(cx, |shell, _| {
        shell
            .add_host
            .as_ref()
            .expect("the dialog is open")
            .key
            .clone()
    });
    key.update(cx, |select, cx| select.set_selected(Some(index), cx));
}

#[gpui::test]
fn the_chosen_key_is_passed_to_the_check_and_the_install_as_identity(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    let folder = ssh_folder(&rig, cx);
    open_dialog(cx);
    choose_key(&rig, cx, 2);
    type_address(&rig, cx, "100.64.0.10");
    click(cx, "add-host-test");
    let key_path = folder
        .path()
        .join("work_key")
        .to_string_lossy()
        .into_owned();
    let reply = loop {
        match rig.script.next() {
            Some(Command::ProbeSshHost {
                ssh_args, reply, ..
            }) => {
                assert_eq!(
                    ssh_args,
                    vec!["-i".to_owned(), key_path.clone()],
                    "the check uses the chosen key"
                );
                break reply;
            }
            Some(_) => continue,
            None => panic!("the host was not checked"),
        }
    };
    reply
        .send(Ok(RemoteMachine {
            os: "Linux".to_owned(),
            arch: "x86_64".to_owned(),
        }))
        .expect("the dialog listens");
    cx.run_until_parked();
    rig.shell.update(cx, |shell, _| {
        shell
            .add_host
            .as_mut()
            .expect("the dialog is open")
            .chosen_program = Some(PathBuf::from("massorca-linux-test"));
    });
    rig.shell.update(cx, |shell, cx| shell.add_add_host(cx));
    cx.run_until_parked();
    let host = loop {
        match rig.script.next() {
            Some(Command::AddSshHost { host, .. }) => break host,
            Some(_) => continue,
            None => panic!("the host was not added"),
        }
    };
    assert_eq!(
        host.ssh_args,
        vec!["-i".to_owned(), key_path],
        "the install uses the key the check used"
    );
}

#[gpui::test]
fn a_key_changed_after_the_check_needs_a_new_check_before_adding(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    let _folder = ssh_folder(&rig, cx);
    open_dialog(cx);
    type_address(&rig, cx, "100.64.0.10");
    click(cx, "add-host-test");
    let reply = loop {
        match rig.script.next() {
            Some(Command::ProbeSshHost { reply, .. }) => break reply,
            Some(_) => continue,
            None => panic!("the host was not checked"),
        }
    };
    reply
        .send(Ok(RemoteMachine {
            os: "Linux".to_owned(),
            arch: "x86_64".to_owned(),
        }))
        .expect("the dialog listens");
    cx.run_until_parked();
    choose_key(&rig, cx, 2);
    rig.shell.update(cx, |shell, _| {
        shell
            .add_host
            .as_mut()
            .expect("the dialog is open")
            .chosen_program = Some(PathBuf::from("massorca-linux-test"));
    });
    rig.shell.update(cx, |shell, cx| shell.add_add_host(cx));
    cx.run_until_parked();
    while let Some(command) = rig.script.next() {
        assert!(
            !matches!(command, Command::AddSshHost { .. }),
            "the fields name a key the check did not test"
        );
    }
}

#[gpui::test]
fn the_address_and_the_port_share_one_row(cx: &mut TestAppContext) {
    let (_rig, cx) = settings_rig(cx);
    open_dialog(cx);
    let address = cx
        .debug_bounds("add-host-address")
        .expect("the address field is drawn");
    let port = cx
        .debug_bounds("add-host-port")
        .expect("the port field is drawn");
    assert!(
        address.origin.x < port.origin.x,
        "the port is right of the address"
    );
    assert!(
        (f32::from(address.origin.y) - f32::from(port.origin.y)).abs() < 0.5,
        "the address and the port share one row"
    );
    assert_eq!(
        f32::from(port.size.width),
        84.0,
        "the port is 84 px wide, as in sh-add-host"
    );
}

#[gpui::test]
fn the_segments_switch_between_the_ssh_form_and_the_server_note(cx: &mut TestAppContext) {
    let (_rig, cx) = settings_rig(cx);
    open_dialog(cx);
    assert!(
        cx.debug_bounds("add-host-address").is_some(),
        "SSH host is the first segment, so its form shows"
    );
    click(cx, "segment-1");
    draw(cx);
    assert!(
        cx.debug_bounds("add-host-server-note").is_some(),
        "Server shows the note that its path is not built here"
    );
    assert!(
        cx.debug_bounds("add-host-address").is_none(),
        "the SSH fields do not show for Server"
    );
    assert!(
        cx.debug_bounds("add-host-test").is_none(),
        "no check runs for Server"
    );
    click(cx, "segment-0");
    draw(cx);
    assert!(
        cx.debug_bounds("add-host-address").is_some(),
        "back on SSH host, its form returns"
    );
}

#[gpui::test]
fn the_server_segment_adds_nothing_even_after_an_ssh_check_passed(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_dialog(cx);
    type_address(&rig, cx, "100.64.0.10");
    click(cx, "add-host-test");
    let reply = loop {
        match rig.script.next() {
            Some(Command::ProbeSshHost { reply, .. }) => break reply,
            Some(_) => continue,
            None => panic!("the host was not checked"),
        }
    };
    reply
        .send(Ok(RemoteMachine {
            os: "Linux".to_owned(),
            arch: "x86_64".to_owned(),
        }))
        .expect("the dialog listens");
    cx.run_until_parked();
    rig.shell.update(cx, |shell, _| {
        shell
            .add_host
            .as_mut()
            .expect("the dialog is open")
            .chosen_program = Some(PathBuf::from("massorca-linux-test"));
    });
    rig.shell
        .update(cx, |shell, cx| shell.set_add_host_kind(1, cx));
    rig.shell.update(cx, |shell, cx| shell.add_add_host(cx));
    cx.run_until_parked();
    while let Some(command) = rig.script.next() {
        assert!(
            !matches!(command, Command::AddSshHost { .. }),
            "the Server segment does not install over SSH"
        );
    }
}
