//! A host for the extension tests: a sandboxed service whose `PATH` is one temporary folder, test packages written
//! as folders or archives, and a fake vendor program that stands in for every CLI and installer.
//!
//! No test runs a real vendor installer: each package's install, verify and uninstall commands point at
//! `massorca-fake-vendor` (`examples/massorca-fake-vendor.rs`), and the package ids and agent ids are not the reserved ones.
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use assert_cmd::assert::Assert;
use serde_json::{Value, json};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

use crate::harness::{Sandbox, execute, single_document, with_json};

/// The fake vendor program every test package runs. It is the example `massorca-fake-vendor`, not a shipped binary, so
/// `cargo test` does not build it: the first call builds it into the same target folder as the test binaries
/// (decision 0062).
pub fn fake_vendor() -> String {
    static BUILT: std::sync::Once = std::sync::Once::new();
    BUILT.call_once(|| {
        let status = std::process::Command::new(env!("CARGO"))
            .args([
                "build",
                "--locked",
                "-p",
                "massorca-cli",
                "--example",
                "massorca-fake-vendor",
            ])
            .status()
            .expect("cargo runs");
        assert!(status.success(), "the fake vendor example builds");
    });
    let test_binary = std::env::current_exe().expect("the test binary");
    let profile = test_binary
        .parent()
        .and_then(Path::parent)
        .expect("the profile folder");
    profile
        .join("examples")
        .join(format!(
            "massorca-fake-vendor{}",
            std::env::consts::EXE_SUFFIX
        ))
        .to_string_lossy()
        .into_owned()
}

/// The program every vendor command of a test package names: an allowlisted program (0062, second review). The bin folder
/// holds a copy of the fake vendor under this name, so the command runs the fake and never a real installer.
pub const VENDOR_PROGRAM: &str = "npm";
/// The package managers the built-ins name; the bin folder holds the fake vendor under each of them.
pub const VENDOR_PROGRAMS: [&str; 5] = ["npm", "winget", "brew", "powershell", "bash"];

/// A fresh bin folder holding the fake vendor as `VENDOR_PROGRAM`.
fn bin_with_npm() -> tempfile::TempDir {
    let bin = tempfile::tempdir().expect("a bin folder");
    for program in VENDOR_PROGRAMS {
        let name = format!("{program}{}", std::env::consts::EXE_SUFFIX);
        fs::copy(fake_vendor(), bin.path().join(name))
            .expect("the fake vendor is copied as a package manager");
    }
    bin
}

/// The package `acme.kit`, whose agent is `kitagent` and whose CLI is `acmecli`.
pub const KIT: Package = Package::new("acme.kit", "kitagent", "acmecli");

/// A test package: the id, the agent it contributes and the CLI binary that agent needs.
#[derive(Debug, Clone, Copy)]
pub struct Package {
    pub id: &'static str,
    pub agent: &'static str,
    pub binary: &'static str,
}

impl Package {
    pub const fn new(id: &'static str, agent: &'static str, binary: &'static str) -> Self {
        Self { id, agent, binary }
    }

    /// The manifest. A test package declares no vendor command (0062, V1): its CLI is only detected, or it is missing.
    pub fn manifest(&self, _bin: &Path, _install: &[&str]) -> Value {
        json!({
            "id": self.id, "name": "Kit", "version": "1.2.0", "publisher": "acme",
            "description": "A test coding agent.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
            "surfaces": ["desktop"], "modes": ["workspaces"],
            "permissions": [
                { "kind": "process", "scope": { "programs": [self.binary] }, "reason": "Runs the agent." }
            ],
            "contributes": { "agentAdapters": [{
                "id": self.agent, "kind": format!("agents/{}.json", self.agent),
                "cli": {
                    "binary": self.binary, "source": "https://vendor.example/install"
                }
            }] },
            "listing": { "category": "agents", "mention": self.agent }
        })
    }

    /// The files of the package.
    pub fn files(&self, manifest: &Value) -> Vec<(String, Vec<u8>)> {
        vec![
            (
                "extension.json".to_owned(),
                manifest.to_string().into_bytes(),
            ),
            (
                format!("agents/{}.json", self.agent),
                format!("{{\"id\":\"{}\"}}", self.agent).into_bytes(),
            ),
        ]
    }
}

/// A sandboxed service and the folders of one test.
#[derive(Debug)]
pub struct Host {
    pub sandbox: Sandbox,
    /// The only folder on the service's `PATH`. Fake CLIs live here, and the fake installer puts them here.
    pub bin: TempDir,
    /// Where test packages are written.
    pub packages: TempDir,
}

impl Host {
    pub fn new() -> Self {
        Self {
            sandbox: Sandbox::new(),
            bin: bin_with_npm(),
            packages: tempfile::tempdir().expect("a packages folder"),
        }
    }

    /// Runs `massorca <args>` with `PATH` set to the bin folder. The service starts on the first call, so it sees
    /// that `PATH` and no other.
    pub fn run(&self, args: &[&str]) -> Assert {
        let mut command = self.sandbox.command("default");
        command.env("PATH", self.bin.path()).args(args);
        execute(&mut command)
    }

    /// The `result` of a command that must succeed, run with `--json`.
    pub fn json(&self, args: &[&str]) -> Value {
        let outcome = self.run(&with_json(args)).success();
        single_document(&outcome)["result"].clone()
    }

    /// The error of a command that must fail with exit code 1, run with `--json` and extra environment variables,
    /// such as the ones an agent's run sets.
    pub fn json_error_with_env(&self, args: &[&str], env: &[(&str, &str)]) -> Value {
        let mut command = self.sandbox.command("default");
        let args = with_json(args);
        command
            .env("PATH", self.bin.path())
            .envs(env.iter().copied())
            .args(&args);
        let document = single_document(&execute(&mut command).code(1));
        assert_eq!(document["ok"], false, "{document}");
        document["error"].clone()
    }

    /// The error of a command that must fail with exit code 1, run with `--json`.
    pub fn json_error(&self, args: &[&str]) -> Value {
        let outcome = self.run(&with_json(args)).code(1);
        let document = single_document(&outcome);
        assert_eq!(document["ok"], false, "{document}");
        document["error"].clone()
    }

    /// Puts a copy of the fake vendor program on the service's `PATH` under the name of a CLI.
    pub fn put_cli(&self, binary: &str) {
        fs::copy(fake_vendor(), self.cli_path(binary)).expect("the fake CLI is copied");
    }

    pub fn cli_path(&self, binary: &str) -> PathBuf {
        self.bin
            .path()
            .join(format!("{binary}{}", std::env::consts::EXE_SUFFIX))
    }

    pub fn has_cli(&self, binary: &str) -> bool {
        self.cli_path(binary).exists()
    }

    /// A file the fake vendor appends to when a command runs, and where it would be.
    pub fn marker(&self) -> PathBuf {
        self.packages.path().join("ran.txt")
    }

    /// Writes `package` as a folder named after its id, and returns its `dir:` source.
    pub fn write_dir(&self, package: Package, install: &[&str]) -> String {
        let manifest = package.manifest(self.bin.path(), install);
        let dir = self.packages.path().join(package.id);
        for (path, bytes) in package.files(&manifest) {
            let target = dir.join(&path);
            fs::create_dir_all(target.parent().expect("a parent")).expect("a folder");
            fs::write(target, bytes).expect("a package file");
        }
        format!("dir:{}", dir.display())
    }

    /// Writes `package` as an archive, and returns its `file:` source.
    pub fn write_archive(&self, package: Package, install: &[&str]) -> String {
        let manifest = package.manifest(self.bin.path(), install);
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, bytes) in package.files(&manifest) {
            writer
                .start_file(path.as_str(), SimpleFileOptions::default())
                .expect("an archive entry");
            writer.write_all(&bytes).expect("the entry's bytes");
        }
        let bytes = writer.finish().expect("the archive").into_inner();
        let file = self.packages.path().join(format!("{}.zip", package.id));
        fs::write(&file, bytes).expect("the archive file");
        format!("file:{}", file.display())
    }

    /// `<profile>/extensions`.
    pub fn extensions_dir(&self) -> PathBuf {
        self.sandbox.profile_dir("default").join("extensions")
    }

    /// The ids of the installed packages.
    pub fn installed_ids(&self) -> Vec<String> {
        self.json(&["extension", "list"])["extensions"]
            .as_array()
            .expect("an extensions array")
            .iter()
            .map(|item| item["id"].as_str().expect("an id").to_owned())
            .collect()
    }
}
