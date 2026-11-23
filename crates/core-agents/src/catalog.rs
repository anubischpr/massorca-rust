//! The kind registry (decision 0050): built-in declarations compiled in, plus `<profile>/agents/kinds/*.json`.
//!
//! The registry is read again on every call, so editing a file needs no restart.

use std::fs;
use std::path::{Path, PathBuf};

use massorca_protocol::methods::agents::{self as wire, AgentKind, KindCredentials};

use crate::credentials;
use crate::kind::{KindDecl, PermissionModeDecl, StatusMechanism};
use crate::paths;
use crate::resolve::{SearchEnv, resolve_first};

/// Owner of the built-in kinds.
pub const OWNER_CORE: &str = "core";
/// Owner of the kinds a profile declares.
pub const OWNER_USER: &str = "user";

/// The built-in declarations, in the order `agent.kinds` lists them.
const BUILT_INS: [&str; 4] = [
    include_str!("../kinds/claude.json"),
    include_str!("../kinds/codex.json"),
    include_str!("../kinds/antigravity.json"),
    include_str!("../kinds/gentle-shell.json"),
];

/// The built-in kinds. They are checked by this crate's tests, so a broken one never ships.
pub fn built_ins() -> Vec<KindDecl> {
    BUILT_INS
        .iter()
        .map(|text| KindDecl::from_json(text).expect("a built-in kind declaration parses"))
        .collect()
}

/// One declared kind: valid, or listed with the problems that keep it from starting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: String,
    pub owner: String,
    /// `None` when the file did not parse.
    pub decl: Option<KindDecl>,
    /// The file it came from; `None` for built-ins.
    pub source: Option<PathBuf>,
    pub problems: Vec<String>,
}

impl CatalogEntry {
    /// The declaration, when it has no problems.
    pub fn valid(&self) -> Option<&KindDecl> {
        self.decl.as_ref().filter(|_| self.problems.is_empty())
    }
}

/// Every declared kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
}

impl Catalog {
    /// The built-ins, then the profile's files by file name.
    pub fn load(profile_dir: &Path) -> Self {
        let mut entries: Vec<CatalogEntry> = built_ins()
            .into_iter()
            .map(|decl| CatalogEntry {
                id: decl.id.clone(),
                owner: OWNER_CORE.to_owned(),
                problems: decl.problems(),
                decl: Some(decl),
                source: None,
            })
            .collect();
        for file in kind_files(&paths::kinds_dir(profile_dir)) {
            let entry = load_file(&file, &entries);
            entries.push(entry);
        }
        Self { entries }
    }

    /// The entry with `id`. A built-in always comes before a profile file that reuses its id.
    pub fn find(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Every kind as `agent.kinds` lists it, with programs resolved in `env`.
    pub fn views(&self, profile_dir: &Path, env: &SearchEnv) -> Vec<AgentKind> {
        self.entries
            .iter()
            .map(|entry| view(entry, profile_dir, env))
            .collect()
    }
}

/// The `.json` files of the kinds directory, by file name. A missing directory has none.
fn kind_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
                && fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
        })
        .collect();
    files.sort();
    files
}

fn load_file(file: &Path, earlier: &[CatalogEntry]) -> CatalogEntry {
    let stem = file
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parsed = fs::read_to_string(file)
        .map_err(|error| format!("cannot read the file: {error}"))
        .and_then(|text| KindDecl::from_json(&text));
    let decl = match parsed {
        Ok(decl) => decl,
        Err(problem) => {
            return CatalogEntry {
                id: stem,
                owner: OWNER_USER.to_owned(),
                decl: None,
                source: Some(file.to_path_buf()),
                problems: vec![problem],
            };
        }
    };
    let mut problems = decl.problems();
    if let Some(taken) = earlier.iter().find(|entry| entry.id == decl.id) {
        problems.push(if taken.owner == OWNER_CORE {
            format!("id {:?} is a built-in kind and cannot be replaced", decl.id)
        } else {
            format!("id {:?} is already declared by another file", decl.id)
        });
    }
    CatalogEntry {
        id: decl.id.clone(),
        owner: OWNER_USER.to_owned(),
        decl: Some(decl),
        source: Some(file.to_path_buf()),
        problems,
    }
}

/// Until packages land (0059), every compiled-in kind and profile file is an enabled package with no id.
fn enabled_package() -> wire::KindPackage {
    wire::KindPackage {
        state: wire::PackageState::Enabled,
        id: None,
        version: None,
    }
}

fn view(entry: &CatalogEntry, profile_dir: &Path, env: &SearchEnv) -> AgentKind {
    // The 0056 home counts only as a plain directory, checked under the homes gate (decision 0056, 0059).
    let imported = {
        let _gate = credentials::lock_homes();
        credentials::usable_home(profile_dir, &entry.id)
            .ok()
            .flatten()
            .is_some()
    };
    let Some(decl) = &entry.decl else {
        return AgentKind {
            id: entry.id.clone(),
            name: entry.id.clone(),
            icon: "bot".to_owned(),
            owner: entry.owner.clone(),
            installed: false,
            detected: false,
            program_path: None,
            package: enabled_package(),
            resumable: false,
            status_mechanism: wire::StatusMechanism::None,
            credentials: KindCredentials {
                supported: false,
                imported,
            },
            problems: entry.problems.clone(),
            permission_modes: Vec::new(),
            default_permission_mode: None,
        };
    };
    let program_path = if entry.problems.is_empty() {
        resolve_first(&decl.program, env)
    } else {
        None
    };
    AgentKind {
        id: entry.id.clone(),
        name: decl.name.clone(),
        icon: decl.icon.clone(),
        owner: entry.owner.clone(),
        installed: program_path.is_some(),
        detected: program_path.is_some(),
        program_path: program_path.map(|path| path.display().to_string()),
        package: enabled_package(),
        resumable: decl.resumable(),
        status_mechanism: mechanism_of(decl.status.mechanism),
        credentials: KindCredentials {
            supported: decl.credentials_supported(),
            imported,
        },
        problems: entry.problems.clone(),
        permission_modes: decl.permission_modes.iter().map(mode_view).collect(),
        default_permission_mode: decl.default_permission_mode.clone(),
    }
}

/// A declared mode as `agent.kinds` lists it: its arguments and switch text stay in the declaration.
fn mode_view(mode: &PermissionModeDecl) -> wire::PermissionMode {
    wire::PermissionMode {
        id: mode.id.clone(),
        label: mode.label.clone(),
        description: mode.description.clone(),
        bypass: mode.bypass,
        live_switch: mode.live_switch.is_some(),
    }
}

pub(crate) fn mechanism_of(mechanism: StatusMechanism) -> wire::StatusMechanism {
    match mechanism {
        StatusMechanism::ClaudeSettings => wire::StatusMechanism::ClaudeSettings,
        StatusMechanism::CodexNotify => wire::StatusMechanism::CodexNotify,
        StatusMechanism::EnvCommand => wire::StatusMechanism::EnvCommand,
        StatusMechanism::None => wire::StatusMechanism::None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::kind::{DoneOutcome, EventTarget, PasteMode, PromptMode};

    fn write_kind(profile: &Path, file: &str, text: &str) {
        let dir = paths::kinds_dir(profile);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file), text).unwrap();
    }

    fn by_id<'a>(catalog: &'a Catalog, id: &str, owner: &str) -> &'a CatalogEntry {
        catalog
            .entries
            .iter()
            .find(|entry| entry.id == id && entry.owner == owner)
            .unwrap_or_else(|| panic!("no {owner} entry {id}"))
    }

    #[test]
    fn every_built_in_parses_without_problems() {
        let kinds = built_ins();
        let ids: Vec<&str> = kinds.iter().map(|kind| kind.id.as_str()).collect();
        assert_eq!(ids, ["claude", "codex", "antigravity", "gentle-shell"]);
        for kind in &kinds {
            assert!(
                kind.problems().is_empty(),
                "{}: {:?}",
                kind.id,
                kind.problems()
            );
        }
    }

    #[test]
    fn built_ins_match_decision_0050() {
        let kinds = built_ins();
        let claude = &kinds[0];
        assert_eq!(claude.program, ["claude"]);
        assert_eq!(claude.prompt.paste, PasteMode::Bracketed);
        let session_ref = claude.session_ref.as_ref().unwrap();
        assert_eq!(
            session_ref.assign.as_deref(),
            Some(&["--session-id".to_owned(), "{ref}".to_owned()][..])
        );
        assert_eq!(session_ref.learn.as_deref(), Some("session_id"));
        assert_eq!(
            claude.resume.as_deref(),
            Some(&["--resume".to_owned(), "{ref}".to_owned()][..])
        );
        assert_eq!(claude.status.mechanism, StatusMechanism::ClaudeSettings);
        assert_eq!(claude.status.events["Notification"], EventTarget::Waiting);
        assert_eq!(
            claude.status.events["Stop"],
            EventTarget::Done(DoneOutcome::Success)
        );
        assert_eq!(claude.status.events.len(), 5);
        let credentials = claude.credentials.as_ref().unwrap();
        assert_eq!(credentials.home_env, "CLAUDE_CONFIG_DIR");
        assert_eq!(credentials.files, [".credentials.json", ".claude.json"]);
        assert_eq!(
            credentials.strip_env,
            [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_AUTH_TOKEN",
                "CLAUDE_CODE_OAUTH_TOKEN"
            ]
        );
        assert!(claude.resumable());

        let codex = &kinds[1];
        assert_eq!(
            codex.resume.as_deref(),
            Some(&["resume".to_owned(), "{ref}".to_owned()][..])
        );
        assert_eq!(
            codex.session_ref.as_ref().unwrap().learn.as_deref(),
            Some("thread-id")
        );
        assert_eq!(codex.status.mechanism, StatusMechanism::CodexNotify);
        assert_eq!(
            codex.status.events["turn-complete"],
            EventTarget::Done(DoneOutcome::Success)
        );
        assert_eq!(codex.credentials.as_ref().unwrap().home_env, "CODEX_HOME");
        assert_eq!(
            codex.credentials.as_ref().unwrap().strip_env,
            ["OPENAI_API_KEY"]
        );
        assert!(codex.resumable());

        let antigravity = &kinds[2];
        assert_eq!(antigravity.program, ["agy"]);
        assert_eq!(antigravity.prompt.mode, PromptMode::Flag);
        assert_eq!(
            antigravity.prompt.flag.as_deref(),
            Some("--prompt-interactive")
        );
        assert_eq!(antigravity.status.mechanism, StatusMechanism::None);
        assert!(antigravity.credentials.is_none());
        assert!(!antigravity.resumable(), "its ref is never learned yet");

        let gentle = &kinds[3];
        assert_eq!(
            (gentle.name.as_str(), gentle.icon.as_str()),
            ("Gentle Shell", "rose")
        );
        assert_eq!(
            gentle.resume.as_deref(),
            Some(&["--".to_owned(), "--session".to_owned(), "{ref}".to_owned()][..])
        );
        assert_eq!(
            gentle.credentials.as_ref().unwrap().home_env,
            "PI_CODING_AGENT_DIR"
        );
        assert!(!gentle.credentials_supported());
        assert!(!gentle.resumable());
    }

    #[test]
    fn a_profile_without_kind_files_lists_the_built_ins() {
        let profile = tempfile::tempdir().unwrap();
        let catalog = Catalog::load(profile.path());
        assert_eq!(catalog.entries.len(), 4);
        assert!(
            catalog
                .entries
                .iter()
                .all(|entry| entry.owner == OWNER_CORE)
        );
    }

    #[test]
    fn profile_files_load_with_owner_user_after_the_built_ins() {
        let profile = tempfile::tempdir().unwrap();
        let fake = json!({ "id": "fake", "name": "Fake agent", "program": ["fake-agent"] });
        write_kind(profile.path(), "fake.json", &fake.to_string());
        write_kind(profile.path(), "notes.txt", "ignored");
        let catalog = Catalog::load(profile.path());
        assert_eq!(catalog.entries.len(), 5);
        let entry = &catalog.entries[4];
        assert_eq!(
            (entry.id.as_str(), entry.owner.as_str()),
            ("fake", OWNER_USER)
        );
        assert!(entry.valid().is_some());
    }

    #[test]
    fn invalid_files_are_listed_with_problems_and_not_valid() {
        let profile = tempfile::tempdir().unwrap();
        write_kind(profile.path(), "broken.json", "{ not json");
        let bad =
            json!({ "id": "bad", "name": "Bad", "program": [], "prompt": { "mode": "flag" } });
        write_kind(profile.path(), "bad.json", &bad.to_string());
        let catalog = Catalog::load(profile.path());

        let broken = by_id(&catalog, "broken", OWNER_USER);
        assert!(broken.decl.is_none());
        assert_eq!(broken.problems.len(), 1);
        assert!(broken.valid().is_none());

        let bad = by_id(&catalog, "bad", OWNER_USER);
        assert_eq!(bad.problems.len(), 2, "{:?}", bad.problems);
        assert!(bad.valid().is_none());

        let views = catalog.views(profile.path(), &SearchEnv::default());
        let view = views.iter().find(|kind| kind.id == "broken").unwrap();
        assert!(!view.installed);
        assert!(!view.problems.is_empty());
    }

    #[test]
    fn a_file_reusing_a_built_in_id_never_replaces_it() {
        let profile = tempfile::tempdir().unwrap();
        let shadow = json!({ "id": "claude", "name": "Not Claude", "program": ["other"] });
        write_kind(profile.path(), "claude.json", &shadow.to_string());
        let catalog = Catalog::load(profile.path());
        let found = catalog.find("claude").unwrap();
        assert_eq!(found.owner, OWNER_CORE);
        assert_eq!(found.decl.as_ref().unwrap().name, "Claude");
        let user = by_id(&catalog, "claude", OWNER_USER);
        assert!(
            user.problems
                .iter()
                .any(|problem| problem.contains("built-in"))
        );
        assert!(user.valid().is_none());
    }

    #[test]
    fn a_second_file_with_the_same_id_has_a_problem() {
        let profile = tempfile::tempdir().unwrap();
        let fake = json!({ "id": "fake", "name": "Fake", "program": ["fake-agent"] });
        write_kind(profile.path(), "a.json", &fake.to_string());
        write_kind(profile.path(), "b.json", &fake.to_string());
        let catalog = Catalog::load(profile.path());
        let fakes: Vec<&CatalogEntry> = catalog.entries.iter().filter(|e| e.id == "fake").collect();
        assert_eq!(fakes.len(), 2);
        assert!(fakes[0].valid().is_some());
        assert!(
            fakes[1]
                .problems
                .iter()
                .any(|problem| problem.contains("already declared"))
        );
    }

    #[test]
    fn views_resolve_programs_and_report_credential_homes() {
        let profile = tempfile::tempdir().unwrap();
        let program = std::env::current_exe().unwrap();
        let fake = json!({
            "id": "fake",
            "name": "Fake",
            "program": [program.to_string_lossy()],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
        });
        write_kind(profile.path(), "fake.json", &fake.to_string());
        fs::create_dir_all(paths::home_dir(profile.path(), "fake")).unwrap();
        let views = Catalog::load(profile.path()).views(profile.path(), &SearchEnv::default());
        let view = views.iter().find(|kind| kind.id == "fake").unwrap();
        assert!(view.installed);
        assert!(view.detected, "detected is the same value as installed");
        assert_eq!(view.package.state, wire::PackageState::Enabled);
        assert_eq!(view.package.id, None);
        assert_eq!(
            view.program_path.as_deref(),
            Some(program.to_string_lossy().as_ref())
        );
        assert_eq!(
            view.credentials,
            KindCredentials {
                supported: true,
                imported: true
            }
        );
        let claude = views.iter().find(|kind| kind.id == "claude").unwrap();
        assert!(!claude.installed, "no PATH, so no bare name resolves");
        assert!(!claude.detected);
        assert_eq!(claude.package.state, wire::PackageState::Enabled);
        assert_eq!(
            claude.status_mechanism,
            wire::StatusMechanism::ClaudeSettings
        );
    }

    #[test]
    fn only_claude_declares_a_sign_in_among_the_built_ins() {
        let kinds = built_ins();
        let with_accounts: Vec<&str> = kinds
            .iter()
            .filter(|kind| kind.accounts_supported())
            .map(|kind| kind.id.as_str())
            .collect();
        assert_eq!(with_accounts, ["claude"]);
        let claude = kinds.iter().find(|kind| kind.id == "claude").unwrap();
        let accounts = claude.accounts.as_ref().unwrap();
        assert_eq!(accounts.login, ["auth", "login"]);
        let check = accounts.check.as_ref().unwrap();
        assert_eq!(check.args, ["auth", "status"]);
        assert_eq!(check.signed_in_exit, 0);
        assert_eq!(check.expired_pattern, None);
        assert_eq!(check.identity_pattern, None);
        assert_eq!(
            accounts.logout,
            Some(vec!["auth".to_owned(), "logout".to_owned()])
        );
    }
    /// One declared mode: `(id, label, args, live switch text, bypass)`.
    type ModeRow<'a> = (&'a str, &'a str, Vec<&'a str>, Option<&'a str>, bool);

    /// Every mode of `kind`, in declared order.
    fn modes_of(kind: &KindDecl) -> Vec<ModeRow<'_>> {
        kind.permission_modes
            .iter()
            .map(|mode| {
                (
                    mode.id.as_str(),
                    mode.label.as_str(),
                    mode.args.iter().map(String::as_str).collect(),
                    mode.live_switch.as_ref().map(|switch| switch.text.as_str()),
                    mode.bypass,
                )
            })
            .collect()
    }

    #[test]
    fn built_in_permission_modes_match_decision_0083() {
        let kinds = built_ins();
        let claude = &kinds[0];
        assert_eq!(claude.default_permission_mode.as_deref(), Some("default"));
        assert_eq!(
            modes_of(claude),
            [
                (
                    "default",
                    "Manual",
                    vec!["--permission-mode", "default"],
                    None,
                    false
                ),
                (
                    "acceptEdits",
                    "Edit automatically",
                    vec!["--permission-mode", "acceptEdits"],
                    None,
                    false
                ),
                (
                    "plan",
                    "Plan",
                    vec!["--permission-mode", "plan"],
                    None,
                    false
                ),
                (
                    "auto",
                    "Auto",
                    vec!["--permission-mode", "auto"],
                    None,
                    false
                ),
                (
                    "dontAsk",
                    "Don't ask",
                    vec!["--permission-mode", "dontAsk"],
                    None,
                    false
                ),
                (
                    "bypassPermissions",
                    "Bypass permissions",
                    vec!["--permission-mode", "bypassPermissions"],
                    None,
                    true
                ),
            ]
        );

        let codex = &kinds[1];
        assert_eq!(codex.default_permission_mode.as_deref(), Some("auto"));
        assert_eq!(
            modes_of(codex),
            [
                (
                    "auto",
                    "Auto",
                    vec![
                        "--sandbox",
                        "workspace-write",
                        "--ask-for-approval",
                        "on-request"
                    ],
                    None,
                    false
                ),
                (
                    "read-only",
                    "Safe read-only browsing",
                    vec!["--sandbox", "read-only", "--ask-for-approval", "on-request"],
                    None,
                    false
                ),
                (
                    "read-only-never",
                    "Read-only non-interactive (CI)",
                    vec!["--sandbox", "read-only", "--ask-for-approval", "never"],
                    None,
                    false
                ),
                (
                    "auto-review",
                    "Auto-review mode",
                    vec!["--approve-for-me"],
                    None,
                    false
                ),
                (
                    "full-access",
                    "Dangerous full access",
                    vec!["--dangerously-bypass-approvals-and-sandbox"],
                    None,
                    true
                ),
            ]
        );

        let antigravity = &kinds[2];
        assert_eq!(antigravity.default_permission_mode.as_deref(), Some("auto"));
        assert_eq!(
            modes_of(antigravity),
            [
                ("auto", "auto", vec!["--mode=auto"], None, false),
                (
                    "accept-edits",
                    "accept-edits",
                    vec!["--mode=accept-edits"],
                    None,
                    false
                ),
                ("plan", "plan", vec!["--mode=plan"], None, false),
                (
                    "always-proceed",
                    "always-proceed",
                    vec!["--dangerously-skip-permissions"],
                    None,
                    true
                ),
            ]
        );

        let gentle = &kinds[3];
        assert_eq!(gentle.default_permission_mode.as_deref(), Some("normal"));
        assert_eq!(
            modes_of(gentle),
            [
                (
                    "normal",
                    "Normal",
                    vec![],
                    Some("/gentle:mode normal"),
                    false
                ),
                ("auto", "Auto", vec![], Some("/gentle:mode auto"), false),
                ("yolo", "YOLO", vec![], Some("/gentle:mode yolo"), false),
                (
                    "super-yolo",
                    "Super Yolo",
                    vec![],
                    Some("/gentle:mode super-yolo"),
                    true
                ),
            ]
        );
    }

    #[test]
    fn views_list_the_modes_without_their_arguments() {
        let profile = tempfile::tempdir().unwrap();
        let views = Catalog::load(profile.path()).views(profile.path(), &SearchEnv::default());
        let gentle = views.iter().find(|kind| kind.id == "gentle-shell").unwrap();
        assert_eq!(gentle.default_permission_mode.as_deref(), Some("normal"));
        let super_yolo = gentle
            .permission_modes
            .iter()
            .find(|mode| mode.id == "super-yolo")
            .unwrap();
        assert_eq!(
            super_yolo,
            &wire::PermissionMode {
                id: "super-yolo".to_owned(),
                label: "Super Yolo".to_owned(),
                description: super_yolo.description.clone(),
                bypass: true,
                live_switch: true,
            }
        );
        let claude = views.iter().find(|kind| kind.id == "claude").unwrap();
        assert_eq!(claude.permission_modes.len(), 6);
        assert!(claude.permission_modes.iter().all(|mode| !mode.live_switch));
    }
}
