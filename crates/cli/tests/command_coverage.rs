//! Every CLI command except `service run` has a command descriptor in the server's core table,
//! except the commands that run on this computer alone.

use std::collections::BTreeSet;

use clap::CommandFactory;
use massorca_cli::agent_context::LOCAL_COMMANDS;
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
fn every_cli_command_with_a_service_request_has_a_core_descriptor() {
    let mut paths = Vec::new();
    command_paths(&Cli::command(), "", &mut paths);
    assert!(
        paths.iter().any(|path| path == "service.run"),
        "the walk must see `service run`: {paths:?}"
    );
    for &client_only in LOCAL_COMMANDS {
        assert!(
            paths.iter().any(|path| path == client_only),
            "`{client_only}` is not a CLI command"
        );
    }

    let ids: BTreeSet<String> = core_commands()
        .into_iter()
        .map(|descriptor| descriptor.id)
        .collect();
    for &client_only in LOCAL_COMMANDS {
        assert!(
            !ids.contains(client_only),
            "`{client_only}` makes no service request and must not have a descriptor"
        );
    }
    let checked: BTreeSet<&str> = paths
        .iter()
        .map(String::as_str)
        .filter(|path| !LOCAL_COMMANDS.contains(path))
        .collect();

    assert_eq!(
        checked,
        BTreeSet::from([
            "agent-context",
            "agent.accounts.activate",
            "agent.accounts.add",
            "agent.accounts.candidates",
            "agent.accounts.check",
            "agent.accounts.import",
            "agent.accounts.list",
            "agent.accounts.remove",
            "agent.accounts.sign-in",
            "agent.archive",
            "agent.attach",
            "agent.bring-back",
            "agent.delegate",
            "agent.credentials.import",
            "agent.credentials.remove",
            "agent.credentials.show",
            "agent.forget",
            "agent.fork",
            "agent.hide",
            "agent.hook",
            "agent.kinds",
            "agent.list",
            "agent.mode",
            "agent.models",
            "agent.prompt",
            "agent.rename",
            "agent.queue.add",
            "agent.queue.edit",
            "agent.queue.list",
            "agent.queue.remove",
            "agent.queue.resume",
            "agent.queue.send-now",
            "agent.release",
            "agent.resume",
            "agent.retain",
            "agent.seen",
            "agent.set-options",
            "agent.start",
            "agent.status",
            "agent.stop",
            "agent.title",
            "agent.tree",
            "automations.create",
            "automations.disable",
            "automations.edit",
            "automations.enable",
            "automations.list",
            "automations.remove",
            "automations.run",
            "automations.runs",
            "automations.show",
            "calendar",
            "settings.get",
            "settings.list",
            "settings.reset",
            "settings.set",
            "skills.installed",
            "skills.roots",
            "skills.preview",
            "skills.place",
            "skills.unplace",
            "skills.managed",
            "device.forget",
            "device.grant",
            "device.list",
            "device.revoke",
            "device.whoami",
            "extension.catalogue",
            "extension.disable",
            "extension.doctor",
            "extension.enable",
            "extension.inspect",
            "extension.log",
            "extension.points",
            "extension.program-update",
            "extension.install",
            "extension.list",
            "extension.remove",
            "extension.rollback",
            "extension.show",
            "extension.update",
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
            "git.remotes",
            "git.show-file",
            "git.stage",
            "git.status",
            "git.switch",
            "git.unstage",
            "git.watch",
            "host.memory",
            "host.ports",
            "host.self-test",
            "marketplace.services.connect",
            "marketplace.services.disconnect",
            "marketplace.services.list",
            "pair.cancel",
            "pair.offer",
            "project.add",
            "project.init-git",
            "project.inspect",
            "project.list",
            "project.rm",
            "project.set-base",
            "project.scripts",
            "project.show",
            "project.trust-scripts",
            "project.update",
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
            "service.tools",
            "workspace.check",
            "workspace.create",
            "workspace.current",
            "workspace.list",
            "workspace.rename",
            "workspace.rm",
            "workspace.show",
            "workspace.update",
            "terminal.attach",
            "terminal.close",
            "terminal.create",
            "terminal.kill",
            "terminal.list",
            "terminal.resize",
            "terminal.screen",
            "terminal.send",
            "terminal.show",
            "terminal.wait",
            "team.board",
            "team.bot.add",
            "team.bot.list",
            "team.bot.update",
            "team.bot.rm",
            "team.create",
            "team.delegate",
            "team.list",
            "team.memory.add",
            "team.memory.list",
            "team.memory.rm",
            "team.project.add",
            "team.project.list",
            "team.project.rm",
            "team.project.show",
            "team.project.update",
            "team.rm",
            "team.room.answer",
            "team.room.list",
            "team.room.mentions",
            "team.room.post",
            "team.room.resolve",
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
            "task.stop",
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
