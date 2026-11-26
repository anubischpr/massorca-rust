//! `workspace.create` as the start page asks for it. A derived name the service makes free itself when it offers
//! `workspace-unique-name/1`; without that, a taken derived name is retried with -2 to -9 after it on the client. A typed
//! name is never changed, and any refusal other than a taken name is returned at once.

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};

use super::bridge::ServiceHandle;
use super::refusal::Refusal;
use crate::model::home_start::{FREE_NAME_TRIES, name_for_try};

/// Creates the workspace a start asks for. `derived` is whether the name came from the prompt; `params.unique_name` is set
/// only for a derived name the service makes free itself (`model::home_start::create_params`).
pub async fn create_for_start(
    service: &ServiceHandle,
    params: WorkspaceCreateParams,
    derived: bool,
) -> Result<Workspace, Refusal> {
    let base = params.name.clone();
    if params.unique_name || !derived || base.is_empty() {
        return service.create_workspace(params).await;
    }
    let first = match try_name(service, &params, name_for_try(&base, 1)).await {
        Ok(workspace) => return Ok(workspace),
        Err(refusal) if is_taken(&refusal) => refusal,
        Err(refusal) => return Err(refusal),
    };
    for attempt in 2..=FREE_NAME_TRIES {
        match try_name(service, &params, name_for_try(&base, attempt)).await {
            Ok(workspace) => return Ok(workspace),
            Err(refusal) if is_taken(&refusal) => {}
            Err(refusal) => return Err(refusal),
        }
    }
    // Every name was taken: the first refusal names the derived name, as the service said it.
    Err(first)
}

async fn try_name(
    service: &ServiceHandle,
    params: &WorkspaceCreateParams,
    name: String,
) -> Result<Workspace, Refusal> {
    let mut candidate = params.clone();
    candidate.name = name;
    service.create_workspace(candidate).await
}

fn is_taken(refusal: &Refusal) -> bool {
    refusal.code() == Some(codes::WORKSPACE_NAME_TAKEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(name: &str) -> WorkspaceCreateParams {
        WorkspaceCreateParams {
            project: "p1".to_owned(),
            name: name.to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            refresh_base: false,
            path: None,
            request_id: None,
            no_setup: false,
            unique_name: false,
            name_generated: false,
        }
    }

    #[test]
    fn the_unique_option_goes_out_only_when_it_is_set() {
        let mut create = params("fix-the-bug");
        create.unique_name = true;
        let asked = serde_json::to_value(&create).expect("the params serialize");
        assert_eq!(asked["unique_name"], serde_json::json!(true));
        assert_eq!(asked["name"], serde_json::json!("fix-the-bug"));
        create.unique_name = false;
        let plain = serde_json::to_value(&create).expect("the params serialize");
        assert!(plain.get("unique_name").is_none(), "no option, no key");
    }

    #[test]
    fn a_taken_name_is_the_only_refusal_that_is_retried() {
        let taken = Refusal::Failed {
            code: codes::WORKSPACE_NAME_TAKEN.to_owned(),
            message: "taken".to_owned(),
        };
        let invalid = Refusal::Failed {
            code: codes::WORKSPACE_NAME_INVALID.to_owned(),
            message: "invalid".to_owned(),
        };
        assert!(is_taken(&taken));
        assert!(!is_taken(&invalid));
    }
}
