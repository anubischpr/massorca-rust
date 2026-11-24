//! Answers to the secret requests the vault page lists (decision 0071). A request is answered by storing a value in the Add
//! dialog, with its name and scope set; by linking its name to a scope that already owns it; or by declining it. The value
//! goes to the service once, as Add sends it. The request resolves at the service, and its card goes when the vault is
//! read again.

use gpui::Context;
use massorca_protocol::methods::secrets::{SecretDecline, SecretLink};

use super::Shell;
use crate::model::secret_form::refusal_sentence;
use crate::model::secret_requests::{RequestRow, request_rows};
use crate::service::Refusal;

impl Shell {
    /// Opens the Add dialog for request `id`, with its name and scope set.
    pub(super) fn open_request_secret(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.request_row(id, cx) {
            self.open_add_for_request(&row, cx);
        }
    }

    /// Links request `id`'s name in its scope to the owned secret of `owner`, which answers the request.
    pub(super) fn link_request(&mut self, id: &str, owner: &str, cx: &mut Context<Self>) {
        let (Some(service), Some(row)) = (self.service.clone(), self.request_row(id, cx)) else {
            return;
        };
        let params = SecretLink {
            scope: row.scope.clone(),
            name: row.name.clone(),
            from_scope: owner.to_owned(),
        };
        let (id, name) = (row.id, row.name);
        cx.spawn(async move |this, cx| {
            let result = service.secret_link(params).await.map(drop);
            let _ = this.update(cx, |shell, cx| {
                shell.request_answered(&id, &name, result, cx)
            });
        })
        .detach();
    }

    /// Declines request `id`. No secret changes.
    pub(super) fn decline_request(&mut self, id: &str, cx: &mut Context<Self>) {
        let (Some(service), Some(row)) = (self.service.clone(), self.request_row(id, cx)) else {
            return;
        };
        let params = SecretDecline {
            request: row.id.clone(),
        };
        let (id, name) = (row.id, row.name);
        cx.spawn(async move |this, cx| {
            let result = service.secret_decline(params).await.map(drop);
            let _ = this.update(cx, |shell, cx| {
                shell.request_answered(&id, &name, result, cx)
            });
        })
        .detach();
    }

    fn request_row(&self, id: &str, cx: &Context<Self>) -> Option<RequestRow> {
        let state = self.store.read(cx).state();
        request_rows(&state.secrets, &state.projects)
            .into_iter()
            .find(|row| row.id == id)
    }

    /// The service's answer to an action on request `id`: a refusal shows on that request's card, success clears it.
    fn request_answered(
        &mut self,
        id: &str,
        name: &str,
        result: Result<(), Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.request_notice = result
            .err()
            .map(|refusal| (id.to_owned(), refusal_sentence(&refusal, name)));
        cx.notify();
    }
}
