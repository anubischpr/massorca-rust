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
    self, DeviceInfo, DeviceList, NetworkStatus, PairingCreateParams, PairingCreated, Secret,
    WhoAmI,
};
use massorca_protocol::methods::extensions::{
    ExtensionInstallParams, ExtensionRemoveParams, ExtensionSetEnabledParams,
};
use massorca_protocol::methods::host::{self, HostMemory, HostPorts};
use massorca_protocol::methods::secrets::{self, SecretSet};
use massorca_protocol::pairing::Offer;
use massorca_protocol::{PeerInfo, codes};
use massorca_server::{ListenSpec, NetworkConfig, RunningService, Scopes, Service};
use tokio::time::{sleep, timeout};

use crate::common::{connect, profile, remote_code, service_config, stop, temp_home};

pub(crate) fn peer() -> PeerInfo {
    PeerInfo {
        name: "massorca-test-device".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}

/// A service listening on an ephemeral loopback port.
pub(crate) async fn start_listening(home: &Path) -> RunningService {
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

pub(crate) async fn offer(home: &Path, admin: bool, ttl_secs: Option<u32>) -> Offer {
    let owner = connect(home, "server").await;
    let created: PairingCreated = owner
        .call(
            devices::PAIRING_CREATE,
            &PairingCreateParams {
                ttl_secs,
                admin,
                standard: !admin,
            },
        )
        .await
        .expect("an offer");
    Offer::decode(created.offer.expose()).expect("the offer decodes")
}

/// A client profile with its own hosts list.
pub(crate) fn client_side() -> (TempHome, Hosts) {
    let home = temp_home();
    let hosts = Hosts::of(&profile(home.path(), "client"));
    (home, hosts)
}

pub(crate) async fn pair(
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

/// Only a revoked device can be forgotten, only with host-admin, and a forgotten device leaves the list.
#[tokio::test(flavor = "multi_thread")]
async fn a_revoked_device_is_forgotten_by_a_host_admin_and_leaves_the_list() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let (standard, laptop) = pair(&hosts, &offer(home.path(), false, None).await, "plain")
        .await
        .unwrap();
    let owner = connect(home.path(), "server").await;
    let forget = |device: &str| devices::DeviceForgetParams {
        device: device.to_owned(),
    };

    let active = owner
        .call::<_, DeviceInfo>(devices::FORGET, &forget(&laptop.device_id))
        .await
        .expect_err("an active device is refused");
    assert_eq!(remote_code(&active), codes::DEVICE_NOT_REVOKED);
    let by_name = owner
        .call::<_, DeviceInfo>(devices::FORGET, &forget("laptop"))
        .await
        .expect_err("a name that only an active device has is refused");
    assert_eq!(remote_code(&by_name), codes::DEVICE_NOT_REVOKED);
    let denied = standard
        .call::<_, DeviceInfo>(devices::FORGET, &forget(&laptop.device_id))
        .await
        .expect_err("a standard device has no host-admin");
    assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED);

    let _: DeviceInfo = owner
        .call(
            devices::REVOKE,
            &devices::DeviceRevokeParams {
                device: laptop.device_id.clone(),
            },
        )
        .await
        .unwrap();
    let forgotten: DeviceInfo = owner
        .call(devices::FORGET, &forget("laptop"))
        .await
        .expect("a revoked device is forgotten by name");
    assert_eq!(forgotten.id, laptop.device_id);
    let list: DeviceList = owner.call(devices::LIST, &()).await.unwrap();
    assert!(list.devices.is_empty(), "{list:?}");
    let again = owner
        .call::<_, DeviceInfo>(devices::FORGET, &forget(&laptop.device_id))
        .await
        .expect_err("gone");
    assert_eq!(remote_code(&again), codes::DEVICE_NOT_FOUND);

    let refused = Client::connect_host(&hosts, "plain", &peer())
        .await
        .expect_err("a forgotten device is still refused");
    assert!(
        matches!(refused, ClientError::Handshake { .. }),
        "{refused:?}"
    );
    stop(running).await;
}

/// An offer that asks for nothing in particular grants every scope, as an older client's `admin: false` does;
/// only `standard` reduces it. `device.grant` raises a reduced device from its next connection.
#[tokio::test(flavor = "multi_thread")]
async fn a_default_offer_is_full_access_and_grant_raises_a_reduced_device() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let owner = connect(home.path(), "server").await;
    let all = vec!["read", "workspace", "host-admin", "device-admin"];

    for params in [
        PairingCreateParams::default(),
        PairingCreateParams {
            ttl_secs: None,
            admin: true,
            standard: false,
        },
    ] {
        let created: PairingCreated = owner
            .call(devices::PAIRING_CREATE, &params)
            .await
            .expect("an offer");
        let offer = Offer::decode(created.offer.expose()).expect("the offer decodes");
        assert_eq!(offer.scopes, all, "{params:?}");
    }

    let reduced = offer(home.path(), false, None).await;
    assert_eq!(reduced.scopes, vec!["read", "workspace"]);
    let (_, record) = pair(&hosts, &reduced, "small").await.unwrap();
    let granted: DeviceInfo = owner
        .call(
            devices::GRANT,
            &devices::DeviceGrantParams {
                device: record.device_id.clone(),
            },
        )
        .await
        .expect("granted");
    assert_eq!(granted.scopes, all);
    let again = Client::connect_host(&hosts, "small", &peer())
        .await
        .unwrap();
    let list: DeviceList = again
        .call(devices::LIST, &())
        .await
        .expect("full access from the next connection");
    assert!(!list.devices.is_empty());
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

/// A stream open when its device is revoked ends with the revocation as its reason, not as a finished stream.
#[tokio::test(flavor = "multi_thread")]
async fn revoking_ends_the_device_streams_with_the_reason() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let offer = offer(home.path(), false, None).await;
    let (device, record) = pair(&hosts, &offer, "vps").await.unwrap();
    let mut events = device.subscribe_events().await.expect("a stream");

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

    let ended = timeout(Duration::from_secs(5), async {
        loop {
            match events.next().await {
                Some(Ok(_)) => {}
                other => return other,
            }
        }
    })
    .await
    .expect("the stream ends");
    let error = match ended {
        Some(Err(error)) => error,
        other => panic!(
            "expected the revocation, got {other:?}, connection lost: {}",
            events.connection_lost()
        ),
    };
    assert_eq!(error.code, codes::AUTH_DEVICE_REVOKED);
    assert!(!events.connection_lost());
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
async fn a_read_only_device_may_read_host_memory() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let standard_offer = offer(home.path(), false, None).await;
    let _ = pair(&hosts, &standard_offer, "reader").await.unwrap();
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

    let memory: HostMemory = reader
        .call(host::MEMORY, &())
        .await
        .expect("host.memory needs only Read");
    assert!(memory.service.bytes > 0, "{memory:?}");
    assert!(memory.total_bytes >= memory.service.bytes, "{memory:?}");
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_read_only_device_may_read_host_ports() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let standard_offer = offer(home.path(), false, None).await;
    let _ = pair(&hosts, &standard_offer, "reader").await.unwrap();
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

    let ports: HostPorts = reader
        .call(host::PORTS, &())
        .await
        .expect("host.ports needs only Read");
    assert!(ports.sampled_at_ms > 0, "{ports:?}");
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
            agents::ACCOUNTS_SIGN_IN,
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

#[tokio::test(flavor = "multi_thread")]
async fn a_device_without_host_admin_inspects_only_the_catalogue() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let standard_offer = offer(home.path(), false, None).await;
    let (standard, _) = pair(&hosts, &standard_offer, "plain").await.unwrap();
    let folder = temp_home();

    standard
        .extension_inspect("catalogue:massorca.claude")
        .await
        .expect("the catalogue needs only Read");
    for source in [
        format!("dir:{}", folder.path().display()),
        format!("file:{}", folder.path().join("kit.mxt").display()),
    ] {
        let denied = standard
            .extension_inspect(&source)
            .await
            .expect_err("a host path needs HostAdmin");
        assert_eq!(remote_code(&denied), codes::PERMISSION_DENIED, "{source}");
    }
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_without_host_admin_cannot_install_enable_or_remove_extensions() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let (_client_home, hosts) = client_side();
    let standard_offer = offer(home.path(), false, None).await;
    let (standard, _) = pair(&hosts, &standard_offer, "plain").await.unwrap();

    let install = standard
        .extension_install(&ExtensionInstallParams {
            source: "catalogue:massorca.codex".to_owned(),
            consent: None,
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect_err("installing needs HostAdmin");
    assert_eq!(remote_code(&install), codes::PERMISSION_DENIED);
    let toggle = standard
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "massorca.claude".to_owned(),
            enabled: false,
        })
        .await
        .expect_err("enabling needs HostAdmin");
    assert_eq!(remote_code(&toggle), codes::PERMISSION_DENIED);
    let removal = standard
        .extension_remove(&ExtensionRemoveParams {
            id: "massorca.claude".to_owned(),
            ..Default::default()
        })
        .await
        .expect_err("removing needs HostAdmin");
    assert_eq!(remote_code(&removal), codes::PERMISSION_DENIED);

    let review = standard
        .extension_inspect("catalogue:massorca.claude")
        .await
        .expect("the catalogue needs only Read");
    for plan in &review.cli {
        if let massorca_protocol::methods::extensions::CliPlanState::Detected { path } = &plan.state
        {
            assert!(
                path.is_none(),
                "a host path reaches only a host administrator"
            );
        }
    }
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_read_only_device_cannot_make_git_start_a_program_the_repository_names() {
    use crate::git_fixture::git;
    use massorca_protocol::methods::git::diff::{self, DiffTarget, GitDiffFile, GitDiffFileParams};
    use massorca_protocol::methods::git::status::GitStatusParams;
    use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
    use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};

    let home = temp_home();
    let repo = home.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "core.autocrlf", "false"]);
    std::fs::write(repo.join(".gitattributes"), "*.txt filter=planted\n").unwrap();
    std::fs::write(repo.join("file.txt"), "one\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "initial"]);
    let marker = home.path().join("marker");
    let shown = marker.to_string_lossy().replace('\\', "/");
    let record = |name: &str| format!("echo {name} >> '{shown}'");
    git(&repo, &["config", "core.fsmonitor", &record("fsmonitor")]);
    let clean = format!("{}; cat", record("clean"));
    git(&repo, &["config", "filter.planted.clean", &clean]);

    let running = start_listening(home.path()).await;
    let owner = connect(home.path(), "server").await;
    let project: Project = owner
        .call(
            projects::ADD,
            &ProjectAddParams {
                path: repo.to_string_lossy().into_owned(),
                name: Some("repo".to_owned()),
                kind: Some(ProjectKind::Git),
                workspaces_root: Some(home.path().join("ws").to_string_lossy().into_owned()),
            },
        )
        .await
        .expect("add project");
    let workspace: Workspace = owner
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                project: project.id,
                name: "feature".to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: None,
            },
        )
        .await
        .expect("create workspace");
    std::fs::write(Path::new(&workspace.path).join("file.txt"), "one\ntwo\n").unwrap();
    let (_client_home, hosts) = client_side();
    let standard_offer = offer(home.path(), false, None).await;
    let _ = pair(&hosts, &standard_offer, "reader").await.unwrap();
    let store = Store::open(profile(home.path(), "server").store_path()).unwrap();
    let device = store
        .list_devices()
        .unwrap()
        .pop()
        .expect("the paired device");
    assert!(
        store
            .set_device_scopes(&device.id, &["read".to_owned()])
            .unwrap()
    );
    drop(store);
    let reader = Client::connect_host(&hosts, "reader", &peer())
        .await
        .expect("reconnects");
    // Creating the workspace ran the planted programs as the owner; only the reads below count.
    let _ = std::fs::remove_file(&marker);

    let status = reader
        .git_status(&GitStatusParams {
            workspace: "feature".to_owned(),
            limit: None,
        })
        .await
        .expect("a read-only device reads the status");
    assert!(
        status.entries.iter().any(|entry| entry.path == "file.txt"),
        "{status:?}"
    );
    let diff_file = GitDiffFileParams {
        workspace: "feature".to_owned(),
        target: DiffTarget::Worktree,
        path: "file.txt".to_owned(),
        original_path: None,
        max_bytes: None,
    };
    let patch: GitDiffFile = reader
        .call(diff::DIFF_FILE, &diff_file)
        .await
        .expect("diff");
    assert!(patch.patch.unwrap_or_default().contains("+two"));
    assert_eq!(std::fs::read_to_string(&marker).unwrap_or_default(), "");

    // The owner, who holds `workspace`, keeps git's own behaviour: the repository's filter runs.
    let _: GitDiffFile = owner.call(diff::DIFF_FILE, &diff_file).await.expect("diff");
    assert!(
        std::fs::read_to_string(&marker)
            .unwrap_or_default()
            .contains("clean")
    );
    drop((owner, reader));
    stop(running).await;
}
