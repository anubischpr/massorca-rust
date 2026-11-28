//! The `massorca` skill (SKL-04): the guide an agent loads to run this build's command. The command reference is generated from
//! the clap definitions the command runs with, so every command and flag it names exists in this build. The checked-in
//! `packages/skills/massorca/SKILL.md` must equal what this build renders; a test fails when they drift.

use clap::{Arg, ArgAction, Command, CommandFactory};

use crate::args::Cli;

/// The name the skill is placed under: its folder in each agent tool, and its `name` in the front matter.
pub const SKILL_NAME: &str = "massorca";

/// The skill text of this build.
pub fn massorca_skill_text() -> String {
    let root = Cli::command();
    let mut lines = vec![
        "---".to_owned(),
        format!("name: {SKILL_NAME}"),
        "description: Runs and controls MassOrca from its massorca command line: services, workspaces, projects, agents, skills, extensions and settings. Use it when a task needs one of those commands.".to_owned(),
        "---".to_owned(),
        String::new(),
        "# massorca".to_owned(),
        String::new(),
        format!(
            "This guide is generated from the `massorca` command of MassOrca {}. Every command and flag below exists in that build. If a command or flag is refused, run `massorca --version` and tell the person that the installed build differs from this guide.",
            env!("CARGO_PKG_VERSION")
        ),
        String::new(),
        "Run `massorca <command> --help` for the full help of one command. Commands that change data, such as removing a workspace or a skill, run only when the person asked for that change.".to_owned(),
        String::new(),
        "## Global options".to_owned(),
        String::new(),
    ];
    for arg in root.get_arguments().filter(|arg| arg.is_global_set()) {
        lines.push(format!("- `{}`: {}", argument_usage(arg), help_of(arg)));
    }
    lines.push(String::new());
    lines.push("## Commands".to_owned());
    lines.push(String::new());
    lines.push("Notation: `<NAME>` is required, `[NAME]` is optional, `[--flag]` is an optional switch, and `...` after an option means it repeats.".to_owned());
    for group in subcommands(&root) {
        lines.push(String::new());
        lines.push(format!("### {}", group.get_name()));
        lines.push(String::new());
        command_lines(group, "massorca", &mut lines);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// The subcommands of a command, without clap's own `help`.
fn subcommands(command: &Command) -> impl Iterator<Item = &Command> {
    command
        .get_subcommands()
        .filter(|sub| sub.get_name() != "help")
}

/// One bullet per leaf command, with the path of the groups above it.
fn command_lines(command: &Command, prefix: &str, lines: &mut Vec<String>) {
    let path = format!("{prefix} {}", command.get_name());
    if command.has_subcommands() {
        for sub in subcommands(command) {
            command_lines(sub, &path, lines);
        }
        return;
    }
    lines.push(command_line(command, &path));
}

/// A leaf command as one bullet: its usage, what it does, and the values its options take.
fn command_line(command: &Command, path: &str) -> String {
    let mut usage = path.to_owned();
    let mut takes = Vec::new();
    for arg in command
        .get_arguments()
        .filter(|arg| !arg.is_global_set() && !arg.is_hide_set())
    {
        usage.push(' ');
        usage.push_str(&argument_usage(arg));
        let values: Vec<String> = arg
            .get_possible_values()
            .iter()
            .filter(|value| !value.is_hide_set())
            .map(|value| value.get_name().to_owned())
            .collect();
        if arg.get_action().takes_values() && !values.is_empty() {
            takes.push(format!(
                "`--{}` takes {}.",
                arg.get_long().unwrap_or_default(),
                values.join(", ")
            ));
        }
    }
    let mut line = format!("- `{usage}`");
    let about = sentence(
        &command
            .get_about()
            .map(|about| about.to_string())
            .unwrap_or_default(),
    );
    if !about.is_empty() {
        line.push_str(&format!(": {about}"));
    }
    for note in takes {
        line.push(' ');
        line.push_str(&note);
    }
    line
}

/// How an argument appears in a usage line: `<NAME>`, `[NAME]`, `--flag <NAME>` or `[--flag]`, with `...` when it repeats.
fn argument_usage(arg: &Arg) -> String {
    if arg.is_positional() {
        let name = value_name(arg);
        return if arg.is_required_set() {
            format!("<{name}>")
        } else {
            format!("[{name}]")
        };
    }
    let takes_value = arg.get_action().takes_values();
    let flag = match arg.get_long() {
        Some(long) if takes_value => format!("--{long} <{}>", value_name(arg)),
        Some(long) => format!("--{long}"),
        None => format!("<{}>", value_name(arg)),
    };
    let optional = if arg.is_required_set() {
        flag
    } else {
        format!("[{flag}]")
    };
    if matches!(arg.get_action(), ArgAction::Append) {
        format!("{optional}...")
    } else {
        optional
    }
}

/// The help of an argument as one sentence.
fn help_of(arg: &Arg) -> String {
    sentence(
        &arg.get_help()
            .map(|help| help.to_string())
            .unwrap_or_default(),
    )
}

/// One line of help text, with its spaces collapsed and a full stop at its end.
fn sentence(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() || collapsed.ends_with(['.', '?', '!']) {
        collapsed
    } else {
        format!("{collapsed}.")
    }
}

fn value_name(arg: &Arg) -> String {
    arg.get_value_names()
        .and_then(|names| names.first().map(|name| name.to_string()))
        .unwrap_or_else(|| arg.get_id().as_str().to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The checked-in skill is what this build renders. When it fails, regenerate the file with
    /// `massorca skills bundled show massorca > packages/skills/massorca/SKILL.md`.
    #[test]
    fn the_checked_in_skill_is_what_this_build_renders() {
        let checked_in = include_str!("../../../packages/skills/massorca/SKILL.md");
        assert_eq!(
            massorca_skill_text(),
            checked_in,
            "the massorca skill drifted from the command definitions; regenerate it"
        );
    }

    #[test]
    fn every_leaf_command_has_a_bullet() {
        let text = massorca_skill_text();
        let mut paths = Vec::new();
        leaf_paths(&Cli::command(), "massorca", &mut paths);
        assert!(paths.iter().any(|path| path == "massorca service run"));
        for path in paths {
            assert!(text.contains(&format!("- `{path}")), "no bullet for {path}");
        }
    }

    fn leaf_paths(command: &Command, prefix: &str, out: &mut Vec<String>) {
        for sub in subcommands(command) {
            let path = format!("{prefix} {}", sub.get_name());
            if sub.has_subcommands() {
                leaf_paths(sub, &path, out);
            } else {
                out.push(path);
            }
        }
    }
}
