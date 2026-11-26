//! Element tests of Remove host (D-HO-03, artboard `sh-confirm-destructive`), through a scripted service: each call
//! the confirm sends is answered by hand, so no window opens and no host is reached.

use std::path::PathBuf;

use super::host_remove::{
    HostRemoveFlow, LossCount, loss_rows, removal_notice, remove_body, remove_title,
};
use super::settings_tests::{Rig, draw, settings_rig};
use crate::service::{Command, HostLoss, HostRemoval, Refusal};
use gpui::{TestAppContext, VisualTestContext};
use massorca_client::DeviceRevocation;

#[test]
fn the_confirm_uses_the_design_copy_and_counts_what_leaves() {
    assert_eq!(remove_title("VPS"), "Remove host VPS?");
    assert_eq!(
        remove_body("VPS"),
        "MassOrca stops showing VPS and the projects on it in this app. Nothing on the server is deleted."
    );
    let counted = LossCount::Counted(HostLoss {
        projects: 3,
        sessions: 4,
    });
    assert_eq!(
        loss_rows("VPS", &counted),
        [
            "3 projects leave the sidebar",
            "4 past sessions on VPS are hidden here",
            "This PC loses access to VPS until it pairs again",
        ]
    );
    let one = LossCount::Counted(HostLoss {
        projects: 1,
        sessions: 1,
    });
    assert_eq!(
        loss_rows("VPS", &one)[..2],
        [
            "1 project leaves the sidebar",
            "1 past session on VPS is hidden here"
        ]
    );
    assert!(
        loss_rows("VPS", &LossCount::Uncounted)
            .iter()
            .all(|row| !row.chars().any(|c| c.is_ascii_digit())),
        "a host that did not answer gets no invented counts"
    );
}

#[test]
fn a_removal_that_left_the_device_paired_says_so() {
    let removal = HostRemoval {
        host: "VPS".into(),
        device: DeviceRevocation::NotRevoked("the host is unreachable".into()),
        leftover: None,
    };
    let notice = removal_notice(&removal).expect("the device left paired is reported");
    assert!(notice.contains("this PC stays paired on it: the host is unreachable"));
    let clean = HostRemoval {
        host: "VPS".into(),
        device: DeviceRevocation::Revoked,
        leftover: None,
    };
    assert_eq!(
        removal_notice(&clean),
        None,
        "a clean removal needs no notice"
    );
    let moved = HostRemoval {
        leftover: Some(PathBuf::from("old-hosts")),
        ..clean
    };
    assert!(removal_notice(&moved).is_some_and(|text| text.contains("old-hosts")));
}

fn flow(rig: &Rig, cx: &mut VisualTestContext) -> Option<HostRemoveFlow> {
    rig.shell
        .read_with(cx, |shell, _| shell.host_remove.clone())
}

/// Opens the confirm for `vps` and answers its count with `answer`.
fn open_counted(rig: &mut Rig, cx: &mut VisualTestContext, answer: Result<HostLoss, Refusal>) {
    rig.shell
        .update(cx, |shell, cx| shell.open_remove_host("vps".into(), cx));
    cx.run_until_parked();
    let reply = loop {
        match rig.script.next() {
            Some(Command::CountHostLoss { name, reply }) => {
                assert_eq!(name, "vps");
                break reply;
            }
            Some(_) => continue,
            None => panic!("the confirm did not ask the host for its counts"),
        }
    };
    reply.send(answer).expect("the confirm listens");
    cx.run_until_parked();
    draw(cx);
}

fn remove_reply(rig: &mut Rig) -> tokio::sync::oneshot::Sender<Result<HostRemoval, Refusal>> {
    loop {
        match rig.script.next() {
            Some(Command::RemoveHost { name, reply }) => {
                assert_eq!(name, "vps");
                break reply;
            }
            Some(_) => continue,
            None => panic!("Remove host was not sent"),
        }
    }
}

#[gpui::test]
fn remove_host_reaches_the_service_and_closes_once_the_host_is_gone(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_counted(
        &mut rig,
        cx,
        Ok(HostLoss {
            projects: 2,
            sessions: 5,
        }),
    );
    let open = flow(&rig, cx).expect("the confirm is open");
    assert_eq!(
        open.loss,
        LossCount::Counted(HostLoss {
            projects: 2,
            sessions: 5
        })
    );
    assert!(
        cx.debug_bounds("host-remove-body").is_some(),
        "the confirm is drawn with its body"
    );

    rig.shell
        .update(cx, |shell, cx| shell.confirm_remove_host(cx));
    cx.run_until_parked();
    assert!(flow(&rig, cx).is_some_and(|flow| flow.busy));
    rig.shell
        .update(cx, |shell, cx| shell.close_remove_host(cx));
    assert!(flow(&rig, cx).is_some(), "Escape is ignored while removing");
    remove_reply(&mut rig)
        .send(Ok(HostRemoval {
            host: "vps".into(),
            device: DeviceRevocation::NotRevoked("the host is unreachable".into()),
            leftover: None,
        }))
        .expect("the confirm listens");
    cx.run_until_parked();
    assert_eq!(
        flow(&rig, cx),
        None,
        "the confirm closes once the host is removed"
    );
    let notice = rig
        .shell
        .read_with(cx, |shell, _| shell.host_remove_notice.clone())
        .expect("the device left paired is reported");
    assert!(notice.contains("stays paired"), "{notice}");
}

#[gpui::test]
fn a_refused_removal_keeps_the_confirm_open_with_the_reason(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_counted(
        &mut rig,
        cx,
        Err(Refusal::unavailable("the host is unreachable")),
    );
    assert_eq!(
        flow(&rig, cx).map(|flow| flow.loss),
        Some(LossCount::Uncounted),
        "an unanswered count still lets the host be removed"
    );
    rig.shell
        .update(cx, |shell, cx| shell.confirm_remove_host(cx));
    cx.run_until_parked();
    let refused = Refusal::Failed {
        code: "host.busy".into(),
        message: "another change to the hosts is running".into(),
    };
    let sentence = refused.sentence();
    remove_reply(&mut rig)
        .send(Err(refused))
        .expect("the confirm listens");
    cx.run_until_parked();
    let open = flow(&rig, cx).expect("a refused removal keeps the confirm");
    assert!(!open.busy);
    assert_eq!(open.error, Some(sentence));
    rig.shell
        .update(cx, |shell, cx| shell.close_remove_host(cx));
    assert_eq!(flow(&rig, cx), None, "Cancel closes it once nothing runs");
}
