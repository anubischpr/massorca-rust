//! Every CLI command except `service run` has a command descriptor in the server's core table,
//! except the commands that run on this computer alone.

use std::collections::BTreeSet;

use clap::CommandFactory;
use massorca_cli::args::Cli;
use massorca_server::core_commands;

/// Commands that run in the CLI itself: they read or write the client's own files, run SSH or Tailscale, or print
/// text. They make no service request, and the server's descriptor test needs every descriptor to name a core method.
const CLIENT_ONLY: [&str; 9] = [
    "host.add-ssh",
    "host.discover",
    "host.list",
    "host.remove",
    "host.show",
    "host.status",
    "pair.accept",
    "profile.secure",
    "service.unit",
];

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
fn every_cli_command_with_a_service_request_has_a_core_descriptor() {
    let mut paths = Vec::new();
    command_paths(&Cli::command(), "", &mut paths);
    assert!(
        paths.iter().any(|path| path == "service.run"),
        "the walk must see `service run`: {paths:?}"
    );
    for client_only in CLIENT_ONLY {
        assert!(
            paths.iter().any(|path| path == client_only),
            "`{client_only}` is not a CLI command"
        );
    }

    let ids: BTreeSet<String> = core_commands()
        .into_iter()
        .map(|descriptor| descriptor.id)
        .collect();
    for client_only in CLIENT_ONLY {
        assert!(
            !ids.contains(client_only),
            "`{client_only}` makes no service request and must not have a descriptor"
        );
    }
    let checked: BTreeSet<&str> = paths
        .iter()
        .map(String::as_str)
        .filter(|path| *path != "service.run")
        .filter(|path| !CLIENT_ONLY.contains(path))
        .collect();

    assert_eq!(
        checked,
        BTreeSet::from([
            "agent.accounts.activate",
            "agent.accounts.add",
            "agent.accounts.check",
            "agent.accounts.import",
            "agent.accounts.list",
            "agent.accounts.remove",
            "agent.attach",
            "agent.bring-back",
            "agent.delegate",
            "agent.credentials.import",
            "agent.credentials.remove",
            "agent.credentials.show",
            "agent.forget",
            "agent.hook",
            "agent.kinds",
            "agent.list",
            "agent.prompt",
            "agent.resume",
            "agent.start",
            "agent.status",
            "agent.stop",
            "agent.tree",
            "settings.get",
            "settings.list",
            "settings.reset",
            "settings.set",
            "device.forget",
            "device.list",
            "device.revoke",
            "device.whoami",
            "extension.catalogue",
            "extension.disable",
            "extension.enable",
            "extension.inspect",
            "extension.install",
            "extension.list",
            "extension.remove",
            "extension.show",
            "git.abort",
            "git.commit",
            "git.branch-create",
            "git.branch-delete",
            "git.branch-rename",
            "git.branches",
            "git.diff",
            "git.fetch",
            "git.discard",
            "git.log",
            "git.patch",
            "git.pull",
            "git.push",
            "git.stage",
            "git.status",
            "git.switch",
            "git.unstage",
            "git.watch",
            "host.memory",
            "pair.offer",
            "project.add",
            "project.list",
            "project.rm",
            "project.show",
            "secret.decline",
            "secret.link",
            "secret.list",
            "secret.remove",
            "secret.request",
            "secret.requests",
            "secret.reset-key",
            "secret.set",
            "secret.status",
            "secret.unlink",
            "server.status",
            "service.info",
            "service.restart",
            "service.status",
            "service.stop",
            "workspace.create",
            "workspace.list",
            "workspace.rm",
            "workspace.show",
            "terminal.attach",
            "terminal.close",
            "terminal.create",
            "terminal.kill",
            "terminal.list",
            "terminal.resize",
            "terminal.screen",
            "terminal.send",
            "terminal.show",
            "team.board",
            "team.bot.add",
            "team.bot.list",
            "team.bot.update",
            "team.bot.rm",
            "team.create",
            "team.list",
            "team.memory.add",
            "team.memory.list",
            "team.memory.rm",
            "team.project.add",
            "team.project.list",
            "team.project.rm",
            "team.project.update",
            "team.rm",
            "team.routine.add",
            "team.routine.list",
            "team.routine.pause",
            "team.routine.resume",
            "team.routine.rm",
            "team.routine.run",
            "team.routine.runs",
            "team.routine.update",
            "team.update",
            "team.show",
            "task.create",
            "task.done",
            "task.start",
            "task.list",
            "task.reopen",
            "task.rm",
            "task.show",
            "task.update",
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
