//! The API is shared between threads, and every mutation is serialised.

use std::sync::Arc;
use std::thread;

use massorca_core_workspace::Workspaces;
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::Workspace;

use crate::fixture::{Fixture, create_params};

#[test]
fn the_api_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Workspaces>();
}

#[test]
fn concurrent_creates_of_one_name_yield_exactly_one_workspace() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let api = Arc::clone(&fx.workspaces);
            let project = project.name.clone();
            thread::spawn(move || api.create_workspace(create_params(&project, "same")))
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread finished"))
        .collect();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    for error in results.iter().filter_map(|result| result.as_ref().err()) {
        assert_eq!(error.code(), codes::WORKSPACE_NAME_TAKEN);
    }
    assert_eq!(fx.workspaces.list_workspaces(None).expect("list").len(), 1);
}

#[test]
fn concurrent_creates_with_one_request_id_all_return_the_same_workspace() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let handles: Vec<_> = (0..4)
        .map(|index| {
            let api = Arc::clone(&fx.workspaces);
            let project = project.name.clone();
            thread::spawn(move || {
                let mut params = create_params(&project, &format!("racer-{index}"));
                params.request_id = Some("shared-request".into());
                api.create_workspace(params)
            })
        })
        .collect();

    let results: Vec<Workspace> = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .expect("thread finished")
                .expect("every request succeeds")
        })
        .collect();

    assert!(results.iter().all(|ws| ws.id == results[0].id));
    assert_eq!(fx.workspaces.list_workspaces(None).expect("list").len(), 1);
}
