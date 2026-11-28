//! A printed URL can carry a secret, as Jupyter prints `?token=...`, so `host.ports` (a `read` method) gives a terminal's
//! printed URL only to a caller that may read that terminal: `workspace`, plus `host-admin` when the terminal holds vault
//! secrets (decisions 0071, 0131). Everyone else gets the guessed URL, and nobody gets a query or fragment.

use std::collections::BTreeMap;
use std::sync::Arc;

use massorca_client::{Client, Hosts};
use massorca_core_store::Store;
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{CreateSpec, EnvValue, ShellChoice};
use massorca_protocol::methods::host::{HostPort, HostPorts};
use massorca_protocol::{ErrorBody, codes};
use massorca_server::{Owner, Permission, RequestCtx};
use serde_json::{Value, json};

use crate::common::{connect, profile, stop, temp_home};
use crate::host_ports::{LISTENER_ENV, listed, listener_shell, written_port};
use crate::network::{client_side, offer, pair, peer, start_listening};

const CREATE: &str = "ext.portstest.terminal";
const SECRET: &str = "sekrit-token";

fn entry(ports: &HostPorts, port: u16) -> HostPort {
    ports
        .ports
        .iter()
        .find(|entry| entry.port == port)
        .cloned()
        .unwrap_or_else(|| panic!("port {port} is listed: {ports:?}"))
}

fn device_ids(home: &std::path::Path) -> Vec<String> {
    let store = Store::open(profile(home, "server").store_path()).expect("the store opens");
    let ids = store
        .list_devices()
        .expect("devices")
        .into_iter()
        .map(|device| device.id)
        .collect();
    drop(store);
    ids
}

/// Pairs a device as host `name`, narrows it to `read` only, and connects it again.
async fn read_only(home: &std::path::Path, hosts: &Hosts, name: &str) -> Client {
    let before = device_ids(home);
    pair(hosts, &offer(home, false, None).await, name)
        .await
        .expect("the device pairs");
    let device = device_ids(home)
        .into_iter()
        .find(|id| !before.contains(id))
        .expect("the new device is stored");
    let store = Store::open(profile(home, "server").store_path()).expect("the store opens");
    assert!(
        store
            .set_device_scopes(&device, &["read".to_owned()])
            .expect("scopes are set")
    );
    drop(store);
    Client::connect_host(hosts, name, &peer())
        .await
        .expect("the narrowed device reconnects")
}

#[cfg_attr(
    target_os = "macos",
    ignore = "macOS reads no socket table yet (decision 0081)"
)]
#[tokio::test(flavor = "multi_thread")]
async fn a_printed_url_reaches_only_callers_that_may_read_its_terminal() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let cwd = Arc::new(home.path().to_path_buf());
    let owner_ext = Owner::Extension("portstest".to_owned());
    let _capability = running
        .registries()
        .capabilities
        .register("portstest/1", owner_ext.clone())
        .expect("the capability registers");
    // A host-admin test method: a listener terminal, marked as holding vault secrets when asked.
    let _method = running
        .registries()
        .methods
        .register(
            owner_ext,
            CREATE,
            "portstest/1",
            Permission::HostAdmin,
            move |ctx: RequestCtx, params: Value| {
                let cwd = Arc::clone(&cwd);
                async move {
                    let shell = listener_shell();
                    let file = params["port_file"].as_str().unwrap_or_default().to_owned();
                    let info = ctx
                        .state
                        .terminals
                        .create(CreateSpec {
                            workspace_id: "w-test".to_owned(),
                            cwd: cwd.as_ref().clone(),
                            shell: Some(ShellChoice {
                                program: shell.program,
                                args: shell.args,
                            }),
                            env: BTreeMap::from([(LISTENER_ENV.to_owned(), EnvValue::new(file))]),
                            size: PtySize {
                                cols: 200,
                                rows: 24,
                            },
                            title: None,
                            request_id: None,
                            holds_secrets: params["secret"].as_bool().unwrap_or_default(),
                        })
                        .map_err(|error| ErrorBody::new(codes::INTERNAL, error.to_string()))?;
                    Ok::<Value, ErrorBody>(json!({ "id": info.id }))
                }
            },
        )
        .expect("the method registers");

    let owner = connect(home.path(), "server").await;
    let mut started = Vec::new();
    for secret in [true, false] {
        let file = home.path().join(format!("listener-{secret}.port"));
        let created: Value = owner
            .call(
                CREATE,
                &json!({ "secret": secret, "port_file": file.to_str() }),
            )
            .await
            .expect("the terminal is created");
        let id = created["id"].as_str().expect("an id").to_owned();
        let port = written_port(&file).await;
        listed(&owner, &id, port).await;
        started.push(port);
    }
    let (marked, plain) = (started[0], started[1]);
    let printed = |port: u16| format!("http://localhost:{port}/app/");
    let guessed = |port: u16| format!("http://127.0.0.1:{port}/");

    // The owner may read both terminals: both printed URLs, without their query or fragment.
    let as_owner = owner.host_ports().await.expect("host.ports");
    for port in [marked, plain] {
        let shown = entry(&as_owner, port);
        assert_eq!(shown.url, Some(printed(port)), "{shown:?}");
        assert!(shown.url_printed, "{shown:?}");
    }

    let (_client_home, hosts) = client_side();
    let (standard, _) = pair(&hosts, &offer(home.path(), false, None).await, "plain")
        .await
        .expect("a standard device pairs");
    let reader = read_only(home.path(), &hosts, "reader").await;

    // `workspace` reads the plain terminal but not the one holding secrets.
    let as_standard = standard.host_ports().await.expect("host.ports");
    let shown = entry(&as_standard, plain);
    assert_eq!((shown.url, shown.url_printed), (Some(printed(plain)), true));
    let hidden = entry(&as_standard, marked);
    assert_eq!(
        (hidden.url, hidden.url_printed),
        (Some(guessed(marked)), false)
    );

    // `read` alone reads no terminal.
    let as_reader = reader.host_ports().await.expect("host.ports");
    for port in [marked, plain] {
        let hidden = entry(&as_reader, port);
        assert_eq!(
            (hidden.url, hidden.url_printed),
            (Some(guessed(port)), false)
        );
    }

    for answer in [&as_owner, &as_standard, &as_reader] {
        let text = serde_json::to_string(answer).expect("the answer encodes");
        assert!(!text.contains(SECRET), "a printed secret leaked: {text}");
    }
    drop((owner, standard, reader));
    stop(running).await;
}
