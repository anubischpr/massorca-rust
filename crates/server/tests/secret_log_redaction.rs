//! A value sent through `secret.set` appears in no log line at any level, on success or refusal (decision 0071).
//! Its own binary, because it installs the process-wide subscriber that every service thread logs to.

use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use massorca_client::{Client, ClientError, ConnectOptions};
use massorca_profile::Profile;
use massorca_protocol::methods::secrets::{self, SecretEntry, SecretSet};
use massorca_protocol::{PeerInfo, codes};
use massorca_server::{Service, ServiceConfig, VaultOptions};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::format::FmtSpan;

const VALUE: &str = "log-redaction-value-9b41d7";
const SENTINEL: &str = "log-capture-is-wired";

/// Every formatted line, in memory.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        let bytes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Captured {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn set_params(scope: &str) -> SecretSet {
    SecretSet {
        scope: scope.to_owned(),
        name: "API_TOKEN".to_owned(),
        value: VALUE.to_owned(),
        replace: false,
    }
}

fn refused_code(result: Result<SecretEntry, ClientError>) -> String {
    match result {
        Err(ClientError::Remote(body)) => body.code,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_value_sent_through_secret_set_appears_in_no_trace_line() {
    let captured = Captured::default();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_span_events(FmtSpan::FULL)
        .with_ansi(false)
        .with_writer(captured.clone())
        .try_init()
        .expect("the only subscriber of this binary");
    tracing::trace!(SENTINEL);

    let home = tempfile::tempdir().expect("a temporary home");
    let profile = Profile::resolve_with(
        Some(home.path().to_path_buf()),
        Some("default".to_owned()),
        |_| None,
    )
    .expect("the profile resolves");
    let config = ServiceConfig {
        profile: profile.clone(),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    };
    let running = Service::start_with(config, VaultOptions { force_file: true })
        .await
        .expect("the service starts");
    let options = ConnectOptions::new(PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    });
    let client = Client::connect(&profile, options)
        .await
        .expect("the client connects");

    let _: SecretEntry = client
        .call(secrets::SET, &set_params("global"))
        .await
        .expect("secret.set stores the value");
    let again = client.call(secrets::SET, &set_params("global")).await;
    assert_eq!(refused_code(again), codes::SECRET_NAME_IN_USE);
    let bad_scope = client.call(secrets::SET, &set_params("nowhere")).await;
    assert_eq!(refused_code(bad_scope), codes::SECRET_INVALID_SCOPE);

    drop(client);
    running.shutdown_handle().shutdown();
    running.wait().await.expect("the service stops cleanly");

    let text = captured.text();
    assert!(text.contains(SENTINEL), "the capture saw nothing: {text}");
    assert!(
        !text.contains(VALUE),
        "a log line carries the value: {text}"
    );
}
