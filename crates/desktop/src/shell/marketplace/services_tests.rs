//! Element tests of the Services section: opening it reads the picked host's services and its rows follow them, the
//! token is sent once and its field cleared at once, and a refused disconnect shows the service's sentence.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::hosted_services::{
    ServiceList, ServiceSource, ServiceState, ServiceStatus,
};

use crate::model::marketplace::{ItemKind, ServicesState};
use crate::model::navigation::MarketplacePage;
use crate::service::{Command, Refusal, ServicesCommand};
use crate::shell::service_tests::{Rig, click, draw, rig};

const TOKEN: &str = "element-test-token-77c1";

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
fn connect_sends_the_token_once_clears_its_field_and_shows_the_answer(cx: &mut TestAppContext) {
    let (mut rig, cx) = open_services(
        cx,
        vec![status(
            "gitea",
            ServiceState::NotConnected,
            ServiceSource::None,
        )],
    );
    click(cx, "button:marketplace-service-connect-gitea");
    let token = rig.shell.read_with(cx, |shell, _| {
        shell
            .marketplace_service_dialog
            .as_ref()
            .map(|dialog| dialog.token_field())
            .expect("the dialog opens")
    });
    cx.update(|window, cx| token.update(cx, |field, cx| field.set_text(TOKEN, window, cx)));
    click(cx, "dialog-action");
    let reply = match rig.script.next() {
        Some(Command::HostedServices {
            host: None,
            command: ServicesCommand::Connect { params, reply },
        }) => {
            assert_eq!(
                (params.service.as_str(), params.token.as_str()),
                ("gitea", TOKEN)
            );
            reply
        }
        _ => panic!("Connect sends the token"),
    };
    let field = rig
        .shell
        .read_with(cx, |_, cx| token.read(cx).text().to_owned());
    assert_eq!(
        field, "",
        "the token field is cleared once the token is sent"
    );
    let _ = reply.send(Ok(status(
        "gitea",
        ServiceState::Connected,
        ServiceSource::Stored,
    )));
    cx.run_until_parked();
    assert_eq!(row_state(&rig, cx, "gitea"), Some("Connected"));
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_service_dialog.is_none()),
        "a connect that worked closes the dialog"
    );
}

#[gpui::test]
fn a_refused_gitlab_disconnect_shows_the_services_sentence(cx: &mut TestAppContext) {
    let (mut rig, cx) = open_services(
        cx,
        vec![status(
            "gitlab",
            ServiceState::Connected,
            ServiceSource::External,
        )],
    );
    click(cx, "button:marketplace-service-disconnect-gitlab");
    let sentence = "MassOrca does not sign you out of GitLab: glab keeps its own sign-in.";
    match rig.script.next() {
        Some(Command::HostedServices {
            host: None,
            command: ServicesCommand::Disconnect { params, reply },
        }) => {
            assert_eq!(params.service, "gitlab");
            let _ = reply.send(Err(Refusal::Failed {
                code: "marketplace.service_unavailable".to_owned(),
                message: sentence.to_owned(),
            }));
        }
        _ => panic!("Disconnect asks the service"),
    }
    cx.run_until_parked();
    let error = rig
        .shell
        .read_with(cx, |shell, _| shell.marketplace_service_error.clone());
    assert_eq!(error, Some(("gitlab".to_owned(), sentence.to_owned())));
    assert_eq!(row_state(&rig, cx, "gitlab"), Some("Connected"));
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
