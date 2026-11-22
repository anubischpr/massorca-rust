//! The systemd user unit of decision 0045, compared byte for byte.

use massorca_core_remote::bootstrap::{UnitSpec, systemd_unit};

const DECISION_0045_UNIT: &str = "\
[Unit]
Description=MassOrca service
After=network-online.target

[Service]
Environment=MASSORCA_HOME=%h/.massorca/home
ExecStart=%h/.massorca/bin/massorca service run --listen tailscale --log-stderr
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
";

#[test]
fn default_unit_matches_decision_0045() {
    let listen = vec!["tailscale".to_owned()];
    let unit = systemd_unit(&UnitSpec {
        exe: "%h/.massorca/bin/massorca",
        home: "%h/.massorca/home",
        listen: &listen,
    });
    assert_eq!(unit, DECISION_0045_UNIT);
}

#[test]
fn every_listen_address_gets_its_own_flag_in_order() {
    let listen = vec!["localhost:7347".to_owned(), "100.64.0.1:7348".to_owned()];
    let unit = systemd_unit(&UnitSpec {
        exe: "%h/.massorca/bin/massorca",
        home: "%h/.massorca/home",
        listen: &listen,
    });
    assert!(
        unit.contains(
            "ExecStart=%h/.massorca/bin/massorca service run --listen localhost:7347 --listen 100.64.0.1:7348 --log-stderr\n"
        ),
        "{unit}"
    );
}

#[test]
fn without_listen_addresses_no_listen_flag_is_written() {
    let unit = systemd_unit(&UnitSpec {
        exe: "%h/.massorca/bin/massorca",
        home: "%h/.massorca/home",
        listen: &[],
    });
    assert!(
        unit.contains("ExecStart=%h/.massorca/bin/massorca service run --log-stderr\n"),
        "{unit}"
    );
}
