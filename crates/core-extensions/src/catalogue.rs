//! The built-in catalogue, embedded in the binary: the first-party agent packages under `packages/agents/` and the tool
//! packages under `packages/tools/`. Installing
//! one copies it from here and downloads nothing.

use std::collections::BTreeMap;

/// One catalogue package: its id and every file, keyed by package path.
#[derive(Debug, Clone, Copy)]
pub struct CatalogueEntry {
    pub id: &'static str,
    pub files: &'static [(&'static str, &'static [u8])],
}

impl CatalogueEntry {
    /// The package's files, as the install flow reads them.
    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        self.files
            .iter()
            .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
            .collect()
    }

    pub fn file(&self, path: &str) -> Option<&'static [u8]> {
        self.files
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| *bytes)
    }
}

macro_rules! agent_package {
    ($id:literal, $kind:literal) => {
        CatalogueEntry {
            id: concat!("massorca.", $id),
            files: &[
                (
                    "extension.json",
                    include_bytes!(concat!(
                        "../../../packages/agents/massorca.",
                        $id,
                        "/extension.json"
                    )),
                ),
                (
                    concat!("agents/", $kind, ".json"),
                    include_bytes!(concat!(
                        "../../../packages/agents/massorca.",
                        $id,
                        "/agents/",
                        $kind,
                        ".json"
                    )),
                ),
            ],
        }
    };
}

/// A tool package: one manifest naming its host program (decision 0151).
macro_rules! tool_package {
    ($id:literal) => {
        CatalogueEntry {
            id: concat!("massorca.", $id),
            files: &[(
                "extension.json",
                include_bytes!(concat!(
                    "../../../packages/tools/massorca.",
                    $id,
                    "/extension.json"
                )),
            )],
        }
    };
}

static ENTRIES: [CatalogueEntry; 7] = [
    agent_package!("claude", "claude"),
    agent_package!("codex", "codex"),
    agent_package!("antigravity", "antigravity"),
    agent_package!("gentle-shell", "gentle-shell"),
    tool_package!("git"),
    tool_package!("gh"),
    tool_package!("node"),
];

/// Every catalogue entry, in display order.
pub fn entries() -> &'static [CatalogueEntry] {
    &ENTRIES
}

/// The catalogue entry with this package id.
pub fn find(id: &str) -> Option<&'static CatalogueEntry> {
    ENTRIES.iter().find(|entry| entry.id == id)
}

/// Whether a lockfile entry is the built-in catalogue's own install: a catalogue source with the same id, and the
/// content hash the embedded package has now. Anything else under a first-party id is not the built-in.
pub(crate) fn is_builtin_install(entry: &crate::lockfile::LockEntry) -> bool {
    entry.source.kind == crate::lockfile::SourceKind::Catalogue
        && entry.source.reference == entry.id
        && find(&entry.id).is_some_and(|catalogue| {
            crate::source::canonical_sha256(&catalogue.files()).eq_ignore_ascii_case(&entry.sha256)
        })
}

/// A built-in catalogue install whose files predate the package this build embeds: same source and id, another hash.
/// [`crate::Extensions::refresh_builtins`] brings it up to date.
pub(crate) fn is_outdated_builtin_install(entry: &crate::lockfile::LockEntry) -> bool {
    entry.source.kind == crate::lockfile::SourceKind::Catalogue
        && entry.source.reference == entry.id
        && find(&entry.id).is_some()
        && !is_builtin_install(entry)
}

/// A lockfile entry under a first-party id (publisher `massorca` or `massorca.*`) that the built-in catalogue does not
/// vouch for. It is never shown as a built-in, never runs its kinds, and never blocks a built-in install.
pub(crate) fn is_unvouched_first_party(entry: &crate::lockfile::LockEntry) -> bool {
    massorca_ext_package::ids::publisher_of(&entry.id)
        == Some(massorca_ext_package::manifest::agent::RESERVED_PUBLISHER)
        && !is_builtin_install(entry)
}

#[cfg(test)]
mod tests {
    use massorca_ext_package::manifest::{self, check_files_present};

    use super::*;

    #[test]
    fn every_catalogue_package_is_valid_and_complete() {
        for entry in entries() {
            let bytes = entry.file("extension.json").expect("manifest");
            let parsed = manifest::parse(bytes).expect("shape");
            assert_eq!(manifest::validate(&parsed), Vec::new(), "{}", entry.id);
            let loaded = manifest::load(bytes).expect("valid");
            assert_eq!(loaded.id, entry.id);
            check_files_present(&loaded, entry.files.iter().map(|(path, _)| *path))
                .expect("every referenced file is embedded");
            for adapter in &loaded.contributes.agent_adapters {
                assert!(adapter.cli.is_some(), "{} has a cli block", adapter.id);
            }
        }
    }

    #[test]
    fn agent_declarations_equal_the_core_agents_copies() {
        let core_agents: [(&str, &[u8]); 4] = [
            (
                "claude",
                include_bytes!("../../core-agents/kinds/claude.json"),
            ),
            (
                "codex",
                include_bytes!("../../core-agents/kinds/codex.json"),
            ),
            (
                "antigravity",
                include_bytes!("../../core-agents/kinds/antigravity.json"),
            ),
            (
                "gentle-shell",
                include_bytes!("../../core-agents/kinds/gentle-shell.json"),
            ),
        ];
        for (kind, expected) in core_agents {
            let entry = find(&format!("massorca.{kind}")).expect("catalogue entry");
            assert_eq!(
                entry.file(&format!("agents/{kind}.json")),
                Some(expected),
                "{kind}"
            );
        }
    }

    /// The Gentle Shell package installs with npm and uninstalls through its own CLI (decision 0164): the vendor's
    /// `self-uninstall` removes its data and the npm package, and MassOrca runs it only for a CLI it installed.
    #[test]
    fn the_gentle_shell_package_uninstalls_through_its_own_cli() {
        use massorca_ext_package::manifest::agent::CliOs;

        let bytes = find("massorca.gentle-shell")
            .and_then(|entry| entry.file("extension.json"))
            .expect("the package is in the catalogue");
        let loaded = manifest::load(bytes).expect("valid");
        let cli = loaded.contributes.agent_adapters[0]
            .cli
            .as_ref()
            .expect("a cli block");
        for os in [CliOs::Windows, CliOs::Linux, CliOs::Macos] {
            let install = cli
                .install
                .iter()
                .find(|step| step.os == os)
                .expect("an install step");
            assert_eq!(install.command, ["npm", "install", "-g", "gentle-pi"]);
            let uninstall = cli
                .uninstall
                .iter()
                .find(|step| step.os == os)
                .expect("an uninstall step");
            assert_eq!(
                uninstall.command,
                ["gentle-shell", "self-uninstall", "--yes"]
            );
        }
    }

    /// A piped download must fail the step when the download fails, not run what arrived.
    #[test]
    fn every_shell_script_step_fails_on_a_failed_pipe() {
        for entry in entries() {
            let bytes = entry.file("extension.json").expect("manifest");
            let loaded = manifest::load(bytes).expect("valid");
            for (_, cli) in loaded.program_clis() {
                for step in cli.install.iter().chain(&cli.uninstall) {
                    if let [shell, flag, script] = step.command.as_slice()
                        && matches!(shell.as_str(), "bash" | "sh")
                        && flag == "-c"
                    {
                        assert!(
                            script.starts_with("set -o pipefail;"),
                            "{}: {script}",
                            entry.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn find_knows_only_catalogue_ids() {
        assert!(find("massorca.claude").is_some());
        assert!(find("acme.kit").is_none());
    }
}
