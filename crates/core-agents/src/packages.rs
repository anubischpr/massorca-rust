//! Where kind declarations come from (decision 0059): the packages that provide them, and their state.
//!
//! The extension runtime implements [`KindPackages`]; this crate never depends on it. Until it does, [`BuiltInPackages`]
//! provides the four compiled-in declarations as enabled first-party packages.

use std::fmt;

pub use massorca_protocol::methods::agents::PackageState;

/// The package id prefix of first-party packages. Their kinds keep the owner `core`.
pub const FIRST_PARTY_PREFIX: &str = "massorca.";

/// The kinds that packages provide, read again on every catalog load.
pub trait KindPackages: Send + Sync {
    /// Every kind a package provides, in the order `agent.kinds` lists them: enabled, disabled and available ones.
    fn kinds(&self) -> Vec<PackageKind>;
}

impl fmt::Debug for dyn KindPackages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KindPackages")
    }
}

/// One kind a package provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageKind {
    /// The kind's id, as the package declares it.
    pub kind_id: String,
    /// The package's catalogue id, such as `massorca.claude`.
    pub package_id: String,
    /// The package's installed version, or the catalogue's for an available one.
    pub version: String,
    pub state: PackageState,
    /// The declaration's JSON text; the error names the package and the cause, never a path.
    pub decl: Result<String, String>,
}

/// The four built-in declarations, in the order `agent.kinds` lists them.
const BUILT_INS: [(&str, &str); 4] = [
    ("claude", include_str!("../kinds/claude.json")),
    ("codex", include_str!("../kinds/codex.json")),
    ("antigravity", include_str!("../kinds/antigravity.json")),
    ("gentle-shell", include_str!("../kinds/gentle-shell.json")),
];

/// The compiled-in declarations as enabled first-party packages `massorca.<kind>` at this crate's version.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuiltInPackages;

impl KindPackages for BuiltInPackages {
    fn kinds(&self) -> Vec<PackageKind> {
        BUILT_INS
            .iter()
            .map(|(id, text)| PackageKind {
                kind_id: (*id).to_owned(),
                package_id: format!("{FIRST_PARTY_PREFIX}{id}"),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                state: PackageState::Enabled,
                decl: Ok((*text).to_owned()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use massorca_core_store::{
        AgentAccountRow, AgentSessionRow, RunExit, RunExitReason, RunMode, new_id, now_ms,
    };
    use massorca_protocol::codes;
    use massorca_protocol::methods::agents::{AgentDelegateParams, AgentStartParams};
    use serde_json::json;

    use super::*;
    use crate::catalog::{Catalog, OWNER_CORE, OWNER_USER};
    use crate::paths;
    use crate::provided_env::SecretGrant;
    use crate::resolve::SearchEnv;
    use crate::sessions::tests::service_with;
    use crate::sessions::{AgentError, Agents};

    /// A provider with fixed kinds.
    struct TestPackages(Vec<PackageKind>);

    impl KindPackages for TestPackages {
        fn kinds(&self) -> Vec<PackageKind> {
            self.0.clone()
        }
    }

    /// The built-ins, with the package of `id` in `state`.
    fn built_ins_with(id: &str, state: PackageState) -> TestPackages {
        let mut kinds = BuiltInPackages.kinds();
        for kind in &mut kinds {
            if kind.kind_id == id {
                kind.state = state;
            }
        }
        TestPackages(kinds)
    }

    /// A program name no `PATH` resolves: a start that got past the gate would stop at the program check.
    const MISSING_PROGRAM: &str = "massorca-test-no-such-agent";

    /// A third-party package `ext.fake` whose kind `fake` runs `program`. It declares a credential home and no sign-in,
    /// check or sign-out, so no account call spawns anything.
    fn fake_package(state: PackageState, program: &str) -> PackageKind {
        let decl = json!({
            "id": "fake",
            "name": "Fake",
            "program": [program],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
        });
        PackageKind {
            kind_id: "fake".to_owned(),
            package_id: "ext.fake".to_owned(),
            version: "1.2.3".to_owned(),
            state,
            decl: Ok(decl.to_string()),
        }
    }

    /// This test binary, as a program that resolves. Only kinds no call can start use it.
    fn this_binary() -> String {
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    /// A workspace whose directory exists, so a start reaches the kind checks.
    fn existing_workspace(agents: &Agents, base: &str, dir: &std::path::Path) -> String {
        let mut row = agents.store().get_workspace(base).unwrap().unwrap();
        row.id = new_id();
        row.name = "existing".to_owned();
        row.path = dir.to_string_lossy().into_owned();
        row.path_key = row.path.clone();
        agents.store().insert_workspace(&row).unwrap();
        row.id
    }

    fn service(kinds: Vec<PackageKind>) -> (Agents, String, tempfile::TempDir) {
        service_with(Arc::new(TestPackages(kinds)))
    }

    fn assert_not_enabled<T: std::fmt::Debug>(
        result: Result<T, AgentError>,
        kind: &str,
        package: &str,
        state: &str,
    ) {
        let error = result.expect_err("a kind that is not enabled is refused");
        assert_eq!(
            error.code,
            codes::AGENT_KIND_NOT_ENABLED,
            "{}",
            error.message
        );
        assert_eq!(
            error.data,
            Some(json!({ "kind": kind, "package": package, "state": state }))
        );
        assert!(
            error
                .message
                .contains("install or enable it in the Marketplace"),
            "{}",
            error.message
        );
    }

    fn start_params(workspace: &str, kind: &str) -> AgentStartParams {
        serde_json::from_value(json!({ "workspace": workspace, "kind": kind })).unwrap()
    }

    /// An exited session of `kind` with a known ref: everything a resume needs before it looks at the kind.
    fn exited_session(agents: &Agents, workspace: &str, kind: &str) -> AgentSessionRow {
        let row = AgentSessionRow {
            id: new_id(),
            kind: kind.to_owned(),
            workspace_id: workspace.to_owned(),
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: Some("ref-1".to_owned()),
            terminal_id: None,
            request_id: None,
            created_at: now_ms(),
            exit: Some(RunExit {
                code: Some(0),
                signal: None,
                reason: RunExitReason::Exited,
                at: now_ms(),
            }),
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
        };
        agents.store().insert_agent_session(&row).unwrap();
        row
    }

    fn account(kind: &str, origin: &str) -> AgentAccountRow {
        AgentAccountRow {
            id: new_id(),
            kind: kind.to_owned(),
            label: "work".to_owned(),
            created_at: now_ms(),
            identity: None,
            state: "signed_in".to_owned(),
            origin: origin.to_owned(),
        }
    }

    #[test]
    fn built_in_packages_list_todays_kinds_as_enabled_first_party_packages() {
        let profile = tempfile::tempdir().unwrap();
        let catalog = Catalog::load(&BuiltInPackages, profile.path());
        let views = catalog.views(profile.path(), &SearchEnv::default());
        let ids: Vec<&str> = views.iter().map(|kind| kind.id.as_str()).collect();
        assert_eq!(ids, ["claude", "codex", "antigravity", "gentle-shell"]);
        for (view, decl) in views.iter().zip(crate::built_ins()) {
            assert_eq!(view.owner, OWNER_CORE);
            assert_eq!(view.name, decl.name);
            assert!(view.problems.is_empty(), "{:?}", view.problems);
            assert_eq!(view.package.state, PackageState::Enabled);
            assert_eq!(
                view.package.id.as_deref(),
                Some(format!("massorca.{}", view.id).as_str())
            );
            assert_eq!(
                view.package.version.as_deref(),
                Some(env!("CARGO_PKG_VERSION"))
            );
        }
        assert!(catalog.entries.iter().all(|entry| entry.enabled()));
    }

    #[test]
    fn a_third_party_package_is_owned_by_its_package() {
        let packages = TestPackages(vec![fake_package(PackageState::Enabled, &this_binary())]);
        let (agents, _, _profile) = service_with(Arc::new(packages));
        let kinds = agents.kinds().kinds;
        assert_eq!(kinds.len(), 1, "only the provider's kinds are listed");
        assert_eq!(kinds[0].owner, "ext:ext.fake");
        assert_eq!(kinds[0].package.id.as_deref(), Some("ext.fake"));
        assert_eq!(kinds[0].package.version.as_deref(), Some("1.2.3"));
        assert!(kinds[0].detected);
    }

    #[test]
    fn an_available_kind_is_listed_and_not_startable() {
        let (agents, base, profile) =
            service(vec![fake_package(PackageState::Available, MISSING_PROGRAM)]);
        let workspace = existing_workspace(&agents, &base, profile.path());
        let kinds = agents.kinds().kinds;
        let fake = kinds.iter().find(|kind| kind.id == "fake").unwrap();
        assert_eq!(fake.package.state, PackageState::Available);
        assert!(fake.problems.is_empty());
        assert_not_enabled(
            agents.start(start_params(&workspace, "fake"), SecretGrant::Provide),
            "fake",
            "ext.fake",
            "available",
        );
    }

    #[test]
    fn a_tampered_package_is_listed_with_its_problem_and_never_starts() {
        let mut tampered = fake_package(PackageState::Enabled, MISSING_PROGRAM);
        tampered.decl = Err("package ext.fake: its declaration does not match its hash".to_owned());
        let (agents, base, profile) = service(vec![tampered]);
        let workspace = existing_workspace(&agents, &base, profile.path());
        let kinds = agents.kinds().kinds;
        let fake = kinds.iter().find(|kind| kind.id == "fake").unwrap();
        assert_eq!(
            fake.problems,
            ["package ext.fake: its declaration does not match its hash"]
        );
        assert!(!fake.detected);
        let error = agents
            .start(start_params(&workspace, "fake"), SecretGrant::Provide)
            .expect_err("a tampered kind never starts");
        assert_eq!(error.code, codes::AGENT_NOT_INSTALLED, "{}", error.message);
    }

    #[test]
    fn a_package_declaration_that_does_not_parse_names_the_package() {
        let mut broken = fake_package(PackageState::Enabled, MISSING_PROGRAM);
        broken.decl = Ok("{ not json".to_owned());
        let profile = tempfile::tempdir().unwrap();
        let catalog = Catalog::load(&TestPackages(vec![broken]), profile.path());
        let entry = catalog.find("fake").unwrap();
        assert!(entry.valid().is_none());
        assert!(
            entry.problems[0].starts_with("package ext.fake: "),
            "{:?}",
            entry.problems
        );
    }

    #[test]
    fn a_user_kind_reusing_a_package_id_keeps_todays_precedence() {
        let profile = tempfile::tempdir().unwrap();
        let dir = paths::kinds_dir(profile.path());
        std::fs::create_dir_all(&dir).unwrap();
        for id in ["claude", "fake"] {
            let text = json!({ "id": id, "name": "Mine", "program": ["mine"] });
            std::fs::write(dir.join(format!("{id}.json")), text.to_string()).unwrap();
        }
        let mut kinds = BuiltInPackages.kinds();
        kinds.push(fake_package(PackageState::Disabled, MISSING_PROGRAM));
        let catalog = Catalog::load(&TestPackages(kinds), profile.path());

        assert_eq!(catalog.find("claude").unwrap().owner, OWNER_CORE);
        let fake = catalog.find("fake").unwrap();
        assert_eq!(fake.owner, "ext:ext.fake");
        assert!(!fake.enabled());
        let users: Vec<_> = catalog
            .entries
            .iter()
            .filter(|entry| entry.owner == OWNER_USER)
            .collect();
        assert_eq!(users.len(), 2);
        assert!(
            users[0]
                .problems
                .iter()
                .any(|problem| problem.contains("built-in"))
        );
        assert!(
            users[1]
                .problems
                .iter()
                .any(|problem| problem.contains("already declared by package ext.fake")),
            "{:?}",
            users[1].problems
        );
        assert!(
            users
                .iter()
                .all(|entry| entry.valid().is_none() && entry.enabled())
        );
    }

    #[test]
    fn a_user_kind_has_no_package_and_counts_as_enabled() {
        let profile = tempfile::tempdir().unwrap();
        let dir = paths::kinds_dir(profile.path());
        std::fs::create_dir_all(&dir).unwrap();
        let text = json!({ "id": "mine", "name": "Mine", "program": ["mine"] });
        std::fs::write(dir.join("mine.json"), text.to_string()).unwrap();
        let catalog = Catalog::load(&TestPackages(Vec::new()), profile.path());
        let mine = catalog.find("mine").unwrap();
        assert!(mine.enabled());
        assert_eq!(mine.package.id, None);
        assert_eq!(mine.package.version, None);
    }

    #[test]
    fn a_disabled_kind_refuses_start_resume_and_delegation() {
        // Every program of the disabled kind is missing, so a start past the gate would stop at the program check.
        let mut packages = built_ins_with("claude", PackageState::Disabled);
        for kind in &mut packages.0 {
            if let Ok(text) = &kind.decl {
                let mut value: serde_json::Value = serde_json::from_str(text).unwrap();
                value["program"] = json!([MISSING_PROGRAM]);
                kind.decl = Ok(value.to_string());
            }
        }
        let (agents, base, profile) = service_with(Arc::new(packages));
        let workspace = existing_workspace(&agents, &base, profile.path());
        assert_not_enabled(
            agents.start(start_params(&workspace, "claude"), SecretGrant::Provide),
            "claude",
            "massorca.claude",
            "disabled",
        );
        let earlier = exited_session(&agents, &workspace, "claude");
        assert_not_enabled(
            agents.resume(&earlier.id, None, SecretGrant::Provide),
            "claude",
            "massorca.claude",
            "disabled",
        );
        let parent = exited_session(&agents, &workspace, "codex");
        let params = AgentDelegateParams {
            parent: parent.id.clone(),
            prompt: "do it".to_owned(),
            kind: Some("claude".to_owned()),
            name: None,
            title: None,
            read_only: true,
            token: None,
            request_id: None,
            permission_mode: None,
        };
        assert_not_enabled(
            agents.delegate(params, SecretGrant::Provide),
            "claude",
            "massorca.claude",
            "disabled",
        );
    }

    #[test]
    fn a_disabled_kind_refuses_account_add_import_and_activate_but_checks_and_removes() {
        let (agents, _, profile) =
            service(vec![fake_package(PackageState::Disabled, &this_binary())]);
        assert_not_enabled(
            agents.accounts_add("fake", None),
            "fake",
            "ext.fake",
            "disabled",
        );
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("token.json"), "{}").unwrap();
        assert_not_enabled(
            agents.accounts_import("fake", source.path().to_str().unwrap(), None),
            "fake",
            "ext.fake",
            "disabled",
        );
        let kept = account("fake", "sign_in");
        agents.store().insert_agent_account(&kept).unwrap();
        std::fs::create_dir_all(paths::accounts_dir(profile.path(), "fake").join(&kept.id))
            .unwrap();
        assert_not_enabled(
            agents.accounts_activate("fake", &kept.id, None),
            "fake",
            "ext.fake",
            "disabled",
        );

        let checked = agents
            .accounts_check("fake", Some(&kept.id))
            .expect("a check works on a disabled kind");
        assert_eq!(checked.id, kept.id);
        let removed = agents
            .accounts_remove("fake", &kept.id)
            .expect("a removal works on a disabled kind");
        assert!(removed.removed);
        assert!(
            agents
                .store()
                .get_agent_account(&kept.id)
                .unwrap()
                .is_none()
        );
        assert!(
            !paths::accounts_dir(profile.path(), "fake")
                .join(&kept.id)
                .exists()
        );
    }
}
