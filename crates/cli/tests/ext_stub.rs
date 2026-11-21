//! `massorca ext ...` is reserved: every form fails with exit 1 and never touches the service.

mod harness;

use harness::{Sandbox, single_document};
use predicates::str::contains;

#[test]
fn ext_list_exits_1_with_extension_not_implemented() {
    let sandbox = Sandbox::new();

    sandbox
        .run("default", &["ext", "list"])
        .code(1)
        .stderr(contains("extension.not_implemented"))
        .stderr(contains("extensions arrive in milestone M6"));
}

#[test]
fn every_ext_form_fails_the_same_way_without_starting_the_service() {
    let sandbox = Sandbox::new();

    for args in [&["ext"][..], &["ext", "install", "widget", "--force"][..]] {
        sandbox
            .run("default", args)
            .code(1)
            .stderr(contains("extension.not_implemented"));
    }

    assert_eq!(sandbox.status("default")["running"], false);
}

#[test]
fn ext_error_in_json_mode_is_a_single_document() {
    let sandbox = Sandbox::new();

    let outcome = sandbox.run("default", &["--json", "ext", "list"]).code(1);

    let doc = single_document(&outcome);
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["error"]["code"], "extension.not_implemented");
    assert_eq!(doc["error"]["message"], "extensions arrive in milestone M6");
}

#[test]
fn json_flag_placed_after_the_ext_group_is_still_honoured() {
    let sandbox = Sandbox::new();

    let outcome = sandbox.run("default", &["ext", "list", "--json"]).code(1);

    assert_eq!(
        single_document(&outcome)["error"]["code"],
        "extension.not_implemented"
    );
}
