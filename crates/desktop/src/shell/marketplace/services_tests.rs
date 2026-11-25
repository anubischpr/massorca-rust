//! Element tests of the Services section: opening it reads the picked host's services and its rows follow them.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::hosted_services::{
    ServiceList, ServiceSource, ServiceState, ServiceStatus,
};

use crate::model::marketplace::{ItemKind, ServicesState};
use crate::model::navigation::MarketplacePage;
use crate::service::{Command, ServicesCommand};
use crate::shell::service_tests::{Rig, draw, rig};

fn status(service: &str, state: ServiceState, source: ServiceSource) -> ServiceStatus {
    ServiceStatus {
        service: service.to_owned(),
        name: service.to_owned(),
        state,
        source,
        account: None,
        base_url: None,
        updated_at_ms: None,
        cli_installed: Some(true),
        detail: None,
    }
}

/// The Services category open on this computer, its status read answered with `services`.
fn open_services(
    cx: &mut TestAppContext,
    services: Vec<ServiceStatus>,
) -> (Rig, &mut VisualTestContext) {
    let (mut rig, cx) = rig(cx, &[]);
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Category(ItemKind::Service), cx);
    });
    cx.run_until_parked();
    match rig.script.next() {
        Some(Command::HostedServices {
            host: None,
            command: ServicesCommand::Status { reply },
        }) => {
            let _ = reply.send(Ok(ServiceList { services }));
        }
        _ => panic!("opening Services reads this computer's services"),
    }
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

fn row_state(rig: &Rig, cx: &mut VisualTestContext, id: &str) -> Option<&'static str> {
    rig.shell
        .read_with(cx, |shell, _| match &shell.marketplace_services.state {
            ServicesState::Ready(rows) => rows.iter().find(|row| row.id == id).map(|row| row.state),
            _ => None,
        })
}

#[gpui::test]
fn opening_services_reads_this_computer_and_shows_each_state(cx: &mut TestAppContext) {
    let (rig, cx) = open_services(
        cx,
        vec![
            status("gitea", ServiceState::Connected, ServiceSource::Stored),
            status("linear", ServiceState::Unknown, ServiceSource::None),
        ],
    );
    assert_eq!(row_state(&rig, cx, "gitea"), Some("Connected"));
    assert_eq!(row_state(&rig, cx, "linear"), Some("Unknown"));
}
