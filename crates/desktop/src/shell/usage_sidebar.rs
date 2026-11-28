//! The usage sidebar's actions: reading one account again on demand (ACC-15), and turning usage on or off on this computer
//! through the core setting `usage.enabled` (ACC-09, ACC-10). The readings come back through the service's events.

use gpui::Context;
use massorca_protocol::methods::config::ConfigSet;
use massorca_protocol::methods::usage::{SETTING_ENABLED, UsageParams};

use super::Shell;

impl Shell {
    /// Reads one account's usage again. The service answers with its readings, which the store takes as it does an event.
    pub(super) fn refresh_usage(&mut self, kind: String, account: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |_, _| {
            let _ = service
                .usage_refresh(UsageParams {
                    kind: Some(kind),
                    account: Some(account),
                })
                .await;
        })
        .detach();
    }

    /// Turns usage on or off on this computer. The setting's change event reads the usage again, so nothing is shown from
    /// the answer.
    pub(super) fn set_usage_enabled(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |_, _| {
            let _ = service
                .config_set(ConfigSet {
                    owner: None,
                    key: SETTING_ENABLED.to_owned(),
                    value: on.into(),
                })
                .await;
        })
        .detach();
    }
}
