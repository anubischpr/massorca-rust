//! Pairing offers beyond the basics (SRV-06, SRV-07): addresses the owner gives, and retiring an offer early.

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::devices::{
    self, NetworkStatus, PairingCancelParams, PairingCancelled, PairingCreateParams, PairingCreated,
};
use massorca_protocol::pairing::Offer;

use crate::common::{connect, remote_code, stop, temp_home};
use crate::network::{client_side, pair, start_listening};

fn with_addresses(addresses: &[&str]) -> PairingCreateParams {
    PairingCreateParams {
        admin: true,
        addresses: addresses
            .iter()
            .map(|address| (*address).to_owned())
            .collect(),
        ..PairingCreateParams::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn given_addresses_replace_the_bound_ones_and_a_device_pairs_through_them() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let owner = connect(home.path(), "server").await;
    let status: NetworkStatus = owner.call(devices::NETWORK_STATUS, &()).await.unwrap();
    let bound = status.listeners[0].bound.clone().expect("a bound listener");
    let port = bound.rsplit(':').next().expect("a port").to_owned();

    let created: PairingCreated = owner
        .call(
            devices::PAIRING_CREATE,
            &with_addresses(&["wss://LOCALHOST", "10.0.2.2:9000", "localhost"]),
        )
        .await
        .expect("an offer");
    let listed = [format!("localhost:{port}"), "10.0.2.2:9000".to_owned()];
    assert_eq!(
        created.addresses, listed,
        "exactly the given addresses, in order"
    );
    let offer = Offer::decode(created.offer.expose()).expect("the offer decodes");
    assert_eq!(offer.addresses, listed, "the offer string lists the same");

    let (_client_home, hosts) = client_side();
    let (_paired, record) = pair(&hosts, &offer, "vps").await.expect("pairs");
    assert_eq!(record.addresses, listed);
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_address_no_device_can_dial_is_refused_and_no_offer_is_made() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let owner = connect(home.path(), "server").await;
    for bad in [
        "0.0.0.0",
        "[::]:7347",
        "host:0",
        "wss://host/path",
        "user@host",
    ] {
        let refused = owner
            .call::<_, PairingCreated>(devices::PAIRING_CREATE, &with_addresses(&[bad]))
            .await
            .expect_err("refused");
        assert_eq!(
            remote_code(&refused),
            codes::PAIRING_INVALID_ADDRESS,
            "{bad}"
        );
    }
    let status: NetworkStatus = owner.call(devices::NETWORK_STATUS, &()).await.unwrap();
    assert_eq!(status.pending_offers, 0);
    stop(running).await;
}

async fn create(owner: &Client) -> PairingCreated {
    owner
        .call(devices::PAIRING_CREATE, &with_addresses(&[]))
        .await
        .expect("an offer")
}

async fn cancel(owner: &Client, params: PairingCancelParams) -> Result<u32, ClientError> {
    owner
        .pairing_cancel(&params)
        .await
        .map(|done: PairingCancelled| done.cancelled)
}

fn by_id(id: &str) -> PairingCancelParams {
    PairingCancelParams {
        offer_id: Some(id.to_owned()),
        all: false,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_offer_is_refused_and_the_others_still_pair() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let owner = connect(home.path(), "server").await;
    let retired = create(&owner).await;
    let kept = create(&owner).await;
    let id = retired.offer_id.clone().expect("the offer has an id");
    assert_ne!(Some(&id), kept.offer_id.as_ref());

    assert_eq!(cancel(&owner, by_id(&id)).await.expect("cancelled"), 1);
    let again = cancel(&owner, by_id(&id)).await.expect_err("not pending");
    assert_eq!(remote_code(&again), codes::PAIRING_OFFER_NOT_FOUND);

    let (_client_home, hosts) = client_side();
    let offer = Offer::decode(retired.offer.expose()).expect("decodes");
    match pair(&hosts, &offer, "vps").await {
        Err(ClientError::Handshake { error, .. }) => {
            assert_eq!(error.code, codes::PAIRING_INVALID);
        }
        other => panic!("a retired offer paired or failed otherwise: {other:?}"),
    }
    let offer = Offer::decode(kept.offer.expose()).expect("decodes");
    pair(&hosts, &offer, "vps")
        .await
        .expect("the other offer pairs");
    stop(running).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn all_retires_every_pending_offer_and_a_request_names_one_way() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let owner = connect(home.path(), "server").await;
    create(&owner).await;
    create(&owner).await;
    let all = PairingCancelParams {
        offer_id: None,
        all: true,
    };
    assert_eq!(cancel(&owner, all.clone()).await.expect("cancelled"), 2);
    let status: NetworkStatus = owner.call(devices::NETWORK_STATUS, &()).await.unwrap();
    assert_eq!(status.pending_offers, 0);
    assert_eq!(cancel(&owner, all).await.expect("nothing left"), 0);

    for params in [
        PairingCancelParams::default(),
        PairingCancelParams {
            offer_id: Some("x".to_owned()),
            all: true,
        },
    ] {
        let refused = cancel(&owner, params).await.expect_err("refused");
        assert_eq!(remote_code(&refused), codes::REQUEST_INVALID_PARAMS);
    }
    stop(running).await;
}
