//! The tool, skill, view, theme and status-rule points (CUS-08, decision 0153). Each is a [`Registry`]; an enabled
//! package's contributions are registered under its owner, and the handles are kept per owner, so a removal, a disable
//! or an update drops everything the package registered at once.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use massorca_core_extensions::Extensions;
use massorca_core_extensions::contributions::{Contributes, EnabledPackage};
use massorca_protocol::codes::EXTENSION_POINT_REFUSED as POINT_REFUSED;

use crate::owner::Owner;
use crate::registry::{Entry, RegistrationError, RegistrationHandle, Registry};

/// Where an agent tool comes from: a tool the package serves over MCP, or a host program it installs (0151).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSource {
    Mcp { description: String },
    HostProgram { binary: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPoint {
    pub source: ToolSource,
}

/// A skill folder; placement (SKL-02) copies it into the agents' folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPoint {
    pub title: String,
    pub directory: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewPoint {
    pub slot: String,
    pub title: String,
    pub document: PathBuf,
}

/// A colour theme or, with `icons`, an icon theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePoint {
    pub title: String,
    pub icons: bool,
    pub document: PathBuf,
}

/// A declarative agent-state rule document (`agentRules`, CUS-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRulePoint {
    pub document: PathBuf,
}

/// The handles one package holds, and the folder they were read from, so an update re-registers.
#[derive(Debug)]
struct Owned {
    root: PathBuf,
    _handles: Vec<RegistrationHandle>,
}

/// A package whose contributions are not registered, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointProblem {
    pub id: String,
    pub code: String,
    pub message: String,
}

/// What one owner has registered on each point, by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnerPoints {
    pub tools: Vec<String>,
    pub skills: Vec<String>,
    pub views: Vec<String>,
    pub themes: Vec<String>,
    pub status_rules: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ExtensionPoints {
    pub tools: Registry<ToolPoint>,
    pub skills: Registry<SkillPoint>,
    pub views: Registry<ViewPoint>,
    pub themes: Registry<ThemePoint>,
    pub status_rules: Registry<StatusRulePoint>,
    owners: Mutex<BTreeMap<String, Owned>>,
    problems: Mutex<Vec<PointProblem>>,
}

impl ExtensionPoints {
    /// Brings the points in line with the enabled packages: a package gone, disabled or moved to another folder loses
    /// its registrations, and one not yet registered gains them. A package that cannot register keeps none.
    pub fn sync(&self, packages: &[EnabledPackage]) {
        let mut owners = lock(&self.owners);
        owners.retain(|id, owned| {
            packages
                .iter()
                .any(|package| &package.id == id && package.root == owned.root)
        });
        let mut problems = Vec::new();
        for package in packages {
            if owners.contains_key(&package.id) {
                continue;
            }
            match self.register_package(package) {
                Ok(handles) => {
                    owners.insert(
                        package.id.clone(),
                        Owned {
                            root: package.root.clone(),
                            _handles: handles,
                        },
                    );
                }
                Err(problem) => problems.push(problem),
            }
        }
        drop(owners);
        *lock(&self.problems) = problems;
    }

    /// Drops every registration of `id`; the remove listener calls it after the lockfile commit.
    pub fn drop_owner(&self, id: &str) {
        let dropped = lock(&self.owners).remove(id);
        drop(dropped);
        lock(&self.problems).retain(|problem| problem.id != id);
    }

    /// What each owner registered, by owner wire form (`core` or `ext:<id>`).
    pub fn by_owner(&self) -> BTreeMap<String, OwnerPoints> {
        let mut out: BTreeMap<String, OwnerPoints> = BTreeMap::new();
        add(&mut out, &self.tools, |points| &mut points.tools);
        add(&mut out, &self.skills, |points| &mut points.skills);
        add(&mut out, &self.views, |points| &mut points.views);
        add(&mut out, &self.themes, |points| &mut points.themes);
        add(&mut out, &self.status_rules, |points| {
            &mut points.status_rules
        });
        out
    }

    /// The packages the last sync could not register.
    pub fn problems(&self) -> Vec<PointProblem> {
        lock(&self.problems).clone()
    }

    fn register_package(
        &self,
        package: &EnabledPackage,
    ) -> Result<Vec<RegistrationHandle>, PointProblem> {
        let problem = |code: &str, message: String| PointProblem {
            id: package.id.clone(),
            code: code.to_owned(),
            message,
        };
        let manifest = package.manifest.as_ref().map_err(|code| {
            problem(
                code,
                format!("the files of {} cannot be trusted", package.id),
            )
        })?;
        self.register_contributions(package, &manifest.contributes)
            .map_err(|error| problem(POINT_REFUSED, error.to_string()))
    }

    fn register_contributions(
        &self,
        package: &EnabledPackage,
        contributes: &Contributes,
    ) -> Result<Vec<RegistrationHandle>, RegistrationError> {
        let owner = Owner::Extension(package.id.clone());
        let key = |id: &str| format!("{}/{id}", package.id);
        let file = |path: &str| {
            path.split('/')
                .fold(package.root.clone(), |at, part| at.join(part))
        };
        // Dropped on the first refusal, so a package registers all of its contributions or none.
        let mut handles = Vec::new();
        for tool in &contributes.tools {
            let source = ToolSource::Mcp {
                description: tool.description.clone(),
            };
            handles.push(self.tools.register(
                owner.clone(),
                key(&tool.id),
                ToolPoint { source },
            )?);
        }
        for program in &contributes.host_programs {
            let source = ToolSource::HostProgram {
                binary: program.cli.binary.clone(),
            };
            handles.push(self.tools.register(
                owner.clone(),
                key(&program.id),
                ToolPoint { source },
            )?);
        }
        for skill in &contributes.skills {
            let value = SkillPoint {
                title: skill.title.clone(),
                directory: file(&skill.path),
            };
            handles.push(self.skills.register(owner.clone(), key(&skill.id), value)?);
        }
        for view in &contributes.views {
            let value = ViewPoint {
                slot: slot_name(&view.slot),
                title: view.title.clone(),
                document: file(&view.document),
            };
            handles.push(self.views.register(owner.clone(), key(&view.id), value)?);
        }
        for (icons, themes) in [
            (false, &contributes.themes),
            (true, &contributes.icon_themes),
        ] {
            for theme in themes {
                let value = ThemePoint {
                    title: theme.title.clone(),
                    icons,
                    document: file(&theme.document),
                };
                handles.push(self.themes.register(owner.clone(), key(&theme.id), value)?);
            }
        }
        for rule in &contributes.agent_rules {
            let value = StatusRulePoint {
                document: file(&rule.document),
            };
            handles.push(
                self.status_rules
                    .register(owner.clone(), key(&rule.id), value)?,
            );
        }
        Ok(handles)
    }
}

/// Syncs `points` with the profile's enabled packages. Blocking: it hashes each package's files. An unreadable lockfile
/// keeps the current registrations, since nothing says which packages changed.
pub(crate) fn sync_extension_points(points: &ExtensionPoints, extensions: &Extensions) {
    match extensions.enabled_packages() {
        Ok(packages) => points.sync(&packages),
        Err(error) => tracing::warn!(%error, "extension points were not synced"),
    }
}

/// The slot's manifest name, such as `status-item`.
fn slot_name(slot: &impl serde::Serialize) -> String {
    serde_json::to_value(slot)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn add<K>(
    out: &mut BTreeMap<String, OwnerPoints>,
    registry: &Registry<K>,
    field: impl Fn(&mut OwnerPoints) -> &mut Vec<String>,
) {
    for entry in registry.list() {
        let Entry { key, owner, .. } = &*entry;
        field(out.entry(owner.to_string()).or_default()).push(key.clone());
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use massorca_core_extensions::contributions::EnabledPackage;
    use serde_json::json;

    use super::{ExtensionPoints, OwnerPoints, ToolSource};

    /// A package `acme.all` with one contribution on every point.
    fn every_point(root: &Path) -> EnabledPackage {
        let manifest = json!({
            "id": "acme.all", "name": "All", "version": "1.0.0", "publisher": "acme",
            "description": "Everything.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
            "views": 1, "surfaces": ["desktop"], "modes": ["workspaces"], "main": "main.wasm",
            "contributes": {
                "tools": [{ "id": "search", "description": "Searches.", "input": "tools/search.json" }],
                "hostPrograms": [{ "id": "kit", "cli": { "binary": "kit", "source": "https://vendor.example" } }],
                "skills": [{ "id": "review", "title": "Review", "path": "skills/review" }],
                "views": [{ "id": "wall", "slot": "status-item", "title": "Wall", "document": "views/wall.json" }],
                "themes": [{ "id": "dusk", "title": "Dusk", "document": "themes/dusk.json" }],
                "iconThemes": [{ "id": "glyphs", "title": "Glyphs", "document": "themes/glyphs.json" }],
                "agentRules": [{ "id": "waiting", "document": "rules/waiting.json" }]
            }
        });
        let manifest = massorca_ext_package::manifest::parse(manifest.to_string().as_bytes())
            .expect("the manifest parses");
        EnabledPackage {
            id: "acme.all".to_owned(),
            version: "1.0.0".to_owned(),
            root: root.to_path_buf(),
            manifest: Ok(manifest),
        }
    }

    fn keys(points: &[&str]) -> Vec<String> {
        points.iter().map(|key| (*key).to_owned()).collect()
    }

    fn everything() -> OwnerPoints {
        OwnerPoints {
            tools: keys(&["acme.all/kit", "acme.all/search"]),
            skills: keys(&["acme.all/review"]),
            views: keys(&["acme.all/wall"]),
            themes: keys(&["acme.all/dusk", "acme.all/glyphs"]),
            status_rules: keys(&["acme.all/waiting"]),
        }
    }

    #[test]
    fn a_package_registers_on_every_point_and_its_removal_empties_them() {
        let points = ExtensionPoints::default();
        let root = PathBuf::from("pkgs").join("acme.all");
        points.sync(&[every_point(&root)]);

        let owners = points.by_owner();
        assert_eq!(owners.len(), 1);
        assert_eq!(owners["ext:acme.all"], everything());
        let tool = points.tools.get("acme.all/kit").expect("the host program");
        assert_eq!(
            tool.value.source,
            ToolSource::HostProgram {
                binary: "kit".to_owned()
            }
        );
        let skill = points.skills.get("acme.all/review").expect("the skill");
        assert_eq!(skill.value.directory, root.join("skills").join("review"));
        assert_eq!(
            points
                .views
                .get("acme.all/wall")
                .expect("the view")
                .value
                .slot,
            "status-item"
        );
        assert!(
            points
                .themes
                .get("acme.all/glyphs")
                .expect("icons")
                .value
                .icons
        );

        points.drop_owner("acme.all");
        assert!(points.by_owner().is_empty());
        assert!(points.tools.list().is_empty());
        assert!(points.skills.list().is_empty());
        assert!(points.views.list().is_empty());
        assert!(points.themes.list().is_empty());
        assert!(points.status_rules.list().is_empty());
    }

    #[test]
    fn sync_drops_a_package_no_longer_enabled_and_reregisters_an_update() {
        let points = ExtensionPoints::default();
        points.sync(&[every_point(Path::new("v1"))]);
        let before = points.skills.get("acme.all/review").expect("v1");
        assert_eq!(
            before.value.directory,
            Path::new("v1").join("skills").join("review")
        );

        points.sync(&[every_point(Path::new("v2"))]);
        let after = points.skills.get("acme.all/review").expect("v2");
        assert_eq!(
            after.value.directory,
            Path::new("v2").join("skills").join("review")
        );

        points.sync(&[]);
        assert!(points.by_owner().is_empty());
    }

    #[test]
    fn a_package_whose_files_cannot_be_trusted_registers_nothing_and_is_reported() {
        let points = ExtensionPoints::default();
        let mut package = every_point(Path::new("v1"));
        package.manifest = Err("extension.tampered".to_owned());
        points.sync(&[package]);

        assert!(points.by_owner().is_empty());
        let problems = points.problems();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].id, "acme.all");
        assert_eq!(problems[0].code, "extension.tampered");

        points.drop_owner("acme.all");
        assert!(points.problems().is_empty());
    }

    #[test]
    fn a_refused_contribution_rolls_back_the_rest_of_its_package() {
        let points = ExtensionPoints::default();
        let mut package = every_point(Path::new("v1"));
        package.id = "Not Valid".to_owned();
        points.sync(&[package]);

        assert!(points.by_owner().is_empty());
        assert_eq!(points.problems()[0].code, "extension.point_refused");
    }
}
