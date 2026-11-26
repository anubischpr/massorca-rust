//! The secret request card in a transcript (X-CH-07): the card of `secret_request_card`, with its presses sent as
//! transcript actions.

use gpui::{AnyElement, IntoElement, SharedString};

use super::{SecretRow, TranscriptAction, TranscriptActionHandler};
use crate::components::secret_request_card::SecretRequestCard;

/// Draws a secret request's card. `host_unreachable` disables Add, since nothing can be saved while the host is away.
pub fn secret_card(
    id: &SharedString,
    secret: &SecretRow,
    host_unreachable: bool,
    on_action: Option<TranscriptActionHandler>,
) -> AnyElement {
    let request_id = secret.request_id.clone();
    let mut card = SecretRequestCard::new(
        id.to_string(),
        secret.name.clone(),
        secret.title.clone(),
        secret.state.clone(),
    )
    .scope(secret.scope.clone())
    .storable(secret.storable)
    .unreachable(host_unreachable)
    .on_action(move |action, window, cx| {
        if let Some(handler) = &on_action {
            handler(
                TranscriptAction::Secret {
                    request_id: request_id.clone(),
                    action,
                },
                window,
                cx,
            );
        }
    });
    if let Some(reason) = &secret.reason {
        card = card.reason(reason.clone());
    }
    card.into_any_element()
}
