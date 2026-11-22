//! The discard dialog of the Source control panel, drawn from its flow (decision 0013).

use gpui::{App, WeakEntity};
use massorca_ui::components::dialog::Dialog;

use super::Shell;
use super::workspace_dialog_views::flow_dialog;
use crate::model::source_control_flow::DiscardFlow;

/// The discard dialog for the flow's current stage. Its action retries the discard with the waiver
/// the stage asks for; dismissing it discards nothing.
pub(super) fn discard_dialog(flow: &DiscardFlow, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    flow_dialog(
        "discard-changes",
        flow.copy(),
        flow.busy(),
        flow.error(),
        cx,
        move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.confirm_discard(cx));
        },
        move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_discard(cx));
        },
    )
}
