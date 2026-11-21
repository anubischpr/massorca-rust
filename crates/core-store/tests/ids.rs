use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use massorca_core_store::{new_id, now_ms};

fn epoch_millis() -> i64 {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    i64::try_from(elapsed.as_millis()).unwrap()
}

#[test]
fn new_ids_are_uuid_v7_strings() {
    let id = new_id();
    assert_eq!(id.len(), 36);
    assert_eq!(uuid::Uuid::parse_str(&id).unwrap().get_version_num(), 7);
}

#[test]
fn ids_created_later_sort_later() {
    let first = new_id();
    sleep(Duration::from_millis(2));
    let second = new_id();
    assert!(first < second, "{first} should sort before {second}");
}

#[test]
fn now_ms_is_milliseconds_since_the_unix_epoch() {
    let before = epoch_millis();
    let now = now_ms();
    let after = epoch_millis();
    assert!(
        before <= now && now <= after,
        "{before} <= {now} <= {after}"
    );
}
