//! `service.tools` (capability `service-tools/1`, decision 0116): the tools on the service's host, detected by the service.

use massorca_protocol::capabilities;
use massorca_protocol::methods::service::{ToolKind, ToolState};

use crate::common::{connect, start, stop, temp_home};

#[tokio::test]
async fn service_tools_is_offered_and_reports_git_and_the_forge_clis() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    assert!(client.has_capability(capabilities::SERVICE_TOOLS));

    let list = client.service_tools(false).await.expect("service.tools");

    let ids: Vec<&str> = list.tools.iter().map(|tool| tool.id.as_str()).collect();
    assert!(ids.contains(&"git"), "{ids:?}");
    assert!(ids.contains(&"gh"), "{ids:?}");
    assert!(ids.contains(&"glab"), "{ids:?}");
    assert!(ids.iter().all(|id| !id.contains("wsl")), "{ids:?}");
    let git = list
        .tools
        .iter()
        .find(|tool| tool.id == "git")
        .expect("git is listed");
    assert_eq!(git.kind, ToolKind::Vcs);
    assert_ne!(git.state, ToolState::Other);
    stop(running).await;
}

#[tokio::test]
async fn a_refresh_detects_again_and_answers_the_same_tools() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let first = client.service_tools(false).await.expect("first detection");
    let refreshed = client
        .service_tools(true)
        .await
        .expect("refreshed detection");

    let first_ids: Vec<&str> = first.tools.iter().map(|tool| tool.id.as_str()).collect();
    let refreshed_ids: Vec<&str> = refreshed
        .tools
        .iter()
        .map(|tool| tool.id.as_str())
        .collect();
    assert_eq!(first_ids, refreshed_ids);
    stop(running).await;
}
