//! Element tests of the Settings page: the sections, the search, and the workspace folder edited
//! through a scripted service. Each call the page sends is answered by hand, so no window opens and
//! no service runs.

use super::settings::host_word;
use super::*;
use crate::model::navigation::{MarketplacePage, Page};
use crate::model::secret_form::FormStatus;
use crate::model::settings::FolderStatus;
use crate::model::settings_sections::SettingsSection;
use crate::model::teams::{TeamsSource, UnavailableTeams};
use crate::service::fixtures::{project, workspace};
use crate::service::{
    Command, ConnectionState, HostLink, Refusal, Script, SecretCommand, SecretsUpdate, StoreUpdate,
};
use gpui::{
    AppContext, ClipboardItem, Entity, Modifiers, TestAppContext, VisualTestContext, point, px,
};
use massorca_client::HostRecord;
use massorca_protocol::codes;
use massorca_protocol::methods::config::{ConfigList, ConfigValue};
use massorca_protocol::methods::secrets::{SecretEntry, SecretRemoved, SecretSet, SecretStatus};
use massorca_ui::components::text_field::TextField;
use massorca_ui::{Appearance, Theme};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// Full folder paths: a rooted path without a drive is not absolute on Windows.
const STORED: &str = if cfg!(windows) {
    r"D:\srv\stored"
} else {
    "/srv/stored"
};
const DEFAULT: &str = if cfg!(windows) {
    r"C:\Users\me\MassOrca\workspaces"
} else {
    "/home/me/massorca/workspaces"
};
const NEXT: &str = if cfg!(windows) {
    r"D:\srv\next"
} else {
    "/srv/next"
};
const REFUSED: &str = if cfg!(windows) {
    r"D:\srv\refused"
} else {
    "/srv/refused"
};
const ABANDONED: &str = if cfg!(windows) {
    r"D:\srv\abandoned"
} else {
    "/srv/abandoned"
};

pub(super) struct Rig {
    pub(super) shell: Entity<Shell>,
    pub(super) _updates: UnboundedSender<StoreUpdate>,
    pub(super) script: Script,
}

pub(super) fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

pub(super) fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

pub(super) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    draw(cx);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
}

/// A connected shell on a scripted service with one project, opened on the Settings page.
pub(super) fn settings_rig(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    setup(cx);
    let (updates, rx) = unbounded_channel();
    let (handle, script) = ServiceHandle::scripted();
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        Shell::with_store(
            store,
            Some(handle),
            None,
            UnavailableTeams.teams(),
            Box::new(UnavailableMarketplace),
            window,
            cx,
        )
    });
    updates
        .send(StoreUpdate::Connection(ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        }))
        .expect("the store listens");
    updates
        .send(StoreUpdate::Resync {
            projects: vec![project("p1", "alpha")],
            workspaces: vec![workspace("w1", "p1", "fix-login", 1)],
        })
        .expect("the store listens");
    cx.run_until_parked();
    shell.update(cx, |shell, _| shell.model.open_page(Page::Settings));
    draw(cx);
    let mut rig = Rig {
        shell,
        _updates: updates,
        script,
    };
    answer_config_list(&mut rig, cx, STORED);
    (rig, cx)
}

fn config_value(path: &str, is_default: bool) -> ConfigValue {
    ConfigValue {
        owner: "core".into(),
        key: "workspaces.default_root".into(),
        value: path.to_owned().into(),
        default: DEFAULT.to_owned().into(),
        is_default,
        ..Default::default()
    }
}

/// Answers the `config.list` the page sends once the service is connected.
fn answer_config_list(rig: &mut Rig, cx: &mut VisualTestContext, stored: &str) {
    cx.run_until_parked();
    let reply = loop {
        match rig.script.next() {
            // Opening Settings also asks for the paired hosts, sent before the folder read is.
            Some(Command::RefreshHosts) => {}
            Some(Command::ListConfig { reply }) => break reply,
            _ => panic!("a config.list call was expected"),
        }
    };
    let values = vec![config_value(stored, stored == DEFAULT)];
    reply
        .send(Ok(ConfigList { values }))
        .expect("the page listens");
    cx.run_until_parked();
    draw(cx);
}

fn folder(rig: &Rig, cx: &mut VisualTestContext) -> crate::model::settings::FolderSetting {
    rig.shell
        .read_with(cx, |shell, _| shell.settings.folder.clone())
}

/// Clicks into the folder field, selects its text and types the new folder.
fn type_folder(cx: &mut VisualTestContext, text: &str) {
    click(cx, "settings-folder");
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input(text);
    draw(cx);
}

#[gpui::test]
fn the_folder_setting_is_read_from_the_service_when_the_page_opens(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    let folder = folder(&rig, cx);
    assert_eq!(folder.draft, STORED);
    assert!(!folder.dirty(), "the draft is the stored folder");
    assert!(
        folder.modified(),
        "a folder other than the default is in effect"
    );
}

#[gpui::test]
fn a_chosen_section_shows_its_rows_and_the_navigation_moves_between_sections(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = settings_rig(cx);
    assert_eq!(
        rig.shell.read_with(cx, |shell, _| shell.settings.section),
        SettingsSection::General
    );
    click(cx, "settings-nav-Appearance");
    assert_eq!(
        rig.shell.read_with(cx, |shell, _| shell.settings.section),
        SettingsSection::Appearance
    );
}

#[gpui::test]
fn a_folder_typed_and_entered_is_sent_and_only_the_service_answer_says_saved(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = settings_rig(cx);
    type_folder(cx, NEXT);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    let reply = match rig.script.next() {
        Some(Command::SetConfig { params, reply }) => {
            assert_eq!(params.key, "workspaces.default_root");
            assert_eq!(params.value.as_str(), Some(NEXT));
            reply
        }
        _ => panic!("a config.set call was expected"),
    };
    assert_eq!(
        folder(&rig, cx).status,
        Some(FolderStatus::Saving),
        "saving until the service answers"
    );

    reply
        .send(Ok(config_value(NEXT, false)))
        .expect("the page listens");
    cx.run_until_parked();
    let folder = folder(&rig, cx);
    assert_eq!(folder.status, Some(FolderStatus::Saved));
    assert_eq!(folder.stored.map(|value| value.path), Some(NEXT.to_owned()));
}

#[gpui::test]
fn a_relative_folder_is_not_sent_and_the_row_says_why(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    type_folder(cx, "workspaces");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(rig.script.next().is_none(), "nothing reaches the service");
    let folder = folder(&rig, cx);
    assert_eq!(
        folder.status,
        Some(FolderStatus::Failed("Enter a full folder path.".into()))
    );
    assert_eq!(folder.draft, "workspaces", "the typed text stays");
}

#[gpui::test]
fn a_refused_folder_keeps_the_stored_one_in_effect(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    type_folder(cx, REFUSED);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::SetConfig { reply, .. }) => reply,
        _ => panic!("a config.set call was expected"),
    };
    reply
        .send(Err(Refusal::Failed {
            code: codes::CONFIG_INVALID_VALUE.into(),
            message: "workspaces.default_root expects an absolute path".into(),
        }))
        .expect("the page listens");
    cx.run_until_parked();
    let folder = folder(&rig, cx);
    assert_eq!(
        folder.status,
        Some(FolderStatus::Failed("Enter a full folder path.".into()))
    );
    assert_eq!(
        folder.stored.map(|value| value.path),
        Some(STORED.to_owned())
    );
}

#[gpui::test]
fn escape_puts_the_stored_folder_back_without_sending_anything(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    type_folder(cx, ABANDONED);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(rig.script.next().is_none());
    assert_eq!(folder(&rig, cx).draft, STORED);
}

#[gpui::test]
fn the_setup_checklist_opens_as_a_full_page_from_settings_general(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    click(cx, "settings-setup-open");
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.model().navigation().page),
        Page::Setup
    );
    draw(cx);
}

#[gpui::test]
fn a_search_query_is_the_text_the_field_settled_on(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    rig.shell.update_in(cx, |shell, window, cx| {
        shell
            .settings_search
            .update(cx, |field, cx| field.set_query("theme", window, cx));
    });
    cx.run_until_parked();
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.settings.query.clone()),
        "theme"
    );
    // A search does not touch the folder.
    assert_eq!(folder(&rig, cx).draft, STORED);
}

#[test]
fn this_pc_reads_the_store_connection_in_words() {
    assert_eq!(host_word(&ConnectionState::Connecting), "Connecting");
    assert_eq!(
        host_word(&ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        }),
        "Connected"
    );
    assert_eq!(
        host_word(&ConnectionState::Reconnecting {
            attempt: 2,
            retry_at: std::time::Instant::now(),
        }),
        "Reconnecting"
    );
    assert_eq!(
        host_word(&ConnectionState::Unreachable {
            reason: "the link is down".into(),
        }),
        "Unreachable"
    );
}

/// An owned name in `scope`, rotated long ago and never used.
fn owned_secret(scope: &str, name: &str) -> SecretEntry {
    SecretEntry {
        scope: scope.into(),
        name: name.into(),
        owner_scope: scope.into(),
        linked: false,
        linked_scopes: Vec::new(),
        created_at_ms: 1,
        rotated_at_ms: 1,
        used_at_ms: None,
    }
}

#[gpui::test]
fn the_vault_section_draws_each_name_under_its_scope(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    let status = SecretStatus {
        protection: "file".into(),
        state: "ready".into(),
        count: 1,
        name_clashes: Vec::new(),
    };
    rig._updates
        .send(StoreUpdate::Secrets(SecretsUpdate::Status(Ok(status))))
        .expect("the store listens");
    rig._updates
        .send(StoreUpdate::Secrets(SecretsUpdate::List(Ok(vec![
            owned_secret("project:p1", "OPENAI_API_KEY"),
        ]))))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "settings-nav-Vault");
    assert!(
        cx.debug_bounds("settings-vault-project:p1-OPENAI_API_KEY")
            .is_some(),
        "the name is drawn under its project"
    );
}

/// Opens Add and types `name` into the name field.
fn open_add_named(rig: &mut Rig, cx: &mut VisualTestContext, name: &str) {
    rig.shell.update(cx, |shell, cx| shell.open_add_secret(cx));
    let field = rig.shell.read_with(cx, |shell, _| {
        shell
            .vault_dialog
            .as_ref()
            .and_then(|dialog| dialog.name_field.clone())
            .expect("the name field is open")
    });
    field.update_in(cx, |field, window, cx| field.set_text(name, window, cx));
}

/// The masked value field of the open dialog.
fn value_of_dialog(rig: &Rig, cx: &mut VisualTestContext) -> Entity<TextField> {
    rig.shell.read_with(cx, |shell, _| {
        shell
            .vault_dialog
            .as_ref()
            .map(|dialog| dialog.value_field.clone())
            .expect("the value field is open")
    })
}

/// Types `text` into the value field as the keyboard does: the field takes focus, then the keys arrive.
fn type_value(rig: &Rig, cx: &mut VisualTestContext, text: &str) {
    let field = value_of_dialog(rig, cx);
    field.update_in(cx, |field, window, cx| field.focus(window, cx));
    draw(cx);
    cx.simulate_input(text);
}

/// Opens Add, types `name`, pastes `value` from the clipboard and saves: what the page sends, and nothing more.
fn add_and_save(rig: &mut Rig, cx: &mut VisualTestContext, name: &str, value: &str) {
    open_add_named(rig, cx, name);
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(value.to_owned())));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.paste_secret_value(window, cx));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.save_secret(window, cx));
    cx.run_until_parked();
}

/// Opens Add, types `name`, types `value` into the masked field and saves.
fn type_and_save(rig: &mut Rig, cx: &mut VisualTestContext, name: &str, value: &str) {
    open_add_named(rig, cx, name);
    type_value(rig, cx, value);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.save_secret(window, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn a_pasted_value_is_sent_once_and_the_dialog_closes_when_the_vault_stores_it(
    cx: &mut TestAppContext,
) {
    const VALUE: &str = "settings-test-value-7b2d";
    let (mut rig, cx) = settings_rig(cx);
    add_and_save(&mut rig, cx, "OPENAI_API_KEY", VALUE);
    match rig.script.next() {
        Some(Command::Secret(SecretCommand::Set { params, reply })) => {
            assert_eq!(
                (params.scope.as_str(), params.name.as_str(), params.replace),
                ("project:p1", "OPENAI_API_KEY", false),
                "the first project is preselected and an add never replaces"
            );
            assert!(params.value == VALUE, "the pasted value is what is sent");
            reply
                .send(Ok(owned_secret("project:p1", "OPENAI_API_KEY")))
                .expect("the page listens");
        }
        _ => panic!("a secret.set was expected"),
    }
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.vault_dialog.is_none()),
        "the dialog closes once the vault stores the secret"
    );
}

#[gpui::test]
fn an_empty_clipboard_is_refused_and_pastes_nothing(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_add_named(&mut rig, cx, "OPENAI_API_KEY");
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(String::new())));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.paste_secret_value(window, cx));
    cx.run_until_parked();
    assert!(
        value_of_dialog(&rig, cx).read_with(cx, |field, _| field.text().is_empty()),
        "an empty clipboard pastes nothing"
    );
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell.vault_dialog.as_ref().expect("the dialog stays open");
        assert!(
            matches!(dialog.form.status, FormStatus::Refused(_)),
            "the refusal says why"
        );
    });
    assert!(rig.script.next().is_none(), "nothing reaches the service");
}

#[gpui::test]
fn a_delete_names_its_links_and_removes_the_secret_only_after_the_confirm(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    let status = SecretStatus {
        protection: "file".into(),
        state: "ready".into(),
        count: 1,
        name_clashes: Vec::new(),
    };
    let mut shared = owned_secret("global", "SHARED");
    shared.linked_scopes = vec!["project:p1".to_owned()];
    rig._updates
        .send(StoreUpdate::Secrets(SecretsUpdate::Status(Ok(status))))
        .expect("the store listens");
    rig._updates
        .send(StoreUpdate::Secrets(SecretsUpdate::List(Ok(vec![shared]))))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.update(cx, |shell, cx| {
        shell.open_remove_secret(
            "global".to_owned(),
            "SHARED".to_owned(),
            vec!["project:p1".to_owned()],
            cx,
        )
    });
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "nothing is sent before the confirm"
    );

    rig.shell
        .update(cx, |shell, cx| shell.confirm_remove_secret(cx));
    cx.run_until_parked();
    match rig.script.next() {
        Some(Command::Secret(SecretCommand::Remove { params, reply })) => {
            assert_eq!(
                (
                    params.scope.as_str(),
                    params.name.as_str(),
                    params.include_links
                ),
                ("global", "SHARED", true),
                "the confirm named the link, so the link goes too"
            );
            reply
                .send(Ok(SecretRemoved {
                    scope: "global".to_owned(),
                    name: "SHARED".to_owned(),
                    links_removed: vec!["project:p1".to_owned()],
                }))
                .expect("the page listens");
        }
        _ => panic!("a secret.remove was expected"),
    }
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.vault_remove.is_none()),
        "the dialog closes once the secret is removed"
    );
}

/// Takes the `secret.set` the page sent next, or fails the test.
fn next_set(
    rig: &mut Rig,
) -> (
    SecretSet,
    tokio::sync::oneshot::Sender<Result<SecretEntry, Refusal>>,
) {
    match rig.script.next() {
        Some(Command::Secret(SecretCommand::Set { params, reply })) => (params, reply),
        _ => panic!("a secret.set was expected"),
    }
}

/// Saves the open dialog, then answers the request with a refusal that names no value.
fn save_and_refuse(rig: &mut Rig, cx: &mut VisualTestContext) {
    rig.shell
        .update_in(cx, |shell, window, cx| shell.save_secret(window, cx));
    cx.run_until_parked();
    let (_, reply) = next_set(rig);
    reply
        .send(Err(Refusal::Failed {
            code: codes::SECRET_NAME_IN_USE.to_owned(),
            message: "OPENAI_API_KEY already exists in project:p1".to_owned(),
        }))
        .expect("the page listens");
    cx.run_until_parked();
}

#[gpui::test]
fn a_typed_value_stays_in_the_locked_field_while_it_is_sent(cx: &mut TestAppContext) {
    const VALUE: &str = "settings-test-value-3a8c";
    let (mut rig, cx) = settings_rig(cx);
    type_and_save(&mut rig, cx, "OPENAI_API_KEY", VALUE);
    let field = value_of_dialog(&rig, cx);
    type_value(&rig, cx, "-typed-over");
    assert!(
        field.read_with(cx, |field, _| field.text() == VALUE),
        "the field is locked while the save is in flight"
    );
    let (params, reply) = next_set(&mut rig);
    assert!(params.value == VALUE, "the typed value is what is sent");
    reply
        .send(Ok(owned_secret("project:p1", "OPENAI_API_KEY")))
        .expect("the page listens");
    cx.run_until_parked();
    assert!(
        rig.script.next().is_none(),
        "one value is sent, and no second request follows"
    );
}

#[gpui::test]
fn a_refused_save_keeps_the_value_in_the_field_and_says_why(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_add_named(&mut rig, cx, "OPENAI_API_KEY");
    type_value(&rig, cx, "settings-test-value-9c4e");
    save_and_refuse(&mut rig, cx);
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell
            .vault_dialog
            .as_ref()
            .expect("a refused save keeps the dialog open");
        assert_eq!(
            dialog.form.status,
            FormStatus::Refused(
                "Not saved: OPENAI_API_KEY already exists in this scope. Pick another name. Nothing was stored."
                    .to_owned()
            )
        );
    });
    let field = value_of_dialog(&rig, cx);
    assert!(
        field.read_with(cx, |field, _| field.text() == "settings-test-value-9c4e"),
        "a refused save keeps the value for the retry"
    );
    type_value(&rig, cx, "-typed");
    assert!(
        field.read_with(cx, |field, _| field.text().ends_with("-typed")),
        "the field is unlocked once the service has refused the save"
    );
}

#[gpui::test]
fn retry_sends_the_same_value_once_more_and_success_clears_it(cx: &mut TestAppContext) {
    const VALUE: &str = "settings-test-value-4d1f";
    let (mut rig, cx) = settings_rig(cx);
    type_and_save(&mut rig, cx, "OPENAI_API_KEY", VALUE);
    let (_, refused) = next_set(&mut rig);
    refused
        .send(Err(Refusal::Failed {
            code: codes::SECRET_VAULT_LOCKED.to_owned(),
            message: "the vault is locked".to_owned(),
        }))
        .expect("the page listens");
    cx.run_until_parked();

    let field = value_of_dialog(&rig, cx);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.save_secret(window, cx));
    cx.run_until_parked();
    let (retried, stored) = next_set(&mut rig);
    assert!(retried.value == VALUE, "the retry sends the same value");
    assert!(rig.script.next().is_none(), "the retry is the only request");
    stored
        .send(Ok(owned_secret("project:p1", "OPENAI_API_KEY")))
        .expect("the page listens");
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.vault_dialog.is_none()),
        "the dialog closes once the vault stores the value"
    );
    assert!(
        field.read_with(cx, |field, _| field.text().is_empty()),
        "success clears the value"
    );
}

#[gpui::test]
fn cancel_clears_the_typed_value_and_sends_nothing(cx: &mut TestAppContext) {
    const VALUE: &str = "settings-test-value-6e0a";
    let (mut rig, cx) = settings_rig(cx);
    open_add_named(&mut rig, cx, "OPENAI_API_KEY");
    type_value(&rig, cx, VALUE);
    let field = value_of_dialog(&rig, cx);
    click(cx, "dialog-cancel");
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.vault_dialog.is_none()),
        "cancel closes the dialog"
    );
    assert!(
        field.read_with(cx, |field, _| field.text().is_empty()),
        "cancel clears the value"
    );
    assert!(rig.script.next().is_none(), "cancel sends nothing");
}

#[gpui::test]
fn escape_clears_the_typed_value_and_sends_nothing(cx: &mut TestAppContext) {
    const VALUE: &str = "settings-test-value-7f2b";
    let (mut rig, cx) = settings_rig(cx);
    open_add_named(&mut rig, cx, "OPENAI_API_KEY");
    type_value(&rig, cx, VALUE);
    let field = value_of_dialog(&rig, cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.vault_dialog.is_none()),
        "escape closes the dialog"
    );
    assert!(
        field.read_with(cx, |field, _| field.text().is_empty()),
        "closing clears the value"
    );
    assert!(rig.script.next().is_none(), "closing sends nothing");
}

#[gpui::test]
fn tearing_the_dialog_down_leaves_no_value_field_behind(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    open_add_named(&mut rig, cx, "OPENAI_API_KEY");
    type_value(&rig, cx, "settings-test-value-2b9d");
    let field = value_of_dialog(&rig, cx).downgrade();
    rig.shell.update(cx, |shell, _| shell.vault_dialog = None);
    // The last frame still holds the field's paint handlers until the next draw replaces it.
    draw(cx);
    cx.run_until_parked();
    assert!(
        field.upgrade().is_none(),
        "the masked field and its buffer go with the dialog"
    );
}

/// The Debug text of the open dialog and of its value field.
fn debug_text(rig: &Rig, cx: &mut VisualTestContext) -> String {
    let dialog = rig
        .shell
        .read_with(cx, |shell, _| format!("{:?}", shell.vault_dialog));
    let field = value_of_dialog(rig, cx).read_with(cx, |field, _| format!("{field:?}"));
    format!("{dialog}{field}")
}

#[gpui::test]
fn the_value_stays_out_of_debug_output_at_every_step(cx: &mut TestAppContext) {
    const VALUE: &str = "settings-test-value-5c7e";
    let (mut rig, cx) = settings_rig(cx);
    open_add_named(&mut rig, cx, "OPENAI_API_KEY");
    type_value(&rig, cx, VALUE);
    assert!(!debug_text(&rig, cx).contains(VALUE), "while typing");
    rig.shell
        .update_in(cx, |shell, window, cx| shell.save_secret(window, cx));
    cx.run_until_parked();
    assert!(!debug_text(&rig, cx).contains(VALUE), "while in flight");
    let (_, reply) = next_set(&mut rig);
    reply
        .send(Err(Refusal::Failed {
            code: codes::SECRET_NAME_IN_USE.to_owned(),
            message: "OPENAI_API_KEY already exists in project:p1".to_owned(),
        }))
        .expect("the page listens");
    cx.run_until_parked();
    assert!(!debug_text(&rig, cx).contains(VALUE), "after a refusal");
}

/// The value the open dialog's field holds, or `None` once no dialog is open.
fn dialog_value(rig: &Rig, cx: &mut VisualTestContext) -> Option<String> {
    let field = rig.shell.read_with(cx, |shell, _| {
        shell
            .vault_dialog
            .as_ref()
            .map(|dialog| dialog.value_field.clone())
    })?;
    Some(field.read_with(cx, |field, _| field.text().to_owned()))
}

#[gpui::test]
fn a_scrim_press_during_a_save_keeps_the_dialog_and_a_late_success_closes_it(
    cx: &mut TestAppContext,
) {
    const VALUE: &str = "settings-test-value-e41a";
    let (mut rig, cx) = settings_rig(cx);
    type_and_save(&mut rig, cx, "OPENAI_API_KEY", VALUE);
    draw(cx);
    cx.simulate_click(point(px(4.), px(4.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        dialog_value(&rig, cx).as_deref(),
        Some(VALUE),
        "a scrim press neither closes the busy dialog nor clears its value"
    );
    let (_, reply) = next_set(&mut rig);
    reply
        .send(Ok(owned_secret("project:p1", "OPENAI_API_KEY")))
        .expect("the page listens");
    cx.run_until_parked();
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.vault_dialog.is_none()),
        "the late success closes the dialog that sent it"
    );
}

#[gpui::test]
fn a_late_success_does_not_close_a_newer_dialog(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    type_and_save(&mut rig, cx, "FIRST_KEY", "settings-test-value-a1");
    let (_, reply) = next_set(&mut rig);
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.close_secret_dialog(window, cx)
    });
    open_add_named(&mut rig, cx, "SECOND_KEY");
    type_value(&rig, cx, "settings-test-value-b2");
    reply
        .send(Ok(owned_secret("project:p1", "FIRST_KEY")))
        .expect("the page listens");
    cx.run_until_parked();
    assert_eq!(
        dialog_value(&rig, cx).as_deref(),
        Some("settings-test-value-b2"),
        "the newer dialog stays open with its value"
    );
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell
            .vault_dialog
            .as_ref()
            .expect("the newer dialog is open");
        assert_eq!(
            dialog.form.status,
            FormStatus::Idle,
            "its own status is untouched"
        );
    });
    assert!(rig.script.next().is_none(), "nothing else is sent");
}

#[gpui::test]
fn a_late_refusal_does_not_touch_a_newer_dialog(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    type_and_save(&mut rig, cx, "FIRST_KEY", "settings-test-value-c3");
    let (_, reply) = next_set(&mut rig);
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.close_secret_dialog(window, cx)
    });
    open_add_named(&mut rig, cx, "SECOND_KEY");
    type_value(&rig, cx, "settings-test-value-d4");
    reply
        .send(Err(Refusal::Failed {
            code: codes::SECRET_NAME_IN_USE.to_owned(),
            message: "FIRST_KEY already exists in project:p1".to_owned(),
        }))
        .expect("the page listens");
    cx.run_until_parked();
    assert_eq!(
        dialog_value(&rig, cx).as_deref(),
        Some("settings-test-value-d4"),
        "the newer dialog keeps its value"
    );
    rig.shell.read_with(cx, |shell, _| {
        let dialog = shell
            .vault_dialog
            .as_ref()
            .expect("the newer dialog is open");
        assert_eq!(
            dialog.form.status,
            FormStatus::Idle,
            "a refusal for an older save is not shown in the newer dialog"
        );
    });
}

#[gpui::test]
fn closing_the_window_clears_the_typed_value_before_the_window_goes(cx: &mut TestAppContext) {
    const VALUE: &str = "settings-test-value-f5c2";
    let (mut rig, cx) = settings_rig(cx);
    open_add_named(&mut rig, cx, "OPENAI_API_KEY");
    type_value(&rig, cx, VALUE);
    let field = value_of_dialog(&rig, cx);
    assert!(cx.simulate_close(), "the window allows its close");
    assert!(
        field.read_with(cx, |field, _| field.text().is_empty()),
        "the window's close clears the value before the window goes"
    );
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.vault_dialog.is_none()),
        "the dialog goes with its window"
    );
}

#[gpui::test]
fn the_extensions_entry_opens_marketplace_installed(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    click(cx, "settings-nav-Extensions");
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.model().navigation().page),
        Page::Marketplace(MarketplacePage::Installed)
    );
}

pub(super) fn paired_host(name: &str) -> HostRecord {
    HostRecord {
        name: name.into(),
        server: "vps-server".into(),
        addresses: vec!["100.64.0.10:7347".into()],
        pin: "sha256:test".into(),
        device_id: "dev-1".into(),
        device_name: "laptop".into(),
        scopes: vec!["read".into()],
        paired_at_ms: 1,
    }
}

#[gpui::test]
fn each_opening_of_settings_asks_the_bridge_for_the_paired_hosts_once(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    rig.shell
        .update(cx, |shell, _| shell.model.open_page(Page::Home));
    draw(cx);
    rig.shell
        .update(cx, |shell, _| shell.model.open_page(Page::Settings));
    draw(cx);
    draw(cx);
    let mut asks = 0;
    while let Some(command) = rig.script.next() {
        if matches!(command, Command::RefreshHosts) {
            asks += 1;
        }
    }
    assert_eq!(asks, 1, "one request each time the page is shown");
}

#[gpui::test]
fn a_paired_host_is_a_row_in_the_hosts_section(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    let link = HostLink::Connected {
        server: "vps-server".into(),
        version: "0.1.0".into(),
    };
    rig._updates
        .send(StoreUpdate::HostsResync(vec![paired_host("vps")]))
        .expect("the store listens");
    rig._updates
        .send(StoreUpdate::HostLinkChanged {
            name: "vps".into(),
            link,
        })
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "settings-nav-Hosts");
    draw(cx);
    assert!(
        cx.debug_bounds("settings-host-vps").is_some(),
        "the paired host has its row, after This PC"
    );
}
