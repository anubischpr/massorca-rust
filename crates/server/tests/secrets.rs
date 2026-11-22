//! Secrets never reach a log line or an error's text (decisions 0041, 0047): the pairing offer and its secret, a wrong
//! secret, and the key files. Its own test binary, because it captures every log event of the process.

use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use massorca_client::{Client, ConnectOptions, Hosts};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::devices::{
    self, NetworkStatus, PairingCreateParams, PairingCreated, Secret,
};
use massorca_protocol::pairing::Offer;
use massorca_server::{ListenSpec, NetworkConfig, Service, ServiceConfig};
use tokio::time::{sleep, timeout};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// Every formatted log line of the process.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        let bytes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Captured {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    _temp: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl TempHome {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

fn profile(home: &Path, name: &str) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some(name.to_owned()), |_| None)
        .expect("the profile resolves")
}

fn peer() -> PeerInfo {
    PeerInfo {
        name: "massorca-test-device".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}

/// The text between the PEM armour lines of every `.key` file under `dir`.
fn key_bodies(dir: &Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("readable").flatten() {
        let path = entry.path();
        if path.is_dir() {
            key_bodies(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "key") {
            let pem = std::fs::read_to_string(&path).expect("a PEM key");
            found.extend(
                pem.lines()
                    .filter(|line| !line.starts_with("-----") && line.len() >= 16)
                    .map(str::to_owned),
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn secrets_never_reach_logs_or_error_text() {
    // The service's logger with `MASSORCA_LOG=trace`: everything, behind the frame-dump guard.
    let captured = Captured::default();
    tracing_subscriber::registry()
        .with(EnvFilter::new("trace"))
        .with(massorca_net::frame_dump_guard())
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(captured.clone()),
        )
        .init();

    let server_home = temp_home();
    let network = NetworkConfig {
        listen: vec![ListenSpec::Address(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            0,
        )))],
    };
    let config = ServiceConfig {
        profile: profile(server_home.path(), "server"),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    };
    let running = Service::start_with_network(config, network)
        .await
        .expect("the service starts");
    let owner = Client::connect(
        &profile(server_home.path(), "server"),
        ConnectOptions::new(peer()),
    )
    .await
    .expect("the owner connects");
    for _ in 0..100 {
        let status: NetworkStatus = owner.call(devices::NETWORK_STATUS, &()).await.unwrap();
        if status.listeners.iter().all(|l| l.state == "listening") {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }

    let mut errors = Vec::new();
    let created: PairingCreated = owner
        .call(
            devices::PAIRING_CREATE,
            &PairingCreateParams {
                ttl_secs: None,
                admin: false,
            },
        )
        .await
        .expect("an offer");
    let offer_text = created.offer.expose().to_owned();
    let offer = Offer::decode(&offer_text).expect("the offer decodes");
    let secret = offer.secret.expose().to_owned();
    errors.push(format!("{created:?} {offer:?}"));

    // A wrong secret, refused.
    let client_home = temp_home();
    let hosts = Hosts::of(&profile(client_home.path(), "client"));
    let wrong_secret = "a-wrong-secret-that-must-not-be-echoed-0123456789";
    let mut wrong = offer.clone();
    wrong.secret = Secret::new(wrong_secret);
    let refused = Client::pair(&hosts, &wrong, "vps", "laptop", &peer())
        .await
        .expect_err("refused");
    errors.push(format!("{refused} {refused:?}"));
    // A damaged offer string, refused by the decoder.
    let damaged = format!("{offer_text}x");
    if let Err(error) = Offer::decode(&damaged) {
        errors.push(format!("{error} {error:?}"));
    }

    // The right secret pairs; the device reconnects with its key.
    let (paired, record) = Client::pair(&hosts, &offer, "vps", "laptop", &peer())
        .await
        .expect("pairs");
    errors.push(format!("{record:?}"));
    drop(paired);
    let again = Client::connect_host(&hosts, "vps", &peer())
        .await
        .expect("reconnects");
    drop(again);
    // A replay of the spent offer, refused.
    let other_home = temp_home();
    let replay = Client::pair(
        &Hosts::of(&profile(other_home.path(), "client")),
        &offer,
        "vps",
        "laptop",
        &peer(),
    )
    .await
    .expect_err("refused");
    errors.push(format!("{replay} {replay:?}"));

    drop(owner);
    running.shutdown_handle().shutdown();
    timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");

    let mut keys = Vec::new();
    key_bodies(server_home.path(), &mut keys);
    key_bodies(client_home.path(), &mut keys);
    assert!(
        keys.len() >= 2,
        "both the server and the device key were read"
    );

    let logs = captured.text();
    assert!(
        logs.contains("a device paired"),
        "the capture sees the service's logs"
    );
    let errors = errors.join("\n");
    let secrets = [offer_text.as_str(), secret.as_str(), wrong_secret]
        .into_iter()
        .chain(keys.iter().map(String::as_str));
    for value in secrets {
        assert!(!logs.contains(value), "a secret reached a log line");
        assert!(!errors.contains(value), "a secret reached an error's text");
    }
}
