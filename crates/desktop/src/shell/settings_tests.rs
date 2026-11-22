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
    Command, ConnectionState, Refusal, Script, SecretCommand, SecretsUpdate, StoreUpdate,
};
use gpui::{AppContext, ClipboardItem, Entity, Modifiers, TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::config::{ConfigList, ConfigValue};
use massorca_protocol::methods::secrets::{SecretEntry, SecretRemoved, SecretStatus};
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

struct Rig {
    shell: Entity<Shell>,
    _updates: UnboundedSender<StoreUpdate>,
    script: Script,
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    draw(cx);
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::default());
}

/// A connected shell on a scripted service with one project, opened on the Settings page.
fn settings_rig(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
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
    match rig.script.next() {
        Some(Command::ListConfig { reply }) => {
            let values = vec![config_value(stored, stored == DEFAULT)];
            reply
                .send(Ok(ConfigList { values }))
                .expect("the page listens");
        }
        _ => panic!("a config.list call was expected"),
    }
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
fn a_typed_value_is_sent_once_and_cleared_from_the_field_as_it_is_sent(cx: &mut TestAppContext) {
    const VALUE: &str = "settings-test-value-3a8c";
    let (mut rig, cx) = settings_rig(cx);
    type_and_save(&mut rig, cx, "OPENAI_API_KEY", VALUE);
    assert!(
        value_of_dialog(&rig, cx).read_with(cx, |field, _| field.text().is_empty()),
        "the typed value leaves the field as it is sent"
    );
    rig.shell
        .update_in(cx, |shell, window, cx| shell.save_secret(window, cx));
    cx.run_until_parked();
    match rig.script.next() {
        Some(Command::Secret(SecretCommand::Set { params, reply })) => {
            assert!(params.value == VALUE, "the typed value is what is sent");
            reply
                .send(Ok(owned_secret("project:p1", "OPENAI_API_KEY")))
                .expect("the page listens");
        }
        _ => panic!("a secret.set was expected"),
    }
    assert!(
        rig.script.next().is_none(),
        "a second save while the first is in flight sends nothing"
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
fn a_refused_save_keeps_the_dialog_open_and_drops_the_sent_value(cx: &mut TestAppContext) {
    let (mut rig, cx) = settings_rig(cx);
    add_and_save(&mut rig, cx, "OPENAI_API_KEY", "settings-test-value-9c4e");
    match rig.script.next() {
        Some(Command::Secret(SecretCommand::Set { reply, .. })) => {
            reply
                .send(Err(Refusal::Failed {
                    code: codes::SECRET_NAME_IN_USE.to_owned(),
                    message: "OPENAI_API_KEY already exists in project:p1".to_owned(),
                }))
                .expect("the page listens");
        }
        _ => panic!("a secret.set was expected"),
    }
    cx.run_until_parked();
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
    assert!(
        value_of_dialog(&rig, cx).read_with(cx, |field, _| field.text().is_empty()),
        "the value was sent and is gone from the field"
    );
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
