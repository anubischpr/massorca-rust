use massorca_protocol::{ErrorBody, codes};

use crate::fixture::{Fixture, create_params};

#[test]
fn a_refusal_carries_its_stable_code_and_converts_to_a_protocol_error_body() {
    let fx = Fixture::new();

    let error = fx
        .workspaces
        .create_workspace(create_params("no-such-project", "x"))
        .expect_err("refused");

    assert_eq!(error.code(), codes::PROJECT_NOT_FOUND);
    assert!(error.to_string().starts_with("project.not_found: "));
    let body: ErrorBody = error.clone().into();
    assert_eq!(body.code, codes::PROJECT_NOT_FOUND);
    assert_eq!(body.message, error.message());
    assert_eq!(body.data, None);
}
