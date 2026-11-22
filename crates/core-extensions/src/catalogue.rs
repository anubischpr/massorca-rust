//! The built-in catalogue, embedded in the binary: the first-party agent packages under `packages/agents/`. Installing
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

static ENTRIES: [CatalogueEntry; 4] = [
    agent_package!("claude", "claude"),
    agent_package!("codex", "codex"),
    agent_package!("antigravity", "antigravity"),
    agent_package!("gentle-shell", "gentle-shell"),
];

/// Every catalogue entry, in display order.
pub fn entries() -> &'static [CatalogueEntry] {
    &ENTRIES
}

/// The catalogue entry with this package id.
pub fn find(id: &str) -> Option<&'static CatalogueEntry> {
    ENTRIES.iter().find(|entry| entry.id == id)
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

    /// A piped download must fail the step when the download fails, not run what arrived.
    #[test]
    fn every_shell_script_step_fails_on_a_failed_pipe() {
        for entry in entries() {
            let bytes = entry.file("extension.json").expect("manifest");
            let loaded = manifest::load(bytes).expect("valid");
            for cli in loaded
                .contributes
                .agent_adapters
                .iter()
                .filter_map(|adapter| adapter.cli.as_ref())
            {
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
