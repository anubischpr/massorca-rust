//! Shell quoting and destination checks of the system ssh wrapper (decision 0044).

use massorca_core_remote::ssh::{SshError, SystemSsh, shell_quote};

#[test]
fn plain_and_metacharacter_words_are_quoted_literally() {
    assert_eq!(shell_quote("massorca"), "'massorca'");
    assert_eq!(
        shell_quote("$HOME `id` \"x\" ; rm -rf /"),
        "'$HOME `id` \"x\" ; rm -rf /'"
    );
    assert_eq!(shell_quote(""), "''");
}

#[test]
fn a_single_quote_is_closed_escaped_and_reopened() {
    assert_eq!(shell_quote("it's"), r"'it'\''s'");
    assert_eq!(shell_quote("'"), r"''\'''");
}

#[test]
fn destinations_that_could_read_as_options_or_split_are_refused() {
    for destination in [
        "-oProxyCommand=sh",
        "--help",
        "",
        "user host",
        "host\n",
        "host\tname",
    ] {
        assert!(
            matches!(
                SystemSsh::new(destination, Vec::new()),
                Err(SshError::Refused(_))
            ),
            "{destination:?} must be refused"
        );
    }
}

#[test]
fn ordinary_destinations_are_accepted_or_report_a_missing_ssh() {
    for destination in ["vps", "deploy@203.0.113.7", "[2001:db8::1]"] {
        match SystemSsh::new(destination, Vec::new()) {
            Ok(_) | Err(SshError::NotFound) => {}
            other => panic!("{destination}: unexpected {other:?}"),
        }
    }
}
