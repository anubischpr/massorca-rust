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
