//! Gallery stories for the composer group: the model picker, the queued message and the
//! composer (ModelPicker.md, QueuedMessage.md, Composer.md).

use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, Window, div, px};

use super::{Story, variant};
use crate::components::agent_icon::AgentKind;
use crate::components::composer::{AccessMode, Attachment, Composer};
use crate::components::model_picker::{
    AgentAvailability, AgentOption, Choice, ModelList, ModelOption, ModelPicker, PickerState,
};
use crate::components::queued_message::{QueueStatus, QueuedItem, QueuedMessages, WaitReason};
use crate::icon::IconName;
use crate::tokens::{LAYOUT_MODEL_PICKER_MAX_HEIGHT, SPACE_4};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Model picker",
            spec: "ModelPicker.md",
            render: model_picker_story,
        },
        Story {
            name: "Queued message",
            spec: "QueuedMessage.md",
            render: queued_message_story,
        },
        Story {
            name: "Composer",
            spec: "Composer.md",
            render: composer_story,
        },
    ]
}

fn access_modes() -> Vec<AccessMode> {
    vec![
        AccessMode {
            name: "Ask for approval".into(),
            icon: IconName::Lock,
            bypass: false,
        },
        AccessMode {
            name: "Workspace write".into(),
            icon: IconName::Shield,
            bypass: false,
        },
        AccessMode {
            name: "Full access".into(),
            icon: IconName::TriangleAlert,
            bypass: true,
        },
    ]
}

fn composer_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let idle = window.use_keyed_state("composer-gallery/composer/idle", cx, |_, cx| {
        Composer::new(sample_state(), access_modes(), cx)
    });
    let drafted = window.use_keyed_state("composer-gallery/composer/drafted", cx, |_, cx| {
        let mut composer = Composer::new(sample_state(), access_modes(), cx);
        composer.set_attachments(
            vec![
                Attachment {
                    id: 1,
                    name: "spec.md".into(),
                },
                Attachment {
                    id: 2,
                    name: "screenshot-of-the-failing-check.png".into(),
                },
            ],
            cx,
        );
        composer.set_draft("Add a loading state to the list, then run the tests", cx);
        composer
    });
    let busy = window.use_keyed_state("composer-gallery/composer/busy", cx, |_, cx| {
        let mut composer = Composer::new(sample_state(), access_modes(), cx);
        composer.set_busy(true, cx);
        composer
    });
    let queued = window.use_keyed_state("composer-gallery/composer/queued", cx, |_, cx| {
        let mut composer = Composer::new(sample_state(), access_modes(), cx);
        composer.set_busy(true, cx);
        composer.set_draft("And then open the pull request", cx);
        composer
    });
    let disabled = window.use_keyed_state("composer-gallery/composer/disabled", cx, |_, cx| {
        let mut composer = Composer::new(sample_state(), access_modes(), cx);
        composer.set_signed_out(true, cx);
        composer
    });
    let unreachable =
        window.use_keyed_state("composer-gallery/composer/unreachable", cx, |_, cx| {
            let mut composer = Composer::new(sample_state(), access_modes(), cx);
            composer.set_host_confirmed(false, cx);
            composer.set_draft("Keep this message in the composer", cx);
            composer
        });
    let failed = window.use_keyed_state("composer-gallery/composer/failed", cx, |_, cx| {
        let mut composer = Composer::new(sample_state(), access_modes(), cx);
        composer.set_error(Some("the host refused the message".into()), cx);
        composer.set_draft("Retry this one", cx);
        composer
    });
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Idle: the primary button is disabled until there is text",
            idle,
            cx,
        ))
        .child(variant(
            "Drafted, with attachment chips: Send is ready",
            drafted,
            cx,
        ))
        .child(variant("Agent running, empty: Stop", busy, cx))
        .child(variant("Agent running, with text: Queue", queued, cx))
        .child(variant(
            "Signed out: the reason line and Sign in",
            disabled,
            cx,
        ))
        .child(variant(
            "Host unreachable: the message stays in the composer",
            unreachable,
            cx,
        ))
        .child(variant("A send failed: Retry, the draft stays", failed, cx))
        .into_any_element()
}

fn queued(id: u64, text: &str, status: QueueStatus) -> QueuedItem {
    QueuedItem {
        id,
        text: text.to_owned().into(),
        status,
    }
}

fn busy(id: u64, text: &str) -> QueuedItem {
    queued(id, text, QueueStatus::Waiting(WaitReason::AgentBusy))
}

fn dock_with(items: Vec<QueuedItem>, cx: &mut Context<QueuedMessages>) -> QueuedMessages {
    let mut dock = QueuedMessages::new();
    dock.set_items(items, cx);
    dock
}

fn queued_message_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let single = window.use_keyed_state("composer-gallery/queued/single", cx, |_, cx| {
        dock_with(vec![busy(1, "Also add a test for the empty state")], cx)
    });
    let several = window.use_keyed_state("composer-gallery/queued/several", cx, |_, cx| {
        dock_with(
            vec![
                busy(1, "Rename the helper to match the module"),
                queued(
                    2,
                    "Run the migration after the sync",
                    QueueStatus::Waiting(WaitReason::HostUnconfirmed),
                ),
                queued(
                    3,
                    "Summarise the failing check",
                    QueueStatus::Failed("the host refused the message".into()),
                ),
            ],
            cx,
        )
    });
    let collapsed = window.use_keyed_state("composer-gallery/queued/collapsed", cx, |_, cx| {
        dock_with(
            (1..=5)
                .map(|id| busy(id, &format!("Follow-up number {id}")))
                .collect(),
            cx,
        )
    });
    let confirming = window.use_keyed_state("composer-gallery/queued/confirming", cx, |_, cx| {
        let mut dock = dock_with(vec![busy(1, "Stop and check the diff first")], cx);
        dock.begin_confirm(1, cx);
        dock
    });
    let editing = window.use_keyed_state("composer-gallery/queued/editing", cx, |window, cx| {
        let mut dock = dock_with(vec![busy(1, "Read the design note again")], cx);
        dock.begin_edit(1, window, cx);
        dock
    });
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("One message", single, cx))
        .child(variant(
            "Several: waiting, host not confirming, failed",
            several,
            cx,
        ))
        .child(variant(
            "Five: collapsed to two and a more row",
            collapsed,
            cx,
        ))
        .child(variant("Send now asks inline first", confirming, cx))
        .child(variant(
            "Editing, Enter saves and Escape cancels",
            editing,
            cx,
        ))
        .into_any_element()
}

fn model(id: &str, name: &str, efforts: &[&str], default: usize, fast: bool) -> ModelOption {
    ModelOption {
        id: id.to_owned().into(),
        name: name.to_owned().into(),
        efforts: efforts
            .iter()
            .map(|level| (*level).to_owned().into())
            .collect(),
        default_effort: default,
        fast,
    }
}

/// Two agents: one with levels, Fast and a model without levels; one with a single model.
fn sample_state() -> PickerState {
    let levels = ["Low", "Medium", "High", "Max"];
    let claude = AgentOption {
        kind: AgentKind::Claude,
        availability: AgentAvailability::Ready,
        list: ModelList::Loaded,
        models: vec![
            model("opus", "Opus 5.5", &levels, 1, true),
            model("sonnet", "Sonnet 5", &levels[..3], 0, false),
            model("haiku", "Haiku 5.5", &[], 0, false),
            model("opus-old", "Opus 4.1", &levels, 2, false),
        ],
    };
    let codex = AgentOption {
        kind: AgentKind::Codex,
        availability: AgentAvailability::SignedOut,
        list: ModelList::Loaded,
        models: vec![model("gpt", "GPT-5 Codex", &["Low", "High"], 0, true)],
    };
    PickerState::new(
        vec![claude, codex],
        Choice {
            model: "opus".into(),
            effort: Some(1),
            fast: false,
        },
    )
    .with_favourites(vec!["gpt".into()])
}

fn model_picker_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let closed = window.use_keyed_state("composer-gallery/model-picker/closed", cx, |_, cx| {
        ModelPicker::new(sample_state(), cx)
    });
    let open = window.use_keyed_state("composer-gallery/model-picker/open", cx, |window, cx| {
        let mut picker = ModelPicker::new(sample_state(), cx);
        picker.open(window, cx);
        picker
    });
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant("Trigger chip, closed", closed, cx))
        .child(variant(
            "Popover open above the chip, search focused",
            // Room above the chip, so the popover opens above it as in the Composer.
            div()
                .pt(px(LAYOUT_MODEL_PICKER_MAX_HEIGHT + SPACE_4))
                .child(open),
            cx,
        ))
        .into_any_element()
}
