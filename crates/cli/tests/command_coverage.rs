//! Every CLI command except `service run` and `ext` has a command descriptor in the server's core table.

use std::collections::BTreeSet;

use clap::CommandFactory;
use massorca_cli::args::Cli;
use massorca_server::core_commands;

/// The dotted path of every leaf command (`service.info`); clap's generated `help` entries are skipped.
fn command_paths(command: &clap::Command, prefix: &str, out: &mut Vec<String>) {
    for sub in command
        .get_subcommands()
        .filter(|sub| sub.get_name() != "help")
    {
        let path = if prefix.is_empty() {
            sub.get_name().to_owned()
        } else {
            format!("{prefix}.{}", sub.get_name())
        };
        if sub.has_subcommands() {
            command_paths(sub, &path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn clap_definition_is_valid() {
    Cli::command().debug_assert();
}

#[test]
fn every_cli_command_except_service_run_and_ext_has_a_core_descriptor() {
    let mut paths = Vec::new();
    command_paths(&Cli::command(), "", &mut paths);
    assert!(
        paths.iter().any(|path| path == "service.run"),
        "the walk must see `service run`: {paths:?}"
    );

    let ids: BTreeSet<String> = core_commands()
        .into_iter()
        .map(|descriptor| descriptor.id)
        .collect();
    let checked: BTreeSet<&str> = paths
        .iter()
        .map(String::as_str)
        .filter(|path| *path != "service.run" && path.split('.').next() != Some("ext"))
        .collect();

    assert_eq!(
        checked,
        BTreeSet::from([
            "config.get",
            "config.list",
            "config.set",
            "service.info",
            "service.restart",
            "service.status",
            "service.stop",
        ])
    );
    for path in &checked {
        assert!(
            ids.contains(*path),
            "`massorca {}` has no command descriptor id; core ids: {ids:?}",
            path.replace('.', " ")
        );
    }
}
