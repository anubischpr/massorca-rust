//! Gallery stories for the desktop panels: notifications, extensions, secret requests, the
//! project and work panel tabs, and the browser chrome.

use super::{Story, variant};
use crate::components::browser_chrome::{BrowserChrome, SiteSecurity};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::extension_card::{ExtensionCard, ExtensionState};
use crate::components::notification_card::{
    NotificationCard, NotificationKind, NotificationStack, UpdateState,
};
use crate::components::project_sidebar_tabs::{ProjectSidebarTabs, ProjectTab, core_tabs};
use crate::components::secret_request_card::{SecretRequestCard, SecretState};
use crate::components::text_field::TextField;
use crate::components::work_panel_tabs::{WorkPanelTabs, WorkTab, WorkTabKind};
use crate::icon::IconName;
use crate::theme::ActiveTheme;
use crate::tokens::{SPACE_3, SPACE_4};
use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "Notification card",
            spec: "NotificationCard.md",
            render: notification_story,
        },
        Story {
            name: "Extension card",
            spec: "ExtensionCard.md",
            render: extension_story,
        },
        Story {
            name: "Secret request card",
            spec: "SecretRequestCard.md",
            render: secret_story,
        },
        Story {
            name: "Project sidebar tabs",
            spec: "ProjectSidebarTabs.md",
            render: project_tabs_story,
        },
        Story {
            name: "Work panel tabs",
            spec: "WorkPanelTabs.md",
            render: work_tabs_story,
        },
        Story {
            name: "Browser chrome",
            spec: "BrowserChrome.md",
            render: browser_story,
        },
    ]
}

fn extension_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let detail = |id: &'static str, state: ExtensionState| {
        ExtensionCard::new(id, "Acme Metrics", state)
            .publisher("Acme")
            .source("Marketplace")
            .version("1.4.2")
            .description("Live charts for every workspace, with alerts when a metric drifts.")
            .add(IconName::PanelLeft, "Rail item")
            .add(IconName::SquareTerminal, "3 commands")
            .on_action(|_, _, _| {})
    };
    let installed = ExtensionState::Installed {
        enabled: true,
        update: None,
    };
    div()
        .flex()
        .flex_wrap()
        .gap(px(SPACE_4))
        .child(variant(
            "Catalog, on two hosts",
            detail("ext-catalog", ExtensionState::Catalog).install_host("VPS"),
            cx,
        ))
        .child(variant(
            "Installed and enabled, with settings",
            detail("ext-enabled", installed.clone()).settings(true),
            cx,
        ))
        .child(variant(
            "Installed and disabled",
            detail(
                "ext-disabled",
                ExtensionState::Installed {
                    enabled: false,
                    update: None,
                },
            )
            .settings(true),
            cx,
        ))
        .child(variant(
            "Update available",
            detail(
                "ext-update",
                ExtensionState::Installed {
                    enabled: true,
                    update: Some("1.5.0".into()),
                },
            ),
            cx,
        ))
        .child(variant(
            "Installing, 40%",
            detail("ext-installing", ExtensionState::Installing { percent: 40 }),
            cx,
        ))
        .child(variant(
            "Failed to start",
            detail(
                "ext-failed",
                ExtensionState::Failed {
                    reason: "Exit code 1 during startup".into(),
                },
            ),
            cx,
        ))
        .child(variant(
            "Needs a newer MassOrca",
            detail(
                "ext-incompatible",
                ExtensionState::Incompatible {
                    requires: "1.6".into(),
                },
            ),
            cx,
        ))
        .child(variant(
            "Host unreachable: last known state",
            detail("ext-unreachable", installed).last_known("5 min ago"),
            cx,
        ))
        .child(variant(
            "Loading",
            detail("ext-loading", ExtensionState::Catalog).loading(true),
            cx,
        ))
        .into_any_element()
}

fn notification_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let attention = NotificationCard::new(
        "notification-attention",
        NotificationKind::Attention,
        "oppm: Run command? pnpm install",
    )
    .source("oppm")
    .age("2 min ago")
    .body("Runs in the project folder with workspace access.")
    .answer(primary("notification-approve", "Approve"))
    .answer(ghost("notification-deny", "Deny"))
    .on_dismiss(|_, _| {});

    let error = NotificationCard::new(
        "notification-error",
        NotificationKind::Error,
        "NexusCare: the run failed",
    )
    .source("NexusCare")
    .age("just now")
    .body("Exit code 1 after 42 seconds.")
    .action(primary("notification-retry", "Retry"))
    .on_dismiss(|_, _| {});

    let sign_in = NotificationCard::new(
        "notification-sign-in",
        NotificationKind::SignIn,
        "Claude was signed out",
    )
    .source("Claude")
    .age("4 min ago")
    .action(primary("notification-sign-in-button", "Sign in"))
    .on_dismiss(|_, _| {});

    let note = NotificationCard::new(
        "notification-note",
        NotificationKind::Info,
        "Acme Metrics: 2 commands added",
    )
    .source("Acme Metrics")
    .age("1 h ago")
    .on_dismiss(|_, _| {});

    let unreachable = NotificationCard::new(
        "notification-unreachable",
        NotificationKind::Attention,
        "Run command? pnpm test",
    )
    .source("oppm")
    .age("5 min ago")
    .unreachable(true)
    .action(ghost("notification-open", "Open"))
    .answer(primary("notification-unreachable-approve", "Approve"))
    .on_dismiss(|_, _| {});

    let updates = [
        UpdateState::Available {
            version: "1.4.2".into(),
        },
        UpdateState::Checking,
        UpdateState::Downloading { percent: 64 },
        UpdateState::Ready,
        UpdateState::Failed {
            reason: "offline".into(),
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, state)| {
        let mut card =
            NotificationCard::new(("notification-update", index), state.kind(), state.title())
                .source("MassOrca")
                .age("now")
                .spinner(state == UpdateState::Checking);
        if let Some(fraction) = state.progress() {
            card = card.progress(fraction);
        }
        for (action, label) in state.action_labels().iter().enumerate() {
            let id = format!("notification-update-{index}-{action}");
            let button = if action == 0 {
                primary(id, *label)
            } else {
                ghost(id, *label)
            };
            card = card.action(button);
        }
        card.on_dismiss(|_, _| {})
    })
    .collect::<Vec<_>>();
    let update_column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .children(updates.into_iter().map(IntoElement::into_any_element));

    let stack = NotificationStack::new("notification-stack")
        .cards(vec![
            NotificationCard::new("stack-1", NotificationKind::Info, "Acme: 2 commands added")
                .source("Acme Metrics")
                .age("1 h ago")
                .on_dismiss(|_, _| {}),
            NotificationCard::new(
                "stack-2",
                NotificationKind::Update,
                "MassOrca 1.4.2 is available",
            )
            .source("MassOrca")
            .age("now")
            .action(primary("stack-2-download", "Download"))
            .action(ghost("stack-2-later", "Later"))
            .on_dismiss(|_, _| {}),
            NotificationCard::new("stack-3", NotificationKind::SignIn, "Claude was signed out")
                .source("Claude")
                .age("4 min ago")
                .action(primary("stack-3-sign-in", "Sign in"))
                .on_dismiss(|_, _| {}),
            NotificationCard::new(
                "stack-4",
                NotificationKind::Error,
                "NexusCare: the run failed",
            )
            .source("NexusCare")
            .age("just now")
            .body("Exit code 1 after 42 seconds.")
            .action(primary("stack-4-retry", "Retry"))
            .on_dismiss(|_, _| {}),
            NotificationCard::new(
                "stack-5",
                NotificationKind::Attention,
                "oppm: Run command? pnpm install",
            )
            .source("oppm")
            .age("2 min ago")
            .body("Runs in the project folder with workspace access.")
            .answer(primary("stack-5-approve", "Approve"))
            .answer(ghost("stack-5-deny", "Deny"))
            .on_dismiss(|_, _| {}),
        ])
        .on_expand(|_, _| {});

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Attention: two answers and a dismiss",
            attention,
            cx,
        ))
        .child(variant("Error: a retry", error, cx))
        .child(variant("Sign-in needed: one action", sign_in, cx))
        .child(variant("Note: an extension card, name first", note, cx))
        .child(variant(
            "Update: available, checking, downloading, ready, failed",
            update_column,
            cx,
        ))
        .child(variant(
            "Host unreachable: last known state, answers disabled",
            unreachable,
            cx,
        ))
        .child(variant(
            "Stack: three in view, the rest behind +2 more",
            stack,
            cx,
        ))
        .into_any_element()
}

fn primary(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id, label)
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Compact)
}

fn ghost(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id, label)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Compact)
}

fn secret_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let asked = |id: &'static str, state: SecretState| {
        SecretRequestCard::new(id, "OPENAI_API_KEY", "Claude needs a secret", state)
            .reason("Calls the OpenAI API for this project.")
            .scope("for project oppm")
            .host("VPS")
            .on_action(|_, _, _| {})
    };
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Agent needs one: the reason is shown",
            asked("secret-pending", SecretState::Pending),
            cx,
        ))
        .child(variant(
            "User asked: no reason line",
            SecretRequestCard::new(
                "secret-user",
                "OPENAI_API_KEY",
                "Add a secret for oppm",
                SecretState::Pending,
            )
            .scope("for project oppm")
            .host("VPS")
            .on_action(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "Modal open: both buttons wait",
            asked("secret-entering", SecretState::Entering),
            cx,
        ))
        .child(variant(
            "Settled: stored, linked and declined",
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_3))
                .child(
                    SecretRequestCard::new(
                        "secret-stored",
                        "OPENAI_API_KEY",
                        "",
                        SecretState::Stored,
                    )
                    .scope("for project oppm")
                    .host("VPS"),
                )
                .child(
                    SecretRequestCard::new(
                        "secret-linked",
                        "GITHUB_TOKEN",
                        "",
                        SecretState::Linked,
                    )
                    .scope("linked from project NexusCare"),
                )
                .child(
                    SecretRequestCard::new(
                        "secret-declined",
                        "SMTP_PASSWORD",
                        "",
                        SecretState::Declined,
                    )
                    .scope("for project oppm"),
                ),
            cx,
        ))
        .child(variant(
            "Not saved: retry or decline",
            asked(
                "secret-failed",
                SecretState::Failed {
                    reason: "the vault is locked".into(),
                },
            ),
            cx,
        ))
        .child(variant(
            "Host unreachable: Add is disabled, Decline works",
            asked("secret-unreachable", SecretState::Pending).unreachable(true),
            cx,
        ))
        .into_any_element()
}

fn project_tabs_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let mut extended = core_tabs(0, 0);
    extended.push(ProjectTab::new(
        "Acme Metrics dashboard panel",
        IconName::Puzzle,
    ));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Files selected, no counts",
            ProjectSidebarTabs::new("project-tabs-plain", core_tabs(0, 0))
                .selected(0)
                .on_select(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "Source control with 3 changed files, Checks with 2 failing runs",
            ProjectSidebarTabs::new("project-tabs-counts", core_tabs(3, 2))
                .selected(3)
                .on_select(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "An extension tab; its label is cut at 24 characters",
            ProjectSidebarTabs::new("project-tabs-extension", extended)
                .selected(4)
                .on_select(|_, _, _| {}),
            cx,
        ))
        .into_any_element()
}

fn work_tabs_story(_: &mut Window, cx: &mut App) -> AnyElement {
    let kinds = vec![
        WorkTab::new(WorkTabKind::Terminal, "zsh · oppm"),
        WorkTab::new(WorkTabKind::Browser, "localhost:3000").loading(true),
        WorkTab::new(WorkTabKind::Markdown, "README.md")
            .dirty(true)
            .path("~/oppm/README.md"),
        WorkTab::new(WorkTabKind::MobileEmulator, "Pixel 8"),
    ];
    let many: Vec<WorkTab> = (0..9)
        .map(|index| WorkTab::new(WorkTabKind::Terminal, format!("shell {index}")))
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Four kinds: the second loads, the third has unsaved changes",
            WorkPanelTabs::new("work-tabs-kinds", kinds)
                .selected(1)
                .on_action(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "Overflow: the strip scrolls sideways in a 360 px panel",
            div().w(px(360.)).child(
                WorkPanelTabs::new("work-tabs-overflow", many)
                    .selected(0)
                    .on_action(|_, _, _| {}),
            ),
            cx,
        ))
        .child(variant(
            "Empty: four ways to start",
            WorkPanelTabs::new("work-tabs-empty", Vec::new()).on_action(|_, _, _| {}),
            cx,
        ))
        .into_any_element()
}

fn browser_story(window: &mut Window, cx: &mut App) -> AnyElement {
    let local = window.use_keyed_state("browser-chrome/local", cx, |_, cx| {
        TextField::new(cx).with_text("localhost:3000")
    });
    let insecure = window.use_keyed_state("browser-chrome/insecure", cx, |_, cx| {
        TextField::new(cx).with_text("http://intranet.test/reports")
    });
    let broken = window.use_keyed_state("browser-chrome/broken", cx, |_, cx| {
        TextField::new(cx).with_text("https://expired.test")
    });
    let loading = window.use_keyed_state("browser-chrome/loading", cx, |_, cx| {
        TextField::new(cx).with_text("https://docs.rs/gpui")
    });
    let agent = window.use_keyed_state("browser-chrome/agent", cx, |_, cx| {
        TextField::new(cx).with_text("https://vps.example.test/app")
    });
    let page = |height: f32| {
        div()
            .h(px(height))
            .bg(cx.theme().palette.surface.sunken)
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Local page: no indicator, back and forward available",
            BrowserChrome::new("chrome-local", local.clone(), "localhost:3000")
                .history(true, true)
                .content(page(96.))
                .on_action(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "Remote page over http: a warning, the host chip, and no forward history",
            BrowserChrome::new("chrome-insecure", insecure, "http://intranet.test/reports")
                .security(SiteSecurity::NotSecure)
                .host("VPS")
                .history(true, false)
                .content(page(96.))
                .on_action(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "Certificate problem",
            BrowserChrome::new("chrome-broken", broken, "https://expired.test")
                .security(SiteSecurity::CertificateError)
                .content(page(96.))
                .on_action(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "Loading: Stop replaces Reload, with the progress line",
            BrowserChrome::new("chrome-loading", loading, "https://docs.rs/gpui")
                .security(SiteSecurity::Secure)
                .loading(true, Some(0.4))
                .content(page(96.))
                .on_action(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "An agent drives the tab, at 125 percent zoom",
            BrowserChrome::new("chrome-agent", agent, "https://vps.example.test/app")
                .security(SiteSecurity::Secure)
                .host("VPS")
                .agent(true)
                .zoom(125)
                .content(page(96.))
                .on_action(|_, _, _| {}),
            cx,
        ))
        .child(variant(
            "Narrow tab: find and grab move into More",
            div().w(px(360.)).child(
                BrowserChrome::new("chrome-compact", local.clone(), "localhost:3000")
                    .compact(true)
                    .content(page(64.))
                    .on_action(|_, _, _| {}),
            ),
            cx,
        ))
        .into_any_element()
}
