use massorca_core_vault::{MAX_VALUE_BYTES, Scope, SecretValue, is_valid_name, is_valid_reason};

#[test]
fn secret_value_redacts_debug_and_display() {
    let value = SecretValue::new("hunter2".to_owned()).expect("small value");
    assert_eq!(format!("{value:?}"), "[redacted]");
    assert_eq!(format!("{value}"), "[redacted]");
    assert_eq!(value.expose(), "hunter2");
}

#[test]
fn secret_value_accepts_exactly_64_kib_and_refuses_more() {
    assert_eq!(MAX_VALUE_BYTES, 65_536);
    assert!(SecretValue::new("a".repeat(MAX_VALUE_BYTES)).is_ok());
    let error = SecretValue::new("a".repeat(MAX_VALUE_BYTES + 1)).expect_err("too large");
    assert!(!error.to_string().contains("aaaa"));
    assert!(!format!("{error:?}").contains("aaaa"));
    // A multi-byte character counts by its UTF-8 bytes.
    assert!(SecretValue::new("é".repeat(MAX_VALUE_BYTES / 2 + 1)).is_err());
}

#[test]
fn names_follow_the_entry_modal_rules() {
    let longest = "x".repeat(64);
    for valid in ["A", "_", "OPENAI_API_KEY", "a1", "_9", longest.as_str()] {
        assert!(is_valid_name(valid), "{valid}");
    }
    let too_long = "x".repeat(65);
    for invalid in ["", "1A", "A-B", "A B", "Ä", "A\0", too_long.as_str()] {
        assert!(!is_valid_name(invalid), "{invalid:?}");
    }
}

#[test]
fn scopes_round_trip_through_text() {
    for text in ["global", "project:abc", "team:t-1", "project:a:b"] {
        let scope: Scope = text.parse().expect(text);
        assert_eq!(scope.to_string(), text);
    }
    assert_eq!(
        "project:p".parse::<Scope>(),
        Ok(Scope::Project("p".to_owned()))
    );
    assert_eq!("team:t".parse::<Scope>(), Ok(Scope::Team("t".to_owned())));
    assert_eq!("global".parse::<Scope>(), Ok(Scope::Global));
}

#[test]
fn scopes_refuse_bad_ids() {
    let long = format!("project:{}", "x".repeat(129));
    for text in [
        "",
        "Global",
        "global:x",
        "project",
        "project:",
        "team:",
        "user:x",
        "project:a/b",
        "project:a\nb",
        "team:\u{7f}",
        long.as_str(),
    ] {
        assert!(text.parse::<Scope>().is_err(), "{text:?}");
    }
    assert!(
        format!("project:{}", "é".repeat(128))
            .parse::<Scope>()
            .is_ok()
    );
}

#[test]
fn request_reasons_are_short_plain_text() {
    assert!(is_valid_reason("Needed to call the API"));
    assert!(is_valid_reason(&"é".repeat(200)));
    assert!(!is_valid_reason(&"x".repeat(201)));
    assert!(!is_valid_reason("line\nbreak"));
    assert!(!is_valid_reason("escape\u{1b}[31m"));
}
