//! `massorca extension doctor`: what detection sees for a package's CLIs, and the PATH source it searched (decision 0092).

mod extension_support;
mod harness;

use extension_support::Host;

const CODEX_ID: &str = "massorca.codex";

#[test]
fn doctor_reports_a_cli_that_detection_finds_and_the_path_source() {
    let host = Host::new();
    host.put_cli("codex");

    let item = host.json(&["extension", "doctor", CODEX_ID]);

    let agent = &item["agents"][0];
    assert_eq!(agent["detected"], true, "{item}");
    assert_eq!(agent["path_source"], "process", "{item}");
}

#[test]
fn doctor_says_not_found_and_names_the_path_source_it_searched() {
    let host = Host::new();

    let output = host.run(&["extension", "doctor", CODEX_ID]).success();
    let text = String::from_utf8_lossy(&output.get_output().stdout).into_owned();

    assert!(text.contains("codex"), "{text}");
    assert!(text.contains("not found"), "{text}");
    assert!(text.contains("PATH source: process"), "{text}");
}
