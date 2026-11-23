//! The Add host dialog's SSH path (D-HO-02, decision 0075): the fields, a test that checks the host before
//! anything is installed, Advanced options with the developer's program override, and the add itself. The
//! dialog says what the bridge answered and nothing before it.

use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, PathPromptOptions, Subscription, WeakEntity, div, px,
};
use massorca_ui::assets::FONT_MONO;
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogVariant, DialogWidth};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::tokens::{
    RADIUS_MD, SPACE_1, SPACE_2, SPACE_3, TYPE_BODY, TYPE_BODY_SM, TYPE_LABEL, TYPE_MONO_SM,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

use super::Shell;
use crate::model::add_host::{
    Check, HostKind, Notice, SshTarget, Tone, can_add, key_choice_at, key_labels, list_key_files,
    program_for, test_notices,
};
use crate::model::pairing::name_or_none;
use crate::service::{SshHost, bundled_program};

/// What the dialog says when the app has no service to check a host through.
const NO_SERVICE_CHECK: &str = "MassOrca is not connected to its service, so no host was checked.";

/// The port field's width in the artboard's Address and Port row.
const PORT_WIDTH: f32 = 84.0;

/// The dialog's state between frames: its fields, the test's answer, the program and the last add.
#[derive(Debug)]
pub(super) struct AddHostFlow {
    /// The segment the dialog shows: an SSH host, or the Server path.
    pub kind: HostKind,
    pub name: Entity<TextField>,
    pub address: Entity<TextField>,
    pub port: Entity<TextField>,
    pub user: Entity<TextField>,
    /// The SSH key menu: the ssh config's choice, then the key files listed when the dialog opened.
    pub key: Entity<SelectView>,
    pub keys: Vec<PathBuf>,
    pub advanced_open: bool,
    /// The developer's override, picked under Advanced options. None uses the program packaged with the app.
    pub chosen_program: Option<PathBuf>,
    pub check: Check,
    /// The target the passed test checked. Add host needs it to be the target the fields make now.
    pub checked: Option<SshTarget>,
    pub adding: bool,
    /// Why the fields do not make a target, shown under them.
    pub form_error: Option<String>,
    /// The last add's refusal and the install's log, when it did not finish.
    pub add_failure: Option<(String, Vec<String>)>,
    /// Keeps the edits of the target in view: an edit drops the result of the check that answered for the old one.
    _target_changes: Vec<Subscription>,
}

/// The target the dialog's fields make now, or why they do not.
fn typed_target(flow: &AddHostFlow, cx: &App) -> Result<SshTarget, String> {
    let key = key_choice_at(&flow.keys, flow.key.read(cx).selected_index());
    SshTarget::from_fields(
        flow.address.read(cx).text(),
        flow.port.read(cx).text(),
        flow.user.read(cx).text(),
        key.identity(),
    )
}

/// The user's `.ssh` folder, where the key menu lists the keys; empty when the system names no home folder.
pub(super) fn default_ssh_dir() -> PathBuf {
    // Tests never list the real user's keys; a test that needs keys sets a temporary folder.
    if cfg!(test) {
        return PathBuf::new();
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".ssh"))
        .unwrap_or_default()
}

impl Shell {
    /// "Add host" in Settings, Hosts: opens the dialog with empty fields.
    pub(super) fn open_add_host(&mut self, cx: &mut Context<Self>) {
        let keys = list_key_files(&self.ssh_dir);
        let key = cx.new(|cx| {
            SelectView::new(key_labels(&keys), cx)
                .label("SSH key")
                .mono()
                .layer(OverlayLayer::Dialog)
                .selected(Some(0))
        });
        let address = cx.new(|cx| TextField::new(cx).placeholder("vps.example.ts.net").mono());
        let port = cx.new(|cx| TextField::new(cx).placeholder("22").mono());
        let user = cx.new(|cx| TextField::new(cx).placeholder("deploy").mono());
        let target_changes = vec![
            cx.subscribe(&address, |shell, _, _: &TextFieldEvent, cx| {
                shell.add_host_target_changed(cx);
            }),
            cx.subscribe(&port, |shell, _, _: &TextFieldEvent, cx| {
                shell.add_host_target_changed(cx);
            }),
            cx.subscribe(&user, |shell, _, _: &TextFieldEvent, cx| {
                shell.add_host_target_changed(cx);
            }),
            cx.subscribe(&key, |shell, _, _: &SelectEvent, cx| {
                shell.add_host_target_changed(cx);
            }),
        ];
        self.add_host = Some(AddHostFlow {
            kind: HostKind::Ssh,
            name: cx.new(|cx| TextField::new(cx).placeholder("The server's name")),
            address,
            port,
            user,
            key,
            keys,
            advanced_open: false,
            chosen_program: None,
            check: Check::Idle,
            checked: None,
            adding: false,
            form_error: None,
            add_failure: None,
            _target_changes: target_changes,
        });
        cx.notify();
    }

    /// Cancel or Escape: closes the dialog unless a check or an add is under way.
    pub(super) fn close_add_host(&mut self, cx: &mut Context<Self>) {
        let busy = self
            .add_host
            .as_ref()
            .is_some_and(|flow| flow.adding || flow.check == Check::Checking);
        if self.add_host.is_some() && !busy {
            self.add_host = None;
            cx.notify();
        }
    }

    /// An edit to the target: the result of the last check described the old target, so it goes. A check still
    /// running is kept, and its answer is dropped when it arrives for a target the fields no longer make.
    fn add_host_target_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(flow) = self.add_host.as_mut() {
            if flow.check != Check::Checking {
                flow.check = Check::Idle;
                flow.checked = None;
            }
            flow.form_error = None;
            flow.add_failure = None;
            cx.notify();
        }
    }

    /// The segmented control: SSH host or Server. Server shows its note and runs no check or add from this dialog.
    pub(super) fn set_add_host_kind(&mut self, index: usize, cx: &mut Context<Self>) {
        let kind = HostKind::from_index(index);
        if let (Some(flow), Some(kind)) = (self.add_host.as_mut(), kind) {
            flow.kind = kind;
            cx.notify();
        }
    }

    /// The Advanced options disclosure.
    pub(super) fn toggle_add_host_advanced(&mut self, cx: &mut Context<Self>) {
        if let Some(flow) = self.add_host.as_mut() {
            flow.advanced_open = !flow.advanced_open;
            cx.notify();
        }
    }

    /// "Choose program file" under Advanced options: the developer's override, picked with the system's dialog.
    pub(super) fn choose_add_host_program(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                let _ = this.update(cx, |shell, cx| {
                    if let Some(flow) = shell.add_host.as_mut() {
                        flow.chosen_program = paths.into_iter().next();
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// "Test connection", or "Test again" after a result: checks the host with `uname -sm`, installing nothing.
    pub(super) fn test_add_host(&mut self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let Some(flow) = self.add_host.as_mut() else {
            return;
        };
        if flow.kind != HostKind::Ssh || flow.adding || flow.check == Check::Checking {
            return;
        }
        let target = match typed_target(flow, cx) {
            Ok(target) => target,
            Err(message) => {
                flow.form_error = Some(message);
                flow.check = Check::Idle;
                cx.notify();
                return;
            }
        };
        flow.form_error = None;
        flow.add_failure = None;
        let Some(service) = service else {
            flow.check = Check::Failed {
                sentence: NO_SERVICE_CHECK.to_owned(),
                log: Vec::new(),
            };
            cx.notify();
            return;
        };
        flow.check = Check::Checking;
        flow.checked = None;
        cx.notify();
        let checked = target.clone();
        cx.spawn(async move |this, cx| {
            let result = service.probe_ssh_host(target.target, target.ssh_args).await;
            let _ = this.update(cx, |shell, cx| {
                if let Some(flow) = shell.add_host.as_mut() {
                    if typed_target(flow, cx).ok().as_ref() != Some(&checked) {
                        // The fields moved on while the check ran: its answer is for another target.
                        flow.check = Check::Idle;
                    } else {
                        match result {
                            Ok(machine) => {
                                flow.check = Check::Passed { arch: machine.arch };
                                flow.checked = Some(checked);
                            }
                            Err(failed) => {
                                flow.check = Check::Failed {
                                    sentence: failed.refusal.sentence(),
                                    log: failed.log,
                                };
                            }
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// "Add host": installs the service with the program for the host's machine, then pairs with it.
    pub(super) fn add_add_host(&mut self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let Some(flow) = self.add_host.as_mut() else {
            return;
        };
        let Check::Passed { arch } = flow.check.clone() else {
            return;
        };
        if flow.kind != HostKind::Ssh {
            return;
        }
        let typed = typed_target(flow, cx).ok();
        let program = program_for(flow.chosen_program.clone(), bundled_program(&arch));
        let name = name_or_none(flow.name.read(cx).text());
        if !can_add(
            &flow.check,
            flow.checked.as_ref(),
            typed.as_ref(),
            program.as_deref(),
            flow.adding,
        ) {
            return;
        }
        let (Some(service), Some(checked), Some(binary)) = (service, flow.checked.clone(), program)
        else {
            return;
        };
        flow.adding = true;
        flow.add_failure = None;
        cx.notify();
        let host = SshHost {
            target: checked.target,
            ssh_args: checked.ssh_args,
            binary,
            name,
            device_name: None,
        };
        cx.spawn(async move |this, cx| {
            let result = service.add_ssh_host(host).await;
            let _ = this.update(cx, |shell, cx| {
                match result {
                    Ok(_) => shell.add_host = None,
                    Err(failed) => {
                        if let Some(flow) = shell.add_host.as_mut() {
                            flow.adding = false;
                            flow.add_failure = Some((failed.refusal.sentence(), failed.log));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// The Add host dialog (SSH path): the fields, the test in the footer, Advanced options, and Add host.
pub(super) fn add_host_dialog(flow: &AddHostFlow, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let theme = *cx.theme();
    let typed = typed_target(flow, cx).ok();
    let packaged = match &flow.check {
        Check::Passed { arch } => bundled_program(arch),
        _ => None,
    };
    let program = program_for(flow.chosen_program.clone(), packaged);
    let ready = flow.kind == HostKind::Ssh
        && can_add(
            &flow.check,
            flow.checked.as_ref(),
            typed.as_ref(),
            program.as_deref(),
            flow.adding,
        );
    let busy = flow.adding || flow.check == Check::Checking;
    let body = dialog_body(flow, program.is_some(), shell, &theme);
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog = Dialog::new("add-host", "Add host")
        .variant(DialogVariant::Confirm)
        .width(DialogWidth::Medium)
        .cancel_variant(ButtonVariant::Ghost)
        .open(true)
        .busy(busy)
        .child(body)
        .action("Add host")
        .action_icon(IconName::Server)
        .action_disabled(!ready)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.add_add_host(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_add_host(cx));
        });
    if flow.kind == HostKind::Ssh {
        dialog = dialog.leading(test_button(flow, shell));
    }
    dialog
}

/// The test at the footer's left: "Test connection" before any result, "Test again" after one (sh-host-test-failed).
fn test_button(flow: &AddHostFlow, shell: &WeakEntity<Shell>) -> AnyElement {
    let testing = flow.check == Check::Checking;
    let label = match flow.check {
        Check::Idle => "Test connection",
        Check::Checking => "Testing",
        Check::Passed { .. } | Check::Failed { .. } => "Test again",
    };
    let shell_test = shell.clone();
    div()
        .debug_selector(|| "add-host-test".to_owned())
        .child(
            Button::new("add-host-test", label)
                .variant(ButtonVariant::Secondary)
                .leading_icon(IconName::RefreshCw)
                .loading(testing)
                .disabled(testing || flow.adding)
                .on_click(move |_, cx| {
                    let _ = shell_test.update(cx, |shell, cx| shell.test_add_host(cx));
                }),
        )
        .into_any_element()
}

/// The dialog's content: the segment, the fields, what the test found, the check's log, the add's refusal, and
/// Advanced options.
fn dialog_body(
    flow: &AddHostFlow,
    has_program: bool,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> AnyElement {
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(host_kind_control(flow, shell));
    if flow.kind == HostKind::Server {
        return body.child(server_note(theme)).into_any_element();
    }
    body = body
        .child(field_row("Name", flow.name.clone().into_any_element()))
        .child(address_and_port(flow))
        .child(field_row("User", flow.user.clone().into_any_element()))
        .child(key_field(flow, theme));
    if let Some(error) = &flow.form_error {
        body = body.child(note(error, theme.palette.status.danger_fg));
    }
    for (index, notice) in test_notices(&flow.check, has_program)
        .into_iter()
        .enumerate()
    {
        body = body.child(notice_banner(format!("add-host-notice-{index}"), notice));
    }
    if let Check::Failed { log, .. } = &flow.check
        && !log.is_empty()
    {
        body = body.child(log_block(log, theme));
    }
    if let Some((sentence, log)) = &flow.add_failure {
        body = body.child(notice_banner(
            "add-host-add-failure".to_owned(),
            Notice {
                tone: Tone::Danger,
                lead: None,
                message: sentence.clone(),
                busy: false,
            },
        ));
        if !log.is_empty() {
            body = body.child(log_block(log, theme));
        }
    }
    body = body.child(advanced(flow, shell, theme));
    body.into_any_element()
}

/// The segmented control that chooses SSH host or Server (sh-add-host).
fn host_kind_control(flow: &AddHostFlow, shell: &WeakEntity<Shell>) -> AnyElement {
    let shell_kind = shell.clone();
    div()
        .debug_selector(|| "add-host-kind".into())
        .flex()
        .child(
            SegmentedControl::new(
                "add-host-kind",
                "Host type",
                vec![Segment::text("SSH host"), Segment::text("Server")],
            )
            .selected(flow.kind.index())
            .on_change(move |index, _, cx| {
                let _ = shell_kind.update(cx, |shell, cx| shell.set_add_host_kind(index, cx));
            }),
        )
        .into_any_element()
}

/// The Server path's note in place of its form: this dialog does not build that path yet (decision 0075).
fn server_note(theme: &Theme) -> AnyElement {
    div()
        .debug_selector(|| "add-host-server-note".into())
        .text_style(TYPE_BODY)
        .text_color(theme.palette.text.secondary)
        .child(
            "Pairing with a server that already runs MassOrca is not in this dialog yet. \
             Accept its offer on the Pairing page.",
        )
        .into_any_element()
}

/// Address and Port on one row: the address takes the room, and the port is 84 px wide (sh-add-host).
fn address_and_port(flow: &AddHostFlow) -> AnyElement {
    div()
        .flex()
        .gap(px(SPACE_3))
        .child(
            div()
                .debug_selector(|| "add-host-address".into())
                .flex_1()
                .min_w_0()
                .child(field_row(
                    "Address",
                    flow.address.clone().into_any_element(),
                )),
        )
        .child(
            div()
                .debug_selector(|| "add-host-port".into())
                .w(px(PORT_WIDTH))
                .child(field_row("Port", flow.port.clone().into_any_element())),
        )
        .into_any_element()
}

/// The SSH key field: the menu's choice, with the line that says what the menu lists.
fn key_field(flow: &AddHostFlow, theme: &Theme) -> AnyElement {
    let helper = div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child("Keys from ~/.ssh. A key with a passphrase must be loaded in your SSH agent first.");
    field_row(
        "SSH key",
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(flow.key.clone())
            .child(helper)
            .into_any_element(),
    )
}

/// One labelled field of the form.
fn field_row(label: &'static str, field: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(div().text_style(TYPE_LABEL).child(label))
        .child(field)
        .into_any_element()
}

/// One notice under the fields as a banner: its tone's look, its bold lead, and its message.
fn notice_banner(id: String, notice: Notice) -> AnyElement {
    let variant = match notice.tone {
        Tone::Info => BannerVariant::Info,
        Tone::Success => BannerVariant::Success,
        Tone::Warning => BannerVariant::Warning,
        Tone::Danger => BannerVariant::Danger,
    };
    let mut banner = Banner::new(id.clone(), variant, notice.message).loading(notice.busy);
    if let Some(lead) = notice.lead {
        banner = banner.title(lead);
    }
    div()
        .debug_selector(move || id)
        .child(banner)
        .into_any_element()
}

/// Advanced options: the program the developer chose, or the packaged one, and the file dialog to choose one.
fn advanced(flow: &AddHostFlow, shell: &WeakEntity<Shell>, theme: &Theme) -> AnyElement {
    let shell_toggle = shell.clone();
    let toggle = Button::new("add-host-advanced", "Advanced options")
        .variant(ButtonVariant::Ghost)
        .trailing_icon(IconName::ChevronRight)
        .on_click(move |_, cx| {
            let _ = shell_toggle.update(cx, |shell, cx| shell.toggle_add_host_advanced(cx));
        });
    let mut section = div()
        .debug_selector(|| "add-host-advanced-toggle".to_owned())
        .child(toggle)
        .into_any_element();
    if flow.advanced_open {
        let shell_choose = shell.clone();
        let choose = Button::new("add-host-choose-program", "Choose program file")
            .variant(ButtonVariant::Secondary)
            .on_click(move |_, cx| {
                let _ = shell_choose.update(cx, |shell, cx| shell.choose_add_host_program(cx));
            });
        let shown = flow.chosen_program.as_ref().map_or_else(
            || "The program packaged with this app".to_owned(),
            |path| path.display().to_string(),
        );
        section = div()
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .child(section)
            .child(
                div()
                    .px(px(SPACE_2))
                    .py(px(SPACE_1))
                    .rounded(px(RADIUS_MD))
                    .bg(theme.palette.surface.sunken)
                    .font_family(FONT_MONO)
                    .text_style(TYPE_BODY_SM)
                    .text_color(theme.palette.text.secondary)
                    .child(shown),
            )
            .child(choose)
            .into_any_element();
    }
    section
}

/// A line of text in a status colour.
fn note(text: &str, color: impl Into<gpui::Hsla>) -> AnyElement {
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(color)
        .child(text.to_owned())
        .into_any_element()
}

/// The connection log of a check or an add, one line each, in mono on a sunken surface (sh-host-test-failed).
fn log_block(log: &[String], theme: &Theme) -> AnyElement {
    div()
        .debug_selector(|| "add-host-log".to_owned())
        .flex()
        .flex_col()
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(theme.palette.border.subtle)
        .bg(theme.palette.surface.sunken)
        .text_style(TYPE_MONO_SM)
        .text_color(theme.palette.text.secondary)
        .children(log.iter().map(|line| div().child(line.clone())))
        .into_any_element()
}
