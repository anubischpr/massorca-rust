//! The `test-hooks` feature (test-only crash points, `MASSORCA_TEST_CRASH_AT`) never reaches a release build: only
//! dev-dependency sections may enable it, and only core-agents declares it. A normal or build dependency, or another
//! crate's feature that forwards it, would put the crash points in every binary that depends on it.

use std::fs;
use std::path::{Path, PathBuf};

const FEATURE: &str = "test-hooks";

/// The workspace's manifests: the root one and every crate's.
fn manifests() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = vec![root.join("Cargo.toml")];
    let crates = fs::read_dir(root.join("crates")).expect("the crates folder");
    for entry in crates.flatten() {
        let manifest = entry.path().join("Cargo.toml");
        if manifest.is_file() {
            out.push(manifest);
        }
    }
    out
}

/// Where `test-hooks` may appear: a dev-dependency section, or the feature's own declaration in core-agents.
fn allowed(manifest: &Path, section: &str, line: &str) -> bool {
    let dev = section == "dev-dependencies" || section.ends_with(".dev-dependencies");
    let declaration = section == "features"
        && manifest.ends_with(Path::new("core-agents").join("Cargo.toml"))
        && line.trim_start().starts_with(&format!("{FEATURE} ="));
    dev || declaration
}

#[test]
fn only_dev_dependencies_enable_the_test_hooks_feature() {
    let mut offending = Vec::new();
    let mut checked = 0;
    for manifest in manifests() {
        let text = fs::read_to_string(&manifest).expect("a manifest");
        checked += 1;
        let mut section = String::new();
        for (number, line) in text.lines().enumerate() {
            let code = line.split('#').next().unwrap_or_default().trim();
            if let Some(header) = code
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                section = header.trim_matches(['[', ']']).to_owned();
                continue;
            }
            if code.contains(FEATURE) && !allowed(&manifest, &section, code) {
                offending.push(format!(
                    "{}:{} [{section}] {code}",
                    manifest.display(),
                    number + 1
                ));
            }
        }
    }
    assert!(checked > 10, "only {checked} manifests were found");
    assert!(
        offending.is_empty(),
        "`{FEATURE}` is enabled outside dev-dependencies, so a release build would carry the crash points:\n{}",
        offending.join("\n")
    );
}

#[test]
fn the_check_flags_a_normal_dependency_and_allows_dev_dependencies() {
    let cli = Path::new("crates/cli/Cargo.toml");
    let line = r#"massorca-core-agents = { workspace = true, features = ["test-hooks"] }"#;
    assert!(allowed(cli, "dev-dependencies", line));
    assert!(allowed(cli, "target.'cfg(unix)'.dev-dependencies", line));
    assert!(!allowed(cli, "dependencies", line));
    assert!(!allowed(cli, "build-dependencies", line));
    assert!(!allowed(
        cli,
        "features",
        r#"hooks = ["massorca-core-agents/test-hooks"]"#
    ));
    let agents = Path::new("crates/core-agents/Cargo.toml");
    assert!(allowed(agents, "features", "test-hooks = []"));
    assert!(!allowed(agents, "features", r#"default = ["test-hooks"]"#));
}
