//! Test packages, written as folders or archives.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use serde_json::{Value, json};
use zip::write::SimpleFileOptions;

/// An agent package `<publisher>.kit` contributing the agent `agent`, whose CLI `binary` has an installer and an
/// declares no vendor command: only a built-in may (0062, V1), so this package only registers.
pub fn agent_package(publisher: &str, agent: &str, binary: &str) -> Value {
    json!({
        "id": format!("{publisher}.kit"), "name": "Kit", "version": "1.2.0", "publisher": publisher,
        "description": "A coding agent.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
        "surfaces": ["desktop"], "modes": ["workspaces"],
        "permissions": [
            { "kind": "process", "scope": { "programs": [binary] }, "reason": "Runs the agent." },
            { "kind": "credentials", "scope": { "agents": [agent] }, "reason": "Keeps its sign-ins." }
        ],
        "contributes": { "agentAdapters": [{
            "id": agent, "kind": format!("agents/{agent}.json"),
            "cli": {
                "binary": binary, "source": "https://vendor.example/install"
            }
        }] },
        "listing": {
            "logo": "media/logo.svg", "mention": agent,
            "banner": { "image": "media/banner.png",
                        "examples": [{ "kind": "mention", "text": format!("@{agent}"), "caption": "Ask it" }] },
            "connections": [{ "id": "account", "kind": "agent-login", "title": "Account", "multiple": true }]
        }
    })
}

/// Every file of a package: the manifest, the agent declarations it names and the listed media.
pub fn package_files(manifest: &Value) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    files.insert(
        "extension.json".to_owned(),
        serde_json::to_vec_pretty(manifest).expect("manifest serializes"),
    );
    for adapter in manifest["contributes"]["agentAdapters"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let id = adapter["id"].as_str().expect("adapter id");
        files.insert(
            format!("agents/{id}.json"),
            format!("{{\"id\":\"{id}\"}}").into_bytes(),
        );
    }
    files.insert("media/logo.svg".to_owned(), b"<svg/>".to_vec());
    files.insert("media/banner.png".to_owned(), vec![0x89, b'P', b'N', b'G']);
    files.insert("notes/private.txt".to_owned(), b"not listed".to_vec());
    files
}

/// Writes the files as a package folder.
pub fn write_dir(root: &Path, files: &BTreeMap<String, Vec<u8>>) {
    for (path, bytes) in files {
        let target = root.join(path);
        fs::create_dir_all(target.parent().expect("parent")).expect("create folder");
        fs::write(target, bytes).expect("write file");
    }
}

/// Builds a package archive in memory.
pub fn zip(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in files {
        writer
            .start_file(path.as_str(), SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(bytes).expect("contents");
    }
    writer.finish().expect("finish").into_inner()
}
