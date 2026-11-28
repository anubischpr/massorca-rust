//! The PATH a terminal starts with (decision 0032, TERM-17): the machine and user PATH merged the way Explorer builds
//! a new process's environment, with the service's own entries kept after them.

use massorca_core_terminal::shell::EnvSnapshot;
use massorca_core_terminal::terminal_path::{PathSource, expand_references, merge_path};

#[test]
fn the_path_sources_have_stable_names() {
    assert_eq!(PathSource::Registry.as_str(), "registry");
    assert_eq!(PathSource::Inherited.as_str(), "inherited");
}

#[test]
fn machine_entries_come_before_user_entries() {
    assert_eq!(
        merge_path(r"C:\Machine", r"C:\User", ""),
        r"C:\Machine;C:\User"
    );
}

#[test]
fn entries_only_the_service_has_come_after_the_registry_entries() {
    assert_eq!(
        merge_path(r"C:\Machine", r"C:\User", r"C:\Service;C:\Machine"),
        r"C:\Machine;C:\User;C:\Service"
    );
}

#[test]
fn an_entry_already_listed_is_not_repeated() {
    // Windows compares entries without regard to case or a trailing separator.
    assert_eq!(
        merge_path(r"C:\Tools\", "", r"c:\tools;C:\Service\"),
        r"C:\Tools\;C:\Service\"
    );
}

#[test]
fn empty_entries_are_dropped() {
    assert_eq!(
        merge_path(r"C:\Machine;;", r";C:\User;", ";"),
        r"C:\Machine;C:\User"
    );
}

#[test]
fn a_missing_registry_value_leaves_the_service_path() {
    assert_eq!(merge_path("", "", r"C:\Service"), r"C:\Service");
}

#[test]
fn percent_references_expand_from_the_service_environment() {
    let env = EnvSnapshot::from_pairs([
        ("SystemRoot", r"C:\Windows"),
        ("USERPROFILE", r"C:\Users\example"),
    ]);
    assert_eq!(
        expand_references(r"%SystemRoot%\system32;%USERPROFILE%\bin", &env),
        r"C:\Windows\system32;C:\Users\example\bin"
    );
}

#[test]
fn an_unknown_or_unclosed_reference_is_kept_as_written() {
    let env = EnvSnapshot::from_pairs([("PATH", "")]);
    assert_eq!(
        expand_references(r"%MASSORCA_NO_SUCH_VARIABLE%\bin", &env),
        r"%MASSORCA_NO_SUCH_VARIABLE%\bin"
    );
    assert_eq!(expand_references("100%", &env), "100%");
}

#[cfg(not(windows))]
#[test]
fn elsewhere_the_terminal_keeps_the_inherited_path_unchanged() {
    use std::borrow::Cow;

    use massorca_core_terminal::terminal_path::start_environment;

    let base = EnvSnapshot::from_pairs([("PATH", "/usr/bin:/bin")]);
    let (env, source) = start_environment(&base);
    assert!(matches!(env, Cow::Borrowed(_)));
    assert_eq!(env.get("PATH"), Some("/usr/bin:/bin"));
    assert_eq!(source, PathSource::Inherited);
}
