use massorca_protocol::{
    ErrorBody, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ProtocolRange, codes, negotiate,
};

fn range(min: u32, max: u32) -> ProtocolRange {
    ProtocolRange { min, max }
}

fn assert_unsupported(result: Result<u32, ErrorBody>) {
    let error = result.expect_err("no common version");
    assert_eq!(error.code, codes::PROTOCOL_UNSUPPORTED_VERSION);
    assert!(error.data.is_none());
}

#[test]
fn overlapping_ranges_pick_the_highest_common_version() {
    assert_eq!(negotiate(range(1, 3), range(2, 5)), Ok(3));
    assert_eq!(negotiate(range(2, 5), range(1, 3)), Ok(3));
    assert_eq!(negotiate(range(1, 1), range(1, 1)), Ok(1));
}

#[test]
fn client_too_new_is_unsupported() {
    assert_unsupported(negotiate(range(4, 6), range(1, 3)));
}

#[test]
fn client_too_old_is_unsupported() {
    assert_unsupported(negotiate(range(1, 2), range(3, 4)));
}

#[test]
fn inverted_client_range_is_unsupported() {
    assert_unsupported(negotiate(range(3, 1), range(1, 3)));
}

#[test]
fn this_build_negotiates_with_itself() {
    let own = range(MIN_PROTOCOL_VERSION, PROTOCOL_VERSION);
    assert_eq!(negotiate(own, own), Ok(PROTOCOL_VERSION));
}
