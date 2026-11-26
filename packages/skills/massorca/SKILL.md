---
name: massorca
description: Runs and controls MassOrca from its massorca command line: services, workspaces, projects, agents, skills, extensions and settings. Use it when a task needs one of those commands.
---

# massorca

This guide is generated from the `massorca` command of MassOrca 0.1.0. Every command and flag below exists in that build. If a command or flag is refused, run `massorca --version` and tell the person that the installed build differs from this guide.

Run `massorca <command> --help` for the full help of one command. Commands that change data, such as removing a workspace or a skill, run only when the person asked for that change.

## Global options

- `[--profile <NAME>]`: Profile to use. Each profile has its own service and data.
- `[--home <DIR>]`: Directory that holds the profiles. Defaults to the platform data directory.
- `[--host <NAME>]`: Host to address: a paired host name, or local (the default).
- `[--json]`: Print one JSON document on stdout instead of text.
- `[--no-autostart]`: Never start the service on its own; fail when it is not running.

## Commands

Notation: `<NAME>` is required, `[NAME]` is optional, `[--flag]` is an optional switch, and `...` after an option means it repeats.

### service

- `massorca service run [--log-stderr] [--listen <ADDR>]... [--allow-all-interfaces]`: Runs the service in this process until it is stopped. Prints its readiness record once it accepts connections.
- `massorca service status`: Shows whether the service is running. Never starts it.
- `massorca service info`: Shows the service's identity. Starts the service when it is not running.
- `massorca service stop`: Stops the service. Succeeds when it is not running.
- `massorca service restart`: Stops the service, then starts it again.
- `massorca service tools [--refresh]`: Shows the git, forge CLI and shell tools on the service's host. Works with --host.
- `massorca service unit [--listen <ADDR>]... [--exe <PATH>]`: Prints the systemd user unit that runs the service on a Linux server.
- `massorca service log path`: Prints the logs folder and the newest service and desktop log files. Never starts the service.

### open

- `massorca open`: Starts the MassOrca desktop app on this profile and returns at once.

### settings

- `massorca settings get <KEY> [--owner <OWNER>]`: Shows one setting and whether it still has its default value.
- `massorca settings set <KEY> <VALUE> [--owner <OWNER>]`: Changes one setting. The value is parsed as JSON when it is valid JSON, otherwise taken as text.
- `massorca settings list`: Lists every setting.
- `massorca settings reset <KEY> [--owner <OWNER>]`: Removes the stored value, so the setting's default applies again.

### project

- `massorca project add <PATH> [--name <NAME>] [--folder] [--git] [--workspaces-root <DIR>] [--icon <ICON>]`: Registers a git repository, or a folder with --folder. Without a kind, a directory inside a repository is a git project.
- `massorca project list`: Lists every project, ordered by name.
- `massorca project show <PROJECT>`: Shows one project, selected by id, name or path.
- `massorca project inspect <PATH>`: Shows what `project add` without a kind would register for a directory, without registering it.
- `massorca project rm <PROJECT> [--forget-workspaces]`: Unregisters a project. Refused while it has workspaces, unless their records are forgotten. Its files stay.
- `massorca project set-base <PROJECT> [BASE] [--clear]`: Sets the base that a project's new workspaces start from, or clears it with --clear.
- `massorca project update <PROJECT> [--icon <ICON>] [--clear-icon]`: Changes a project's icon: `--icon` sets it, `--clear-icon` removes it so the project shows the default one.
- `massorca project scripts <PROJECT> [--workspace <WORKSPACE>]`: Shows a project's orca.yaml setup and archive scripts with their hashes and whether each is trusted.
- `massorca project trust-scripts <PROJECT> [--workspace <WORKSPACE>] [--hash <HASH>]...`: Shows a project's orca.yaml setup and archive scripts with their hashes; `--hash` trusts the shown script with that hash. A script runs only once its exact content is trusted, and a changed script needs trust again.

### workspace

- `massorca workspace create <PROJECT> [NAME] [--branch <BRANCH>] [--base <REF>] [--existing-branch] [--refresh-base] [--path <DIR>] [--request-id <ID>] [--no-setup] [--unique] [--name-generated]`: Creates a worktree on a new branch (git project), or registers a folder (folder project).
- `massorca workspace list [--project <PROJECT>]`: Lists every workspace, or only those of one project.
- `massorca workspace show <WORKSPACE>`: Shows one workspace, selected by id, name or path.
- `massorca workspace current [--path <DIR>]`: Shows the workspace that contains the current directory, or the one that contains --path.
- `massorca workspace rm <WORKSPACE> [--force] [--discard-ignored] [--forget-orphan] [--allow-failed-archive]`: Removes a workspace. A worktree is deleted with its directory; a folder is only forgotten.
- `massorca workspace update <WORKSPACE> [--name <NAME>] [--comment <COMMENT>]`: Renames a workspace or sets its comment. A worktree's directory and branch keep their names.
- `massorca workspace rename <WORKSPACE> <NEW_NAME> --from-generated [--expected-branch <BRANCH>]`: Renames a workspace whose name the service chose, and its unpublished branch. The folder keeps its name.
- `massorca workspace check <WORKSPACE>`: Checks that a workspace's folder exists and is readable and, for a worktree, that git still lists it.

### terminal

- `massorca terminal create <WORKSPACE> [--shell <PROGRAM>] [--arg <ARG>]... [--cwd <DIR>] [--cols <N>] [--rows <N>] [--env <KEY=VALUE>]... [--title <TITLE>]`: Starts a shell, or a program with --shell, in a workspace. The terminal keeps running after this command exits.
- `massorca terminal list [--workspace <WORKSPACE>]`: Lists live and exited terminals, or only those of one workspace.
- `massorca terminal show <TERMINAL>`: Shows one terminal, selected by id.
- `massorca terminal send <TERMINAL> <TEXT> [--no-enter]`: Types text into a terminal. The arguments are joined with spaces; Enter follows unless --no-enter.
- `massorca terminal screen <TERMINAL> [--scrollback]`: Prints the terminal's visible screen as text.
- `massorca terminal attach <TERMINAL>`: Connects this terminal to the terminal's input and output. Ctrl+] detaches; the terminal keeps running.
- `massorca terminal resize <TERMINAL> --cols <COLS> --rows <ROWS>`: Sets the terminal's size.
- `massorca terminal kill <TERMINAL> [--force]`: Ends the terminal's processes. The terminal stays listed as exited until closed.
- `massorca terminal close <TERMINAL>`: Ends the terminal's processes and forgets the terminal.
- `massorca terminal wait <TERMINAL> [--for <CONDITION>] [--timeout <SECS>]`: Waits until the terminal's process exits, up to --timeout seconds, and prints its exit code. `--for` takes exit.

### profile

- `massorca profile secure [--yes]`: Makes the home private when other programs on this computer could change its files. Prints the access entries it removes and keeps, and changes them only with --yes; without it, it exits 0 having changed nothing.

### cli

- `massorca cli install`: Puts `massorca` on your PATH for this user only, without administrator rights: a link in `~/.local/bin` on Unix, a command file in a folder added to your Windows user PATH. Running it again changes nothing.
- `massorca cli uninstall`: Removes what `cli install` made: the link or command file, and its PATH entry once its folder is empty. A file massorca did not make is left in place.
- `massorca cli status`: Shows whether `massorca` is on your PATH and which binary it runs. Changes nothing.

### pair

- `massorca pair offer [--admin] [--standard] [--ttl <SECS>] [--address <HOST[:PORT]>]... [--no-qr]`: Makes a pairing offer on the service. Only its owner or a device-admin can, and it works once.
- `massorca pair accept <OFFER> [--name <HOST>] [--device-name <NAME>]`: Pairs this computer with the server of an offer, and stores the server as a host.
- `massorca pair cancel [OFFER-ID] [--all]`: Retires a pending offer before it expires, so a code that leaked no longer pairs.

### device

- `massorca device list`: Lists the paired devices, their scopes and state. Needs device-admin.
- `massorca device revoke <DEVICE>`: Revokes a device: its connections close at once and it is refused afterwards.
- `massorca device forget <DEVICE>`: Removes a revoked device from the list. Needs host-admin; an active device is refused.
- `massorca device grant <DEVICE>`: Gives a paired device full access, every scope, from its next connection. Needs host-admin.
- `massorca device whoami`: Shows the device this command runs as.

### server

- `massorca server status`: Shows the listeners, server pin, devices and pending offers; on this computer also Tailscale.

### host

- `massorca host list`: Lists the hosts this client paired with. Makes no connection.
- `massorca host show <NAME>`: Shows one host: addresses, pin, device and scopes.
- `massorca host remove <NAME> [--keep-device] [--uninstall] [--ssh <TARGET>]`: Forgets a host. Unless --keep-device, first revokes this device on the server when it answers.
- `massorca host status [NAME] [--watch]`: Connects to one host, or to every host, and reports whether it answers.
- `massorca host self-test [NAME]`: Checks one paired host step by step (address, port, key, protocol, pairing, service, round trip) and names the step that fails and how to fix it. Without a name, checks this computer's own service. Exit code 0 only when every check passes.
- `massorca host discover`: Lists the online Tailscale peers, and which of them run massorca.
- `massorca host add-ssh <TARGET> --binary <PATH> [--name <HOST>] [--listen <ADDR>]... [--ssh-arg <ARG>]... [--device-name <NAME>]`: Installs or upgrades massorca on a Linux host over SSH, then pairs with it.
- `massorca host memory`: Shows the memory of the service and of its terminals, read on the host that runs them. Works with --host.
- `massorca host ports`: Shows the TCP ports that the processes of each live terminal listen on, read on the host that runs them. Works with --host.

### agent-context

- `massorca agent-context`: Lists every command with its method, permission and parameters, for agents.

### agent

- `massorca agent kinds`: Lists the agent kinds: built in and declared in the profile, each one's package state, whether its program is detected, and any problems.
- `massorca agent models <KIND> [--account <ACCOUNT>] [--refresh]`: Lists the models a kind's picker offers for an account: the declared models, then the ones its vendor CLI lists.
- `massorca agent start <KIND> [--workspace <WORKSPACE>] [--prompt <TEXT>] [--read-only] [--title <TITLE>] [--mode <MODE>] [--model <MODEL>] [--effort <EFFORT>] [--fast] [--chat] [ARGS]`: Starts an agent in a workspace. The agent keeps running after this command exits.
- `massorca agent list [--workspace <WORKSPACE>] [--archived]`: Lists agent sessions, newest first: every session, or those of one workspace.
- `massorca agent status [SESSION] [--watch] [--until <STATE>] [--timeout <SECS>]`: Shows one session's execution and activity, or every session's when none is named. `--until` takes working, waiting, idle, done, exited.
- `massorca agent stop <SESSION> [--force]`: Ends a session's run. The session stays listed as exited until forgotten.
- `massorca agent prompt <SESSION> <TEXT> [--force]`: Types a prompt into a session's terminal once its agent is idle or done, then submits it.
- `massorca agent resume <SESSION> [--mode <MODE>] [--cols <COLS>] [--rows <ROWS>]`: Starts a new run of an exited session with the agent's resume arguments; the session keeps its id.
- `massorca agent fork <SESSION> [--prompt <PROMPT>] [--mode <MODE>] [--title <TITLE>]`: Starts a new session from a copy of an exited session's conversation, with the kind's fork arguments, in the same workspace. The source session keeps its own conversation.
- `massorca agent mode <SESSION> <MODE> [--force]`: Switches a session's permission mode: at once where the kind has a live switch, otherwise at its next resume.
- `massorca agent set-options <SESSION> [--model <MODEL>] [--effort <EFFORT>] [--fast] [--no-fast]`: Changes a session's model, effort or fast mode: typed into its run where the kind declares a live command for it, otherwise applied at the session's next resume (decision 0171).
- `massorca agent seen <SESSION>`: Marks a session's Waiting for you or Done state as seen, which clears its marker in every client.
- `massorca agent queue add <SESSION> <TEXT>`: Queues a prompt at the end of a session's queue; it is typed once the agent is idle or done.
- `massorca agent queue list <SESSION>`: Lists the queued prompts of a session, head first, with their states and reasons.
- `massorca agent queue edit <ID> <TEXT>`: Replaces the text of a waiting or failed queued prompt.
- `massorca agent queue send-now <ID>`: Sends a queued prompt at once, with force, ahead of the queue's order; it leaves the queue. Refused while the agent asks the person something.
- `massorca agent queue resume <SESSION>`: Lifts the session's queue pauses, after a stop or a service restart, and types its head once the agent is ready.
- `massorca agent queue remove <ID>`: Removes a waiting or failed queued prompt.
- `massorca agent forget <SESSION> [--remove-workspace] [--force] [--discard-ignored] [--preview]`: Forgets a session whose run has ended; with --remove-workspace, removes its workspace too, as `workspace rm` would, when no other session uses it.
- `massorca agent rename <SESSION> <TITLE>`: Sets a session's title, live or exited: 1 to 120 characters once trimmed, without control characters.
- `massorca agent hide <SESSION> [--off]`: Hides (archives) an exited session from `agent list`, keeping its record, terminal and workspace; --off shows it again. `agent list --archived` lists hidden sessions too.
- `massorca agent release <SESSION>`: Archives an exited session's output (its newest 256 KiB) and frees its terminal. The record stays. Refused for a live run.
- `massorca agent retain <SESSION> [--off]`: Keeps a session's exited terminal from eviction, or clears that with --off. A release clears it too.
- `massorca agent archive <SESSION>`: Shows the archived output of a released session.
- `massorca agent attach <SESSION>`: Connects this terminal to the session's run. Ctrl+] detaches; the agent keeps running.
- `massorca agent hook [EVENT] [PAYLOAD]`: Reports an agent run's status to the service; agents' hooks call it. Always exits 0 within 5 s.
- `massorca agent delegate [--from <SESSION>] --prompt <TEXT> [--kind <KIND>] [--name <NAME>] [--read-only] [--same-workspace] [--title <TITLE>] [--mode <MODE>]`: Starts a child session for a parent session, in a new workspace of its own unless it is read-only, the parent works in a plain folder, or --same-workspace is given; a child that shares the parent's workspace and writes makes the parent wait. The parent is told when the child finishes.
- `massorca agent tree <SESSION>`: Shows a session's delegation tree: its root and the root's children.
- `massorca agent bring-back <CHILD> [--no-merge] [--no-prompt]`: Merges a finished child's branch into its parent's worktree and tells the parent's run about it.
- `massorca agent credentials import <KIND> --from <DIR> [--replace]`: Copies the kind's declared login files from a directory into this profile. Prints names and sizes only.
- `massorca agent credentials show <KIND>`: Shows whether the profile has a home for the kind, and which declared files are in it.
- `massorca agent credentials remove <KIND>`: Deletes the profile's home for the kind; its runs use the agent's usual home again.
- `massorca agent accounts list <KIND>`: Lists the kind's accounts: label, state, identity, whether active, and the projects that chose each.
- `massorca agent accounts add <KIND> [--label <LABEL>] [--no-attach]`: Signs a new account in: runs the kind's login in a terminal, then attaches to it unless `--no-attach`.
- `massorca agent accounts sign-in <KIND> <ACCOUNT> [--no-attach]`: Signs an existing account in again: runs the kind's login in a terminal with the account's own home, then attaches to it unless `--no-attach`. The account is kept whatever the login ends with.
- `massorca agent accounts import <KIND> [--from <DIR>] [--label <LABEL>]`: Imports the kind's declared login files into a new account, then checks it. Without `--from`, each file is copied from where the kind's CLI keeps it (see `accounts candidates`), and the files it will copy are printed first.
- `massorca agent accounts candidates [KIND]`: Shows where an import without `--from` would copy each kind's login files from: per kind its folder, and each file's name, whether it is required, whether it was found, and its path. Nothing is read or copied.
- `massorca agent accounts activate <KIND> <ACCOUNT> [--project <PROJECT>]`: Makes an account the kind's active one, or the account a project uses with `--project`.
- `massorca agent accounts remove <KIND> <ACCOUNT>`: Deletes an account. Refused while a live run uses it; the kind's sign-out runs first when it declares one.
- `massorca agent accounts check <KIND> [ACCOUNT]`: Checks an account now, or the kind's active account when none is named, and prints its state.
- `massorca agent title --kind <KIND> --prompt <PROMPT> [--reply <REPLY>] [--model <MODEL>] [--account <ACCOUNT>]`: Names a chat from its first prompt and reply, by one run of the kind's one-shot form (decision 0167). Prints the slug.

### git

- `massorca git status [--workspace <WORKSPACE>] [--limit <LIMIT>]`: Shows the changed files: staged, unstaged, untracked and conflicted.
- `massorca git stage [PATHS] [--all] [--workspace <WORKSPACE>]`: Stages files, or every change with `--all`.
- `massorca git unstage [PATHS] [--all] [--workspace <WORKSPACE>]`: Unstages files, or everything with `--all`; the working files are kept.
- `massorca git discard [PATHS] [--all] [--workspace <WORKSPACE>] [--delete-untracked] [--include-staged] [--dry-run]`: Discards changes to files. Refuses to delete untracked files or drop staged changes unless told to.
- `massorca git commit [--message <MESSAGE>] [--amend] [--allow-published] [--workspace <WORKSPACE>]`: Commits the staged changes, or amends HEAD; hooks run.
- `massorca git fetch [REMOTE] [--all] [--workspace <WORKSPACE>]`: Fetches from a remote, pruning branches deleted there.
- `massorca git pull [--mode <MODE>] [--workspace <WORKSPACE>]`: Brings the branch up to date with its upstream; never merges unless `--mode` says so. `--mode` takes ff-only, merge, rebase.
- `massorca git push [REMOTE] [--force-with-lease <COMMIT>] [--workspace <WORKSPACE>]`: Pushes the branch to its upstream, creating it on the remote if needed. Never forces.
- `massorca git abort [--workspace <WORKSPACE>]`: Aborts the merge, rebase, cherry-pick or revert in progress.
- `massorca git watch [--workspace <WORKSPACE>] [--max-events <MAX_EVENTS>]`: Prints what changes in a workspace (working tree, index, HEAD, refs) until interrupted.
- `massorca git diff [--workspace <WORKSPACE>] [--staged] [--head] [--from <REV>] [--to <REV>] [--limit <LIMIT>]`: Lists the files a comparison changes, with line counts.
- `massorca git log [REVISION] [--workspace <WORKSPACE>] [--limit <LIMIT>] [--cursor <CURSOR>]`: Shows the history, newest first, a page at a time.
- `massorca git patch <PATH> [--workspace <WORKSPACE>] [--staged] [--head] [--from <REV>] [--to <REV>] [--original-path <PATH>] [--max-bytes <MAX_BYTES>]`: Shows the patch of one file.
- `massorca git branches [--workspace <WORKSPACE>]`: Lists the local branches, the current one first.
- `massorca git branch-create <NAME> [--start <START>] [--switch] [--workspace <WORKSPACE>]`: Creates a branch with no upstream, at HEAD or `--start`.
- `massorca git switch <BRANCH> [--workspace <WORKSPACE>]`: Checks out a local branch; refuses when local changes would be overwritten.
- `massorca git branch-rename <FROM> <TO> [--workspace <WORKSPACE>]`: Renames a branch; never overwrites another.
- `massorca git branch-delete <BRANCH> [--force] [--workspace <WORKSPACE>]`: Deletes a merged branch, or any with `--force`; prints the commit it pointed at.

### secret

- `massorca secret status`: Shows where the vault key is kept, whether it opens, and how many secrets there are.
- `massorca secret list [--scope <SCOPE>]`: Lists secret names by scope, owned and linked. Never values.
- `massorca secret set <SCOPE> <NAME> [--replace]`: Stores a secret, or rotates one with `--replace`. The value comes from stdin or a hidden prompt.
- `massorca secret link <SCOPE> <NAME> --from <SCOPE>`: Uses the secret of the same name that another scope owns.
- `massorca secret unlink <SCOPE> <NAME>`: Removes a link; the owner's secret stays.
- `massorca secret remove <SCOPE> <NAME> [--include-links] [--force]`: Removes an owned secret. Refuses one that other scopes link to unless `--include-links`.
- `massorca secret request <SCOPE> <NAME> --reason <REASON> [--session <SESSION>]`: Asks for a secret by name, with a reason the owner sees.
- `massorca secret requests [--all]`: Lists open requests for secrets, or every request with `--all`.
- `massorca secret decline <REQUEST>`: Declines a pending request.
- `massorca secret reset-key`: Forgets the vault key while no secret exists, so the next `set` creates a fresh one.

### team

- `massorca team create <NAME> [--purpose <TEXT>] [--character <SHAPE>] [--access <ACCESS>]`: Creates a team. Its name is unique. `--character` takes triangle, square, circle, diamond, hexagon, star. `--access` takes full, default.
- `massorca team list`: Lists every team, ordered by name.
- `massorca team show <TEAM>`: Shows one team, selected by id or name.
- `massorca team rm <TEAM> [--force]`: Removes a team with its bots, projects and tasks. Refused while a task runs, unless --force.
- `massorca team update <TEAM> [--name <NAME>] [--purpose <TEXT>] [--character <SHAPE>] [--workspaces <BOOL>] [--access <ACCESS>]`: Changes a team: its name, purpose, shape or workspaces flag. Only the options given change. `--character` takes triangle, square, circle, diamond, hexagon, star. `--workspaces` takes true, false. `--access` takes full, default.
- `massorca team bot list <TEAM>`: Lists a team's bots, in the order they were added.
- `massorca team bot update <TEAM> <BOT> [--name <NAME>] [--job <TEXT>] [--agent <KIND>] [--model <MODEL>] [--effort <EFFORT>] [--lead <BOOL>] [--skills <SKILLS>]`: Changes a bot. Only the options given change; an empty model or effort clears it. `--lead` takes true, false.
- `massorca team bot add <TEAM> <NAME> --agent <KIND> --job <TEXT> [--lead] [--skills <SKILLS>]`: Adds a bot to a team. Its agent kind and job are required.
- `massorca team bot rm <TEAM> <BOT> [--unassign]`: Removes a bot. Refused while it owns open tasks, unless --unassign is given.
- `massorca team project list <TEAM>`: Lists a team's projects, in the order they were added.
- `massorca team project update <TEAM> <PROJECT> [--name <NAME>] [--goal <TEXT>] [--brief <TEXT>] [--source <PROJECT>] [--code-source <KIND>]`: Changes a team's project. Only the options given change. `--code-source` takes existing-repository, new-folder-with-git.
- `massorca team project rm <TEAM> <PROJECT>`: Removes a project. Its tasks stay on the Board without it.
- `massorca team project add <TEAM> <NAME> [--goal <TEXT>] [--source <PROJECT>] [--code-source <KIND>]`: Adds a project to a team. Its name is unique within the team. `--code-source` takes existing-repository, new-folder-with-git.
- `massorca team memory list <TEAM> [--bot <BOT>]`: Lists a team's memory lines, newest first: all of the team's, or only one bot's with --bot.
- `massorca team memory add <TEAM> <TEXT> [--bot <BOT>] [--task <TASK>]`: Adds a line of at most 500 characters: to the team's memory, or to one bot's with --bot.
- `massorca team memory rm <TEAM> <LINE>`: Removes a memory line by its id.
- `massorca team room list <TEAM> [--limit <N>]`: Lists a team's room, oldest first: who wrote each line, its text, and the task it is about.
- `massorca team room post <TEAM> <TEXT> [--bot <BOT>] [--to <BOT>] [--no-start] [--reply-to <MESSAGE>] [--ask] [--task]`: Posts a line of at most 4000 characters. As a person, the line becomes a task for the bot it names with @name, else --to, else the lead, else the only bot, and the task starts unless --no-start. With several bots and none of those, the room answers with a choice card instead (see `team room answer`). The task's title is the line's first line without the @name of the team's bots, cut to 80 characters. A person's line that names one day (YYYY-MM-DD, today, tomorrow or a weekday) and asks for one time ("at 09:30", "at 9pm", "a las 17", or any time in a line that says schedule or remind) becomes that bot's one-time routine on the host's clock instead; a routine the host refuses leaves it a task. With --bot, that bot posts the line. Inside an agent run, the line is posted as the bot of the run's own task or reply, and naming another bot is refused. With --ask, or --reply-to a bot's line, the line is a conversation instead: one bot (the one it names, else the one it replies to, else --to, else the lead, else the first) answers it once in the room, in at most 1,200 characters, and no task is made. `team room list` shows who is typing.
- `massorca team room answer <TEAM> <MESSAGE> <CHOICE>`: Answers a choice card once: `bot:<id>` gives the request to that bot, `team:<id>` puts it in another team's Todo, and `todo` keeps it in this team's Todo with no bot. `team room list` prints each card's choices.
- `massorca team routine list <TEAM>`: Lists a team's routines, in creation order.
- `massorca team routine add <TEAM> <NAME> --bot <BOT> --prompt <TEXT> --time <HH:MM> --cadence <CADENCE> [--weekday <DAY>] [--month-day <DAY>] [--date <YYYY-MM-DD>] [--zone <ZONE>]`: Adds a routine: a bot does a task at a time of day, daily, weekly, monthly or once. `--cadence` takes daily, weekly, monthly, once. `--weekday` takes mon, tue, wed, thu, fri, sat, sun.
- `massorca team routine update <TEAM> <ROUTINE> [--bot <BOT>] [--name <NAME>] [--prompt <TEXT>] [--cadence <CADENCE>] [--time <HH:MM>] [--weekday <DAY>] [--month-day <DAY>] [--date <YYYY-MM-DD>] [--zone <ZONE>]`: Changes a routine. Only the options given change; `--zone ""` returns it to the host's zone. `--cadence` takes daily, weekly, monthly, once. `--weekday` takes mon, tue, wed, thu, fri, sat, sun.
- `massorca team routine rm <TEAM> <ROUTINE>`: Removes a routine and its runs. The tasks the runs made stay on the Board.
- `massorca team routine pause <TEAM> <ROUTINE>`: Pauses a routine: it runs no more until it is resumed.
- `massorca team routine resume <TEAM> <ROUTINE>`: Resumes a paused routine from now. Runs missed while it was paused are not made up.
- `massorca team routine runs <TEAM> <ROUTINE> [--limit <N>]`: Lists a routine's runs, newest first.
- `massorca team routine run <TEAM> <ROUTINE> [--now]`: Makes one run now, for testing. The routine's schedule does not move; `--now` is required.
- `massorca team board <TEAM> [--project <PROJECT>] [--no-project]`: Shows a team's Board: its tasks in four columns, or only one project's, or only the tasks no project holds.

### task

- `massorca task create <TEAM> <TITLE> [--bot <BOT>] [--project <PROJECT>] [--body <TEXT>]`: Creates a task on a team's Board, in the Open column.
- `massorca task list [--team <TEAM>] [--column <COLUMN>]`: Lists tasks, oldest first, filtered by team or Board column. `--column` takes open, running, waiting, done.
- `massorca task show <TASK>`: Shows one task.
- `massorca task update <TASK> [--title <TEXT>] [--body <TEXT>] [--bot <BOT>] [--project <PROJECT>]`: Changes a task: its title, body, bot or project. An empty bot or project clears it.
- `massorca task start <TASK>`: Starts the task's bot on it, in the team's folder workspace. A failed start keeps the task Open with the reason.
- `massorca task done <TASK>`: Marks a task done by hand, for work that finished outside an agent.
- `massorca task reopen <TASK>`: Clears a task's outcome and error. Refused while its agent session runs.
- `massorca task stop <TASK> [--force]`: Stops the agent a task runs in. The task keeps its session and shows Stopped.
- `massorca task rm <TASK>`: Removes a task. Refused while its agent session runs.

### automations

- `massorca automations create <NAME> --project <PROJECT> --prompt <TEXT> [--agent <KIND>] [--cadence <CADENCE>] [--time <HH:MM>] [--weekday <DAY>] [--cron <EXPRESSION>] [--zone <ZONE>] [--grace <MINUTES>] [--creation-key <KEY>]`: Creates a scheduled automation in a project. It starts enabled, with its next run from its schedule. `--cadence` takes hourly, daily, weekdays, weekly, cron. `--weekday` takes mon, tue, wed, thu, fri, sat, sun.
- `massorca automations edit <AUTOMATION> [--name <NAME>] [--project <PROJECT>] [--prompt <TEXT>] [--agent <KIND>] [--cadence <CADENCE>] [--time <HH:MM>] [--weekday <DAY>] [--cron <EXPRESSION>] [--zone <ZONE>] [--grace <MINUTES>]`: Changes an automation. Only the options given change; `--zone ""` returns it to the host's zone. `--cadence` takes hourly, daily, weekdays, weekly, cron. `--weekday` takes mon, tue, wed, thu, fri, sat, sun.
- `massorca automations show <AUTOMATION>`: Shows one automation, by id or exact name.
- `massorca automations list`: Lists the automations, in creation order.
- `massorca automations remove <AUTOMATION>`: Removes an automation. It cannot be restored.
- `massorca automations enable <AUTOMATION>`: Enables an automation and schedules its next run from now.
- `massorca automations disable <AUTOMATION>`: Disables an automation; it has no next run until it is enabled.

### calendar

- `massorca calendar [--from <DATE>] [--days <N>] [--kind <KIND>] [--done]`: Lists the runs of routines and automations in a window of days, on the shared schedule (decision 0124). `--kind` takes routines, automations.

### extension

- `massorca extension list`: Lists the installed extensions.
- `massorca extension show <ID>`: Shows one extension, installed or from the catalogue.
- `massorca extension doctor <ID>`: Shows what detection sees for a package's CLIs: the path found or not found, and the PATH it searched.
- `massorca extension catalogue`: Lists the built-in catalogue and which entries are installed.
- `massorca extension inspect <SOURCE>`: Reviews a package: its risk, permissions and what installing would run. Changes nothing.
- `massorca extension install <SOURCE> [--sha256 <HEX>] [--yes] [--consent <FINGERPRINT>] [--yes-run-installer]`: Installs a package. Without --yes it prints the review and fails with extension.consent_required.
- `massorca extension update <ID> [--sha256 <HEX>] [--yes] [--consent <FINGERPRINT>]`: Updates an installed package to its source's current version, in place. Without --yes it prints the new review and fails with extension.consent_required. Runs no vendor command; an unchanged package is already up to date.
- `massorca extension rollback <ID> [--yes] [--consent <FINGERPRINT>]`: Rolls an extension back to the version its last update replaced. Without --yes it prints the review of that version and fails with extension.consent_required; the built-in catalogue needs none.
- `massorca extension enable <ID>`: Enables an installed extension.
- `massorca extension disable <ID>`: Disables an installed extension. Its files and data stay.
- `massorca extension remove <ID> [--delete-agent-homes] [--uninstall-cli] [--uninstall-consent <FINGERPRINT>] [--yes]`: Removes an installed extension, its files and its data.
- `massorca extension program-update <ID> <PROGRAM> [--yes] [--consent <FINGERPRINT>]`: Updates a host program a tool package installed, such as Git, with the vendor's update command. Without --yes it prints what would run and fails with extension.consent_required. A program MassOrca did not install is refused.
- `massorca extension log <ID>`: Lists the recent install, update, rollback and remove runs of an extension, oldest first.

### skills

- `massorca skills installed`: Lists the skills the agent tools and the registered projects keep on this host.
- `massorca skills roots`: Lists the skill roots this host reads, and the roots it skipped and why.
- `massorca skills preview <SKILL_ID> --agent <AGENTS>...`: Shows what placing a skill for agent tools would do. Changes nothing. `--agent` takes codex, claude, cursor, gemini, droid, continue, trae, grok, aug, muse, zcode.
- `massorca skills place <SKILL_ID> --agent <AGENTS>...`: Copies a skill the agent tools keep into the chosen agents' folders, and records the copies. `--agent` takes codex, claude, cursor, gemini, droid, continue, trae, grok, aug, muse, zcode.
- `massorca skills unplace <NAME> [--agent <AGENTS>]...`: Removes the copies MassOrca placed for a skill. A copy the user changed is kept. `--agent` takes codex, claude, cursor, gemini, droid, continue, trae, grok, aug, muse, zcode.
- `massorca skills managed`: Lists the skills MassOrca placed on this host.
- `massorca skills bundled list`: Lists the skills MassOrca ships, and whether each is placed on this host.
- `massorca skills bundled install <NAME> [--agent <AGENTS>]...`: Places a skill MassOrca ships for the chosen agents; without --agent, for every agent folder on this host. `--agent` takes codex, claude, cursor, gemini, droid, continue, trae, grok, aug, muse, zcode.
- `massorca skills bundled refresh <NAME>`: Brings a placed skill MassOrca ships to this build's text. A copy the person changed is kept.
- `massorca skills bundled remove <NAME>`: Removes a skill MassOrca ships from the agent folders it was placed in. A copy the person changed is kept.
- `massorca skills bundled show <NAME>`: Prints a bundled skill's text as this build renders it. The checked-in copy must equal it; no service is needed.

### marketplace

- `massorca marketplace services list`: Lists each service and its connection state on the host.
- `massorca marketplace services connect <SERVICE> [--base-url <URL>] [--email <EMAIL>] [--token-env <NAME>]`: Connects a service with a token read from stdin, a hidden prompt, or `--token-env`.
- `massorca marketplace services disconnect <SERVICE>`: Deletes what MassOrca stored for a service on the host.
