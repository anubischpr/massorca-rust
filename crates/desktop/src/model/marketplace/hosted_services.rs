//! The Services section (SKL-35, decision 0097): each hosted service of the picked host as a row, with what it can
//! do from here. Reads are numbered, so an answer for a host no longer picked, or an older read, is dropped.

use massorca_protocol::methods::hosted_services::{
    ServiceList, ServiceSource, ServiceState, ServiceStatus,
};

use crate::service::Refusal;

/// The one button a row shows beside its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    Connect,
    /// A connected service takes a new token.
    Replace,
    /// Linear and Jira are status-only, and GitLab needs `glab` on the host.
    None,
}

/// The fields the connect dialog asks for besides the token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectFields {
    /// The label of the base URL field, for a service that may be self-hosted.
    pub base_url: Option<&'static str>,
    /// Bitbucket's Atlassian email, for an API token.
    pub email: bool,
}

/// One service as its row shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRow {
    pub id: String,
    pub name: String,
    /// Connected, Not connected or Unknown.
    pub state: &'static str,
    pub connected: bool,
    /// The account, or else the base URL, when the host shows them to this client.
    pub address: Option<String>,
    pub detail: Option<String>,
    pub action: ServiceAction,
    pub can_disconnect: bool,
}

/// The fields of `service`'s connect dialog.
#[must_use]
pub fn connect_fields(service: &str) -> ConnectFields {
    let base_url = match service {
        "gitlab" => Some("GitLab address (self-hosted only)"),
        "azure-devops" => Some("Organization URL"),
        "gitea" => Some("Gitea server URL"),
        _ => None,
    };
    ConnectFields {
        base_url,
        email: service == "bitbucket",
    }
}

/// The row of one service.
#[must_use]
pub fn service_row(status: &ServiceStatus) -> ServiceRow {
    let connected = status.state == ServiceState::Connected;
    let status_only = matches!(status.service.as_str(), "linear" | "jira");
    let no_glab = status.service == "gitlab" && status.cli_installed == Some(false);
    let action = match (status_only || no_glab, connected) {
        (true, _) => ServiceAction::None,
        (false, true) => ServiceAction::Replace,
        (false, false) => ServiceAction::Connect,
    };
    ServiceRow {
        id: status.service.clone(),
        name: status.name.clone(),
        state: match status.state {
            ServiceState::Connected => "Connected",
            ServiceState::NotConnected => "Not connected",
            ServiceState::Unknown => "Unknown",
        },
        connected,
        address: status.account.clone().or_else(|| status.base_url.clone()),
        detail: status.detail.clone(),
        action,
        // GitLab's disconnect is refused with a sentence that says how to sign out of glab; the row shows it.
        can_disconnect: connected
            && !status_only
            && (status.source == ServiceSource::Stored || status.service == "gitlab"),
    }
}

/// What the section shows for the picked host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ServicesState {
    #[default]
    Loading,
    Ready(Vec<ServiceRow>),
    Failed(String),
}

/// The Services section's reads of the picked host. `host` is `None` for this computer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostedServicesView {
    request: u64,
    host: Option<String>,
    pub state: ServicesState,
}

impl HostedServicesView {
    /// Starts a read of `host`; its answer must carry the number returned.
    pub fn begin(&mut self, host: Option<String>) -> u64 {
        self.request += 1;
        self.host = host;
        self.state = ServicesState::Loading;
        self.request
    }

    /// The host the latest read is for.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    /// Takes the answer to read `request`; an older read's answer is dropped. Whether it was taken.
    pub fn accept(&mut self, request: u64, result: Result<ServiceList, Refusal>) -> bool {
        if request != self.request {
            return false;
        }
        self.state = match result {
            Ok(list) => ServicesState::Ready(list.services.iter().map(service_row).collect()),
            Err(refusal) => ServicesState::Failed(refusal.sentence()),
        };
        true
    }

    /// Puts a connect or disconnect answer for read `request` in place of its service's row.
    pub fn update_row(&mut self, request: u64, status: &ServiceStatus) {
        if request != self.request {
            return;
        }
        if let ServicesState::Ready(rows) = &mut self.state
            && let Some(row) = rows.iter_mut().find(|row| row.id == status.service)
        {
            *row = service_row(status);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(service: &str, state: ServiceState, source: ServiceSource) -> ServiceStatus {
        ServiceStatus {
            service: service.to_owned(),
            name: service.to_uppercase(),
            state,
            source,
            account: None,
            base_url: None,
            updated_at_ms: None,
            cli_installed: None,
            detail: None,
        }
    }

    #[test]
    fn a_stored_service_is_connected_with_its_address_and_can_be_replaced_or_disconnected() {
        let mut gitea = status("gitea", ServiceState::Connected, ServiceSource::Stored);
        gitea.base_url = Some("https://gitea.example.com".to_owned());
        let row = service_row(&gitea);
        assert_eq!(
            (
                row.state,
                row.address.as_deref(),
                row.action,
                row.can_disconnect
            ),
            (
                "Connected",
                Some("https://gitea.example.com"),
                ServiceAction::Replace,
                true
            )
        );
        let mut bitbucket = status("bitbucket", ServiceState::Connected, ServiceSource::Stored);
        bitbucket.base_url = Some("https://bitbucket.example.com".to_owned());
        bitbucket.account = Some("me@example.com".to_owned());
        assert_eq!(
            service_row(&bitbucket).address.as_deref(),
            Some("me@example.com")
        );
    }

    #[test]
    fn a_service_not_connected_offers_connect_and_no_disconnect() {
        let row = service_row(&status(
            "azure-devops",
            ServiceState::NotConnected,
            ServiceSource::None,
        ));
        assert_eq!(
            (row.state, row.action, row.can_disconnect),
            ("Not connected", ServiceAction::Connect, false)
        );
    }

    #[test]
    fn linear_and_jira_are_status_only() {
        for id in ["linear", "jira"] {
            let mut row = status(id, ServiceState::Unknown, ServiceSource::None);
            row.detail = Some("not connected from MassOrca yet".to_owned());
            let row = service_row(&row);
            assert_eq!(
                (
                    row.state,
                    row.action,
                    row.can_disconnect,
                    row.detail.as_deref()
                ),
                (
                    "Unknown",
                    ServiceAction::None,
                    false,
                    Some("not connected from MassOrca yet")
                )
            );
        }
    }

    #[test]
    fn gitlab_needs_glab_to_connect_and_offers_disconnect_when_signed_in() {
        let mut missing = status("gitlab", ServiceState::NotConnected, ServiceSource::None);
        missing.cli_installed = Some(false);
        assert_eq!(service_row(&missing).action, ServiceAction::None);
        let signed_in = status("gitlab", ServiceState::Connected, ServiceSource::External);
        let row = service_row(&signed_in);
        assert_eq!(
            (row.action, row.can_disconnect),
            (ServiceAction::Replace, true)
        );
    }

    #[test]
    fn the_dialog_asks_for_an_address_or_an_email_by_service() {
        assert_eq!(connect_fields("gitea").base_url, Some("Gitea server URL"));
        assert_eq!(
            connect_fields("azure-devops").base_url,
            Some("Organization URL")
        );
        assert_eq!(
            connect_fields("gitlab").base_url,
            Some("GitLab address (self-hosted only)")
        );
        assert_eq!(
            connect_fields("bitbucket"),
            ConnectFields {
                base_url: None,
                email: true
            }
        );
    }

    #[test]
    fn an_answer_to_an_older_read_is_dropped() {
        let mut view = HostedServicesView::default();
        let first = view.begin(None);
        let second = view.begin(Some("vps".to_owned()));
        let old = ServiceList {
            services: vec![status(
                "gitea",
                ServiceState::Connected,
                ServiceSource::Stored,
            )],
        };
        assert!(!view.accept(first, Ok(old)));
        assert_eq!(view.state, ServicesState::Loading);
        let new = ServiceList {
            services: vec![status(
                "gitea",
                ServiceState::NotConnected,
                ServiceSource::None,
            )],
        };
        assert!(view.accept(second, Ok(new)));
        assert_eq!(view.host(), Some("vps"));
        let ServicesState::Ready(rows) = &view.state else {
            panic!("ready");
        };
        assert_eq!(rows[0].state, "Not connected");
        let connected = status("gitea", ServiceState::Connected, ServiceSource::Stored);
        view.update_row(first, &connected);
        let ServicesState::Ready(rows) = &view.state else {
            panic!("ready");
        };
        assert_eq!(
            rows[0].state, "Not connected",
            "an older read's answer is dropped"
        );
        view.update_row(second, &connected);
        let ServicesState::Ready(rows) = &view.state else {
            panic!("ready");
        };
        assert_eq!(rows[0].state, "Connected");
    }
}
