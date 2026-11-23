//! A terminal created for a run that got vault secrets carries the mark on its own record for its whole life in the
//! registry, exited included (decision 0071): a standard-scope device sees the mark but cannot attach to, type into or
//! resize it, before or after it ends. No agent is started: a host-admin test method creates a marked shell terminal.

use std::sync::Arc;

use massorca_core_terminal::CreateSpec;
use massorca_core_terminal::pty::PtySize;
use massorca_protocol::ErrorBody;
use massorca_protocol::codes;
use massorca_protocol::methods::terminals::{
    self, Terminal, TerminalAttachParams, TerminalInputParams, TerminalKillParams, TerminalList,
    TerminalResizeParams,
};
use massorca_server::{Owner, Permission, RequestCtx};
use serde_json::{Value, json};

use super::common::{connect, remote_code, stop, temp_home};
use super::network::{client_side, offer, pair, start_listening};

const CREATE_MARKED: &str = "ext.vaulttest.marked_terminal";

fn listed(list: &TerminalList, id: &str) -> Terminal {
    list.terminals
        .iter()
        .find(|terminal| terminal.id == id)
        .cloned()
        .expect("the terminal is listed")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_marked_terminal_stays_host_admin_only_after_it_exits() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let cwd = Arc::new(home.path().to_path_buf());
    let owner_ext = Owner::Extension("vaulttest".to_owned());
    let _capability = running
        .registries()
        .capabilities
        .register("vaulttest/1", owner_ext.clone())
        .expect("the capability registers");
    let _method = running
        .registries()
        .methods
        .register(
            owner_ext,
            CREATE_MARKED,
            "vaulttest/1",
            Permission::HostAdmin,
            move |ctx: RequestCtx, _params: Value| {
                let cwd = Arc::clone(&cwd);
                async move {
                    let info = ctx
                        .state
                        .terminals
                        .create(CreateSpec {
                            workspace_id: "w-test".to_owned(),
                            cwd: cwd.as_ref().clone(),
                            shell: None,
                            env: Default::default(),
                            size: PtySize { cols: 80, rows: 24 },
                            title: None,
                            request_id: None,
                            holds_secrets: true,
                        })
                        .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?;
                    Ok::<Value, ErrorBody>(json!({ "id": info.id }))
                }
            },
        )
        .expect("the method registers");

    let owner = connect(home.path(), "server").await;
    let created: Value = owner
        .call(CREATE_MARKED, &json!({}))
        .await
        .expect("created");
    let id = created["id"].as_str().expect("an id").to_owned();
    let (_client_home, hosts) = client_side();
    let (device, _) = pair(&hosts, &offer(home.path(), false, None).await, "plain")
        .await
        .expect("the device pairs");

    let refused_everything = |label: &'static str| {
        let device = &device;
        let id = id.clone();
        async move {
            let attach = device
                .subscribe(
                    terminals::ATTACH,
                    &TerminalAttachParams {
                        terminal: id.clone(),
                        since: None,
                        cols: None,
                        rows: None,
                        answers_queries: None,
                    },
                )
                .await
                .map(|_| ())
                .expect_err("attach is refused");
            assert_eq!(remote_code(&attach), codes::PERMISSION_DENIED, "{label}");
            let input = device
                .call::<_, Value>(
                    terminals::INPUT,
                    &TerminalInputParams::from_bytes(&id, b"x"),
                )
                .await
                .expect_err("input is refused");
            assert_eq!(remote_code(&input), codes::PERMISSION_DENIED, "{label}");
            let resize = device
                .call::<_, Value>(
                    terminals::RESIZE,
                    &TerminalResizeParams {
                        terminal: id.clone(),
                        cols: 90,
                        rows: 30,
                    },
                )
                .await
                .expect_err("resize is refused");
            assert_eq!(remote_code(&resize), codes::PERMISSION_DENIED, "{label}");
        }
    };

    let list: TerminalList = device
        .call(terminals::LIST, &json!({}))
        .await
        .expect("the device lists terminals");
    assert!(listed(&list, &id).holds_secrets, "the mark is visible");
    refused_everything("live").await;

    let _: Value = owner
        .call(
            terminals::KILL,
            &TerminalKillParams {
                terminal: id.clone(),
                force: true,
            },
        )
        .await
        .expect("the owner kills it");
    for _ in 0..200 {
        let list: TerminalList = owner.call(terminals::LIST, &json!({})).await.unwrap();
        if listed(&list, &id).exit.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let list: TerminalList = device.call(terminals::LIST, &json!({})).await.unwrap();
    let exited = listed(&list, &id);
    assert!(exited.exit.is_some(), "the terminal has exited");
    assert!(exited.holds_secrets, "the exited terminal keeps its mark");
    refused_everything("exited").await;

    // The owner still reads it.
    owner
        .subscribe(
            terminals::ATTACH,
            &TerminalAttachParams {
                terminal: id.clone(),
                since: None,
                cols: None,
                rows: None,
                answers_queries: None,
            },
        )
        .await
        .expect("the owner attaches");
    drop((device, owner));
    stop(running).await;
}
