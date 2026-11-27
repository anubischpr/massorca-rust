//! The enabled packages the extension points read, and the remove listeners that drop what a package registered.

use std::sync::{Arc, Mutex};

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::setup;

#[test]
fn only_enabled_packages_are_listed_with_their_trusted_manifest() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, false).expect("installs");

    let enabled = s.service.enabled_packages().expect("lists");
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id, "acme.kit");
    assert_eq!(enabled[0].version, "1.2.0");
    assert!(enabled[0].root.join("extension.json").is_file());
    let manifest = enabled[0].manifest.as_ref().expect("trusted");
    assert_eq!(manifest.contributes.agent_adapters[0].id, "kit");

    s.service.set_enabled("acme.kit", false).expect("disables");
    assert!(s.service.enabled_packages().expect("lists").is_empty());
}

#[test]
fn a_changed_file_is_reported_instead_of_its_manifest() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, false).expect("installs");
    let root = s.service.enabled_packages().expect("lists")[0].root.clone();
    std::fs::write(root.join("agents/kit.json"), "{}").expect("tamper");

    let enabled = s.service.enabled_packages().expect("lists");
    assert_eq!(
        enabled[0].manifest.as_ref().expect_err("refused"),
        "extension.tampered"
    );
}

#[test]
fn listeners_hear_every_removal_and_stop_once_dropped() {
    let heard = Arc::new(Mutex::new(Vec::new()));
    for delete_agent_homes in [false, true] {
        let s = setup(FakeHost::new().with_program("kit"));
        let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
        s.install(&source, false).expect("installs");
        let log = heard.clone();
        let listener = s
            .service
            .register_remove_listener("points", move |removed| {
                log.lock().expect("heard").push(removed.id.clone());
            });
        s.service
            .remove("acme.kit", delete_agent_homes, false, None)
            .expect("removes");
        drop(listener);
        s.install(&source, false).expect("installs again");
        s.service
            .remove("acme.kit", delete_agent_homes, false, None)
            .expect("removes again");
    }
    assert_eq!(*heard.lock().expect("heard"), ["acme.kit", "acme.kit"]);
}
