//! Parsing of `tailscale status --json` (decision 0043). Every name and address here is synthetic.

use std::net::{IpAddr, Ipv4Addr};

use massorca_core_remote::tailscale::{TailscaleStatus, TailscaleUnavailable, parse_status};

const RUNNING: &str = r#"{
  "Version": "1.0.0-test",
  "BackendState": "Running",
  "Self": {
    "HostName": "node-self",
    "DNSName": "node-self.tailnet-test.ts.net.",
    "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"],
    "OS": "linux",
    "Online": true,
    "FutureField": {"ignored": true}
  },
  "Peer": {
    "nodekey:zeta": {"HostName": "peer-zeta", "DNSName": "peer-zeta.tailnet-test.ts.net.", "TailscaleIPs": ["100.64.0.3"], "OS": "windows", "Online": false},
    "nodekey:alpha": {"HostName": "peer-alpha", "DNSName": "peer-alpha.tailnet-test.ts.net.", "TailscaleIPs": ["100.64.0.2", "fd7a:115c:a1e0::2"], "OS": "linux", "Online": true},
    "nodekey:v6": {"HostName": "peer-v6", "DNSName": "", "TailscaleIPs": ["fd7a:115c:a1e0::9"], "OS": "", "Online": true},
    "nodekey:gone": null
  },
  "CurrentTailnet": {"Name": "tailnet-test", "MagicDNSSuffix": "tailnet-test.ts.net", "MagicDNSEnabled": true}
}"#;

fn parse(json: &str) -> Result<TailscaleStatus, TailscaleUnavailable> {
    parse_status(json.as_bytes())
}

fn v4(octets: [u8; 4]) -> IpAddr {
    IpAddr::V4(Ipv4Addr::from(octets))
}

fn v6(text: &str) -> IpAddr {
    text.parse().expect("test IPv6 address")
}

#[test]
fn running_status_reads_self_and_peers_sorted_by_host_name() {
    let status = parse(RUNNING).expect("running status parses");
    assert_eq!(status.backend_state, "Running");
    assert_eq!(status.self_node.host_name, "node-self");
    assert_eq!(
        status.self_node.dns_name.as_deref(),
        Some("node-self.tailnet-test.ts.net")
    );
    assert_eq!(
        status.self_node.addresses,
        vec![v4([100, 64, 0, 1]), v6("fd7a:115c:a1e0::1")]
    );
    assert_eq!(status.self_node.os.as_deref(), Some("linux"));
    assert!(status.self_node.online);
    assert_eq!(status.self_node.ipv4(), Some(Ipv4Addr::new(100, 64, 0, 1)));

    let names: Vec<&str> = status
        .peers
        .iter()
        .map(|peer| peer.host_name.as_str())
        .collect();
    assert_eq!(names, ["peer-alpha", "peer-v6", "peer-zeta"]);
    let alpha = &status.peers[0];
    assert_eq!(
        alpha.dns_name.as_deref(),
        Some("peer-alpha.tailnet-test.ts.net")
    );
    assert_eq!(alpha.ipv4(), Some(Ipv4Addr::new(100, 64, 0, 2)));
    assert!(alpha.online);
    assert_eq!(
        status.magic_dns_suffix.as_deref(),
        Some("tailnet-test.ts.net")
    );
}

#[test]
fn peer_with_only_ipv6_has_no_ipv4_and_empty_fields_become_none() {
    let status = parse(RUNNING).expect("running status parses");
    let v6_peer = &status.peers[1];
    assert_eq!(v6_peer.host_name, "peer-v6");
    assert_eq!(v6_peer.ipv4(), None);
    assert_eq!(v6_peer.dns_name, None);
    assert_eq!(v6_peer.os, None);
}

#[test]
fn stopped_backend_is_not_running_and_names_its_state() {
    let json = r#"{"BackendState":"Stopped","Self":{"HostName":"node-self","TailscaleIPs":["100.64.0.1"]}}"#;
    assert_eq!(
        parse(json),
        Err(TailscaleUnavailable::NotRunning {
            backend_state: "Stopped".to_owned()
        })
    );
}

#[test]
fn login_needed_is_not_running_either() {
    let json = r#"{"BackendState":"NeedsLogin"}"#;
    assert_eq!(
        parse(json),
        Err(TailscaleUnavailable::NotRunning {
            backend_state: "NeedsLogin".to_owned()
        })
    );
}

#[test]
fn running_status_without_self_is_a_failure() {
    let json = r#"{"BackendState":"Running","Peer":{}}"#;
    assert!(matches!(parse(json), Err(TailscaleUnavailable::Failed(_))));
}

#[test]
fn status_without_backend_state_is_a_failure() {
    let json = r#"{"Self":{"HostName":"node-self"}}"#;
    assert!(matches!(parse(json), Err(TailscaleUnavailable::Failed(_))));
}

#[test]
fn invalid_json_fails_without_echoing_the_input() {
    match parse(r#"{"BackendState": SECRET-PASTED-TEXT"#) {
        Err(TailscaleUnavailable::Failed(message)) => {
            assert!(!message.contains("SECRET-PASTED-TEXT"), "{message}");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn magic_dns_suffix_falls_back_to_the_top_level_field() {
    let json = r#"{"BackendState":"Running","Self":{"HostName":"node-self"},"MagicDNSSuffix":"fallback.example.test."}"#;
    let status = parse(json).expect("status parses");
    assert_eq!(
        status.magic_dns_suffix.as_deref(),
        Some("fallback.example.test")
    );
}

#[test]
fn minimal_self_node_has_no_addresses_and_is_offline() {
    let status = parse(r#"{"BackendState":"Running","Self":{"HostName":"node-self"}}"#)
        .expect("status parses");
    assert!(status.self_node.addresses.is_empty());
    assert_eq!(status.self_node.ipv4(), None);
    assert!(!status.self_node.online);
    assert_eq!(status.self_node.dns_name, None);
    assert_eq!(status.magic_dns_suffix, None);
    assert!(status.peers.is_empty());
}

#[test]
fn null_peer_map_and_null_addresses_mean_none() {
    let json = r#"{"BackendState":"Running","Self":{"HostName":"node-self","TailscaleIPs":null},"Peer":null}"#;
    let status = parse(json).expect("status parses");
    assert!(status.self_node.addresses.is_empty());
    assert!(status.peers.is_empty());
}

#[test]
fn user_facing_messages_are_sentences() {
    assert_eq!(
        TailscaleUnavailable::NotInstalled.to_string(),
        "Tailscale is not installed on this computer."
    );
    assert_eq!(
        TailscaleUnavailable::NotRunning {
            backend_state: "NeedsLogin".to_owned()
        }
        .to_string(),
        "Tailscale is not connected (state: NeedsLogin)."
    );
    assert_eq!(
        TailscaleUnavailable::Failed("timed out".to_owned()).to_string(),
        "Tailscale could not be read: timed out"
    );
    fn is_std_error<E: std::error::Error>(_: &E) {}
    is_std_error(&TailscaleUnavailable::NotInstalled);
}
