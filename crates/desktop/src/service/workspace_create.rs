//! `workspace.create` as the start page asks for it. A derived name asks the service for a free one when it offers
//! `workspace-unique-name/1`; without that, a taken derived name is retried with -2 to -9 after it on the client. A typed
//! name is never changed, and any refusal other than a taken name is returned at once.

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use serde::Serialize;

use super::bridge::ServiceHandle;
use super::refusal::Refusal;
use crate::model::home_start::{FREE_NAME_TRIES, name_for_try};

/// `workspace.create` with its collision-free option. The protocol's params have no `unique_name` field yet, so the
/// desktop sends it here; when the field lands in the protocol, move it there and drop this struct.
#[derive(Debug, Serialize)]
pub struct WorkspaceCreateRequest<'a> {
    #[serde(flatten)]
    pub create: &'a WorkspaceCreateParams,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unique_name: bool,
}

/// Creates the workspace a start asks for. `derived` is whether the name came from the prompt; `unique_offered` is
/// whether the service makes names unique itself.
pub async fn create_for_start(
    service: &ServiceHandle,
    params: WorkspaceCreateParams,
    derived: bool,
    unique_offered: bool,
) -> Result<Workspace, Refusal> {
    if derived && unique_offered {
        return service.create_workspace_with(params, true).await;
    }
    let base = params.name.clone();
    if !derived || base.is_empty() {
        return service.create_workspace_with(params, false).await;
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
    service.create_workspace_with(candidate, false).await
}

fn is_taken(refusal: &Refusal) -> bool {
    refusal.code() == Some(codes::WORKSPACE_NAME_TAKEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(name: &str) -> WorkspaceCreateParams {
        WorkspaceCreateParams {
            in_place: false,
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
    fn the_unique_option_goes_out_only_when_it_is_asked_for() {
        let create = params("fix-the-bug");
        let asked = serde_json::to_value(WorkspaceCreateRequest {
            create: &create,
            unique_name: true,
        })
        .expect("the request serializes");
        assert_eq!(asked["unique_name"], serde_json::json!(true));
        assert_eq!(asked["name"], serde_json::json!("fix-the-bug"));
        let plain = serde_json::to_value(WorkspaceCreateRequest {
            create: &create,
            unique_name: false,
        })
        .expect("the request serializes");
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
