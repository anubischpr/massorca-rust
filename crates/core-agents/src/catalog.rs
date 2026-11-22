//! The kind registry (decision 0050): built-in declarations compiled in, plus `<profile>/agents/kinds/*.json`.
//!
//! The registry is read again on every call, so editing a file needs no restart.

use std::fs;
use std::path::{Path, PathBuf};

use massorca_protocol::methods::agents::{self as wire, AgentKind, KindCredentials};

use crate::kind::{KindDecl, StatusMechanism};
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

fn view(entry: &CatalogEntry, profile_dir: &Path, env: &SearchEnv) -> AgentKind {
    let imported = paths::valid_home(profile_dir, &entry.id).is_some_and(|home| home.is_dir());
    let Some(decl) = &entry.decl else {
        return AgentKind {
            id: entry.id.clone(),
            name: entry.id.clone(),
            icon: "bot".to_owned(),
            owner: entry.owner.clone(),
            installed: false,
            program_path: None,
            resumable: false,
            status_mechanism: wire::StatusMechanism::None,
            credentials: KindCredentials {
                supported: false,
                imported,
            },
            problems: entry.problems.clone(),
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
        program_path: program_path.map(|path| path.display().to_string()),
        resumable: decl.resumable(),
        status_mechanism: mechanism_of(decl.status.mechanism),
        credentials: KindCredentials {
            supported: decl.credentials_supported(),
            imported,
        },
        problems: entry.problems.clone(),
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
        assert_eq!(
            claude.status_mechanism,
            wire::StatusMechanism::ClaudeSettings
        );
    }
}
