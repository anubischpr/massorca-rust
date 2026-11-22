//! The network listener, pairing and device authentication (decisions 0040, 0041), with a real client over loopback TLS.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::Duration;

use crate::common::TempHome;
use massorca_client::{Client, ClientError, HostRecord, Hosts};
use massorca_core_store::Store;
use massorca_net::TlsIdentity;
use massorca_protocol::methods::agents;
use massorca_protocol::methods::devices::{
    self, DeviceList, NetworkStatus, PairingCreateParams, PairingCreated, Secret, WhoAmI,
};
use massorca_protocol::methods::secrets::{self, SecretSet};
use massorca_protocol::pairing::Offer;
use massorca_protocol::{PeerInfo, codes};
use massorca_server::{ListenSpec, NetworkConfig, RunningService, Scopes, Service};
use tokio::time::{sleep, timeout};

use crate::common::{connect, profile, remote_code, service_config, stop, temp_home};

fn peer() -> PeerInfo {
    PeerInfo {
        name: "massorca-test-device".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}

/// A service listening on an ephemeral loopback port.
async fn start_listening(home: &Path) -> RunningService {
    let network = NetworkConfig {
        listen: vec![ListenSpec::Address(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            0,
        )))],
    };
    let running = Service::start_with_network(service_config(home, "server"), network)
        .await
        .expect("the service starts");
    // The listener binds in the background; wait until an offer can name it.
    let owner = connect(home, "server").await;
    for _ in 0..100 {
        let status: NetworkStatus = owner.call(devices::NETWORK_STATUS, &()).await.unwrap();
        if status
            .listeners
            .iter()
            .all(|listener| listener.state == "listening")
        {
            return running;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("the network listener did not start");
}

async fn offer(home: &Path, admin: bool, ttl_secs: Option<u32>) -> Offer {
    let owner = connect(home, "server").await;
    let created: PairingCreated = owner
        .call(
            devices::PAIRING_CREATE,
            &PairingCreateParams { ttl_secs, admin },
        )
        .await
        .expect("an offer");
    Offer::decode(created.offer.expose()).expect("the offer decodes")
}

/// A client profile with its own hosts list.
fn client_side() -> (TempHome, Hosts) {
    let home = temp_home();
    let hosts = Hosts::of(&profile(home.path(), "client"));
    (home, hosts)
}

async fn pair(
    hosts: &Hosts,
    offer: &Offer,
    name: &str,
) -> Result<(Client, HostRecord), ClientError> {
    Client::pair(hosts, offer, name, "laptop", &peer()).await
}

fn handshake_code(error: &ClientError) -> String {
    match error {
        ClientError::Handshake { error, .. } => error.code.clone(),
        other => panic!("expected a refused handshake, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_pairs_once_and_reconnects_with_its_key() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let offer = offer(home.path(), false, None).await;

    let (paired, record) = pair(&hosts, &offer, "vps").await.expect("pairs");
    assert_eq!(record.scopes, ["read", "workspace"]);
    drop(paired);

    let again = Client::connect_host(&hosts, "vps", &peer())
        .await
        .expect("reconnects");
    let me: WhoAmI = again.call(devices::WHOAMI, &()).await.unwrap();
    assert_eq!(me.principal, "device");
    assert_eq!(me.device.expect("a device").id, record.device_id);

    // The same offer is spent: a replay is refused like a wrong secret.
    let (_other_home, other_hosts) = client_side();
    let replay = pair(&other_hosts, &offer, "vps")
        .await
        .expect_err("refused");
    assert_eq!(handshake_code(&replay), codes::PAIRING_INVALID);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_or_expired_secret_is_refused() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();

    let mut wrong = offer(home.path(), false, None).await;
    wrong.secret = Secret::new("not-the-secret");
    let refused = pair(&hosts, &wrong, "vps").await.expect_err("refused");
    assert_eq!(handshake_code(&refused), codes::PAIRING_INVALID);
    assert!(
        !hosts.exists("vps"),
        "nothing is stored for a refused pairing"
    );

    let short = offer(home.path(), false, Some(1)).await;
    sleep(Duration::from_millis(1200)).await;
    let expired = pair(&hosts, &short, "vps").await.expect_err("refused");
    assert_eq!(handshake_code(&expired), codes::PAIRING_EXPIRED);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_pairing_attempts_are_rate_limited() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let good = offer(home.path(), false, None).await;
    let mut wrong = good.clone();
    wrong.secret = Secret::new("guess");
    for _ in 0..5 {
        let refused = pair(&hosts, &wrong, "vps").await.expect_err("refused");
        assert_eq!(handshake_code(&refused), codes::PAIRING_INVALID);
    }
    let limited = pair(&hosts, &good, "vps").await.expect_err("limited");
    assert_eq!(handshake_code(&limited), codes::PAIRING_RATE_LIMITED);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unpaired_or_silent_client_is_refused() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let offer = offer(home.path(), false, None).await;

    // A key the server never saw, presented as a paired device.
    let stranger = HostRecord {
        name: "vps".to_owned(),
        server: offer.server.clone(),
        addresses: offer.addresses.clone(),
        pin: offer.pin.clone(),
        device_id: "none".to_owned(),
        device_name: "stranger".to_owned(),
        scopes: Vec::new(),
        paired_at_ms: 0,
    };
    hosts
        .add(&stranger, &TlsIdentity::generate("stranger").unwrap())
        .unwrap();
    let refused = Client::connect_host(&hosts, "vps", &peer())
        .await
        .expect_err("refused");
    assert_eq!(handshake_code(&refused), codes::AUTH_UNKNOWN_DEVICE);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_server_key_is_never_trusted() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let mut forged = offer(home.path(), false, None).await;
    forged.pin = TlsIdentity::generate("impostor").unwrap().pin().to_string();
    let refused = pair(&hosts, &forged, "vps").await.expect_err("refused");
    assert!(
        matches!(refused, ClientError::ServerIdentityMismatch),
        "{refused:?}"
    );
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn scopes_limit_a_device_and_admin_widens_them() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();

    let standard_offer = offer(home.path(), false, None).await;
    let (standard, _) = pair(&hosts, &standard_offer, "plain").await.unwrap();
    let denied = standard
        .call::<_, DeviceList>(devices::LIST, &())
        .await
        .expect_err("denied");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);
    let denied = standard
        .call::<_, PairingCreated>(devices::PAIRING_CREATE, &PairingCreateParams::default())
        .await
        .expect_err("denied");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);

    let admin_offer = offer(home.path(), true, None).await;
    let (admin, _) = pair(&hosts, &admin_offer, "admin").await.unwrap();
    let list: DeviceList = admin.call(devices::LIST, &()).await.unwrap();
    assert_eq!(list.devices.len(), 2);
    assert_eq!(
        list.devices.iter().filter(|device| device.current).count(),
        1
    );
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_standard_device_cannot_set_or_reset_a_secret() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let standard_offer = offer(home.path(), false, None).await;
    let (standard, _) = pair(&hosts, &standard_offer, "plain").await.unwrap();
    let value = "device-secret-value-27aa";
    let set = SecretSet {
        scope: "global".to_owned(),
        name: "TOKEN".to_owned(),
        value: value.to_owned(),
        replace: false,
    };
    let denied = standard
        .call::<_, serde_json::Value>(secrets::SET, &set)
        .await
        .expect_err("denied");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);
    assert!(!format!("{denied:?}").contains(value));
    let denied = standard
        .call::<_, serde_json::Value>(secrets::RESET_KEY, &())
        .await
        .expect_err("denied");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn revoking_closes_the_device_at_once_and_refuses_it_later() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let offer = offer(home.path(), false, None).await;
    let (device, record) = pair(&hosts, &offer, "vps").await.unwrap();

    let owner = connect(home.path(), "server").await;
    let _: serde_json::Value = owner
        .call(
            devices::REVOKE,
            &devices::DeviceRevokeParams {
                device: record.device_id.clone(),
            },
        )
        .await
        .unwrap();

    let closed = timeout(Duration::from_secs(5), async {
        loop {
            match device.call::<_, WhoAmI>(devices::WHOAMI, &()).await {
                Err(ClientError::Disconnected) => break,
                Ok(_) => sleep(Duration::from_millis(20)).await,
                Err(other) => panic!("unexpected {other:?}"),
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "the revoked device's connection stays open");

    let refused = Client::connect_host(&hosts, "vps", &peer())
        .await
        .expect_err("refused");
    assert_eq!(handshake_code(&refused), codes::AUTH_DEVICE_REVOKED);
    let list: DeviceList = owner.call(devices::LIST, &()).await.unwrap();
    assert!(list.devices[0].revoked_at_ms.is_some());
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_listener_there_is_no_offer() {
    let home = temp_home();
    let running = crate::common::start(home.path(), "server").await;
    let owner = connect(home.path(), "server").await;
    let refused = owner
        .call::<_, PairingCreated>(devices::PAIRING_CREATE, &PairingCreateParams::default())
        .await
        .expect_err("refused");
    assert_eq!(remote_code(&refused), codes::PAIRING_UNAVAILABLE);
    let me: WhoAmI = owner.call(devices::WHOAMI, &()).await.unwrap();
    assert_eq!(me.principal, "owner");
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hello_without_auth_gets_no_service() {
    use massorca_net::Keepalive;
    use massorca_protocol::{
        Frame, MAX_FRAME_LEN_NETWORK, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION,
        ProtocolRange,
    };
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let offer = offer(home.path(), true, None).await;
    let pin: massorca_net::Pin = offer.pin.parse().unwrap();
    let identity = TlsIdentity::generate("raw").unwrap();
    let (ws, _) = massorca_net::connect(&offer.addresses, pin, &identity, Duration::from_secs(5))
        .await
        .expect("TLS and WebSocket");
    let (inbound_tx, mut inbound) = tokio::sync::mpsc::channel::<Frame>(8);
    let (outbound, outbound_rx) = tokio::sync::mpsc::channel::<Message>(8);
    let _tasks = massorca_net::spawn_frame_tasks(
        ws,
        MAX_FRAME_LEN_NETWORK,
        Keepalive::default(),
        inbound_tx,
        outbound_rx,
    );
    outbound
        .send(Message::Hello {
            protocol: ProtocolRange {
                min: MIN_PROTOCOL_VERSION,
                max: PROTOCOL_VERSION,
            },
            client: peer(),
            capabilities: Vec::new(),
            auth: None,
        })
        .await
        .unwrap();
    let answer = timeout(Duration::from_secs(5), inbound.recv())
        .await
        .unwrap();
    match answer {
        Some(Frame::Message(Message::Rejected { error, .. })) => {
            assert_eq!(error.code, codes::AUTH_REQUIRED)
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    // The server closes after refusing, and a request never reaches the dispatcher.
    outbound
        .send(crate::common::request(
            1,
            devices::LIST,
            serde_json::Value::Null,
        ))
        .await
        .ok();
    let closed = timeout(Duration::from_secs(5), inbound.recv())
        .await
        .unwrap();
    assert!(closed.is_none(), "{closed:?}");
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_read_only_device_is_refused_every_agent_account_write_and_may_list() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let standard_offer = offer(home.path(), false, None).await;
    let _ = pair(&hosts, &standard_offer, "reader").await.unwrap();
    // The pairing flow grants `read` and `workspace`; hold this device to `read` alone, then reconnect so it is checked.
    let store = Store::open(profile(home.path(), "server").store_path()).unwrap();
    let standard_set = Scopes::STANDARD.names();
    let device = store
        .list_devices()
        .unwrap()
        .into_iter()
        .find(|device| device.scopes == standard_set)
        .expect("the paired device holds the standard scopes");
    assert!(
        store
            .set_device_scopes(&device.id, &["read".to_owned()])
            .unwrap()
    );
    drop(store);
    let reader = Client::connect_host(&hosts, "reader", &peer())
        .await
        .expect("the narrowed device reconnects");

    let kind = "fake";
    let account = "0195c0de-0000-7000-8000-000000000001";
    let writes = [
        (agents::ACCOUNTS_ADD, serde_json::json!({ "kind": kind })),
        (
            agents::ACCOUNTS_IMPORT,
            serde_json::json!({ "kind": kind, "from": "/no-such-source" }),
        ),
        (
            agents::ACCOUNTS_ACTIVATE,
            serde_json::json!({ "kind": kind, "account": account }),
        ),
        (
            agents::ACCOUNTS_REMOVE,
            serde_json::json!({ "kind": kind, "account": account }),
        ),
        (
            agents::ACCOUNTS_CHECK,
            serde_json::json!({ "kind": kind, "account": account }),
        ),
    ];
    for (method, params) in writes {
        let denied = reader
            .call::<_, serde_json::Value>(method, &params)
            .await
            .expect_err("denied");
        assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED, "{method}");
    }
    let listed: serde_json::Value = reader
        .call(agents::ACCOUNTS_LIST, &serde_json::json!({ "kind": kind }))
        .await
        .expect("a read is allowed to a read-only device");
    assert_eq!(listed["accounts"], serde_json::json!([]));
    stop(running).await;
}
