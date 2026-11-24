//! The binding driven from Rust as the app drives it (decision 0080, task A2): pairing, connecting, whoami and the
//! protocol gate against a real in-process server, with the hosts kept in a sealed in-memory store.

mod common;

use massorca_client_ffi::{FfiError, parse_offer};
use massorca_protocol::methods::devices::DeviceRevokeParams;
use tokio::runtime::Runtime;

use common::{app, offer, owner, start, stop, temp_home};

#[test]
fn the_phone_pairs_reconnects_and_reads_who_it_is() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let massorca = app();

    let text = test.block_on(offer(&home));
    let summary = parse_offer(text.clone()).unwrap();
    let paired = test
        .block_on(massorca.pair(text.clone(), "office".into()))
        .expect("paired");
    assert_eq!(paired.name, "office");
    assert_eq!(paired.server, summary.server);
    assert_eq!(paired.device_name, "phone");
    assert_eq!(massorca.hosts().unwrap(), vec![paired.clone()]);

    let host = test
        .block_on(massorca.connect("office".into()))
        .expect("connected");
    let me = test.block_on(host.whoami()).expect("whoami");
    assert_eq!(me.device_name.as_deref(), Some("phone"));
    assert!(me.scopes.contains(&"device-admin".to_owned()), "{me:?}");
    let protocol = host.protocol();
    assert_eq!(protocol.server_name, "test-server");
    assert!(protocol.protocol > 0);

    // A second connection with the stored, sealed key.
    drop(host);
    let again = test
        .block_on(massorca.connect("office".into()))
        .expect("reconnected");
    assert_eq!(
        test.block_on(again.whoami()).unwrap().device_id,
        me.device_id
    );

    // The offer worked once; the name is taken.
    let reused = test.block_on(massorca.pair(text, "second".into()));
    assert_eq!(reused.map(|_| ()).unwrap_err().code(), "invalid_offer");
    let fresh = test.block_on(offer(&home));
    let taken = test.block_on(massorca.pair(fresh, "office".into()));
    assert_eq!(taken.map(|_| ()).unwrap_err().code(), "host_exists");

    massorca.forget("office".into()).expect("forgotten");
    assert!(massorca.hosts().unwrap().is_empty());
    let gone = test.block_on(massorca.connect("office".into()));
    assert_eq!(gone.map(|_| ()).unwrap_err().code(), "host_not_found");
    test.block_on(stop(running));
}

#[test]
fn a_revoked_phone_must_pair_again_and_a_stopped_host_is_worth_a_retry() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let massorca = app();
    let text = test.block_on(offer(&home));
    test.block_on(massorca.pair(text, "office".into()))
        .expect("paired");

    test.block_on(async {
        owner(&home)
            .await
            .device_revoke(&DeviceRevokeParams {
                device: "phone".into(),
            })
            .await
            .expect("revoked");
    });
    let refused = test
        .block_on(massorca.connect("office".into()))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(
        refused,
        FfiError::NotPaired {
            message: "office no longer trusts this phone. Pair again.".into()
        }
    );
    assert!(!refused.is_transient());

    test.block_on(stop(running));
    let stopped = test
        .block_on(massorca.connect("office".into()))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(stopped.code(), "unreachable", "{stopped:?}");
    assert!(stopped.is_transient());
}
