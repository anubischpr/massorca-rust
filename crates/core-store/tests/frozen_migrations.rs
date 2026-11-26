//! The frozen migration list: shipped steps are never edited, and new steps are one file each (decision 0119).

use massorca_core_store::{MIGRATIONS, Migration};

/// FNV-1a over a step's SQL and its foreign-keys flag; unlike `DefaultHasher`, the value is stable across toolchains.
fn fingerprint(step: &Migration) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in step.sql.bytes().chain([u8::from(step.foreign_keys_off)]) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// One fingerprint per shipped step, in order. Pinned: a change here means a shipped step was edited.
const FROZEN_FINGERPRINTS: &[u64] = &[
    4614656031340014572,
    6844089024956595756,
    3460209381600344435,
    5806470151682429118,
    15708615285992038195,
    1527289105317536396,
    10934449255944297130,
    2591668217586614321,
    16461430720204924020,
    13721166905111846392,
    6120213730433227161,
    4950889942781158621,
    13341053241181561496,
    338783594757428509,
    11781659268209999622,
    15613357578472639480,
    15920670233595043584,
    10564811614389543825,
    1384823462183703893,
    5247007160555611335,
    8273744621846467813,
    11226524170696581351,
    2270311617521509388,
    15521433393138811005,
    5691976375266423526,
    4147649276172426483,
    1316024270686485209,
    2475938730647612957,
    6075309066940402345,
    16209617523214717554,
    15958017068757802044,
    3006721177455032833,
    1599126886907689468,
    15166386381867365839,
    13345638274389457872,
    13134528415099969478,
    383564131950568258,
    12227937043920872572,
    7288076747870686331,
    1295393612664016484,
    8889530923989648848,
];

#[test]
fn the_frozen_migration_list_is_unchanged() {
    let actual: Vec<u64> = MIGRATIONS.iter().map(fingerprint).collect();
    assert_eq!(
        actual.as_slice(),
        FROZEN_FINGERPRINTS,
        "the frozen migration list changed: add a file under crates/core-store/migrations instead"
    );
}
