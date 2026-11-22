# 0044: Remote hosts reached over SSH

Status: accepted, 2026-11-21.

## Context

A desktop must be able to reach a machine that has no MassOrca server yet:
the VPS before installation, or any Linux machine the user can SSH into. The
old app deploys a relay daemon per SSH host and keeps a separate managed
server (`orcad`), with a migration between the two (REM-08, REM-18). The
remote-hosts inventory asks whether one host daemon should do both.

## Decision

**One host daemon.** The remote host runs the same `massorca service run` as
any server. There is no relay and no second daemon. An SSH host and a paired
server are the same thing on the client: a host with a pinned key and a
device credential (0041, 0046). SSH is only how it gets installed and paired.

**Bootstrap: `massorca host add-ssh <ssh-target> --name <host>`.**

1. Runs the system `ssh` (OpenSSH, present on Windows, macOS and Linux) in
   batch mode, so it uses the user's `~/.ssh/config`, `known_hosts` and agent.
   Host key checking is OpenSSH's own; we never pass options that weaken it.
   A host OpenSSH does not trust fails the bootstrap.
2. Checks the remote system (`uname -sm`): Linux on x86_64 or aarch64. macOS
   and Windows hosts are later.
3. Uploads the `massorca` Linux binary given with `--binary` (checked to be an
   ELF file for that machine) into `~/.massorca/bin/` through the SSH channel,
   atomically (`.new`, then rename).
4. Writes a systemd user unit `~/.config/systemd/user/massorca.service`
   (0045) with `MASSORCA_HOME=%h/.massorca/home` and the chosen `--listen`
   (default `tailscale`), then enables and (re)starts it.
5. Waits for the service, asks it over SSH, as the owner on its local socket,
   for a pairing offer valid two minutes, and pairs with it. The offer travels
   only inside the SSH channel and the client process; it is never printed.

Running `add-ssh` again upgrades the binary and restarts the service; paired
devices keep their access. `host remove` revokes this device on the server
when it is reachable, then forgets it locally; `--uninstall` also stops and
deletes the unit and `~/.massorca` over SSH.

**The execution host owns execution.** Terminals, git and workspaces of a
remote host live in its service and its store. The client keeps only what it
needs to connect. When the client cannot reach a host, its state is
`unverifiable`, never `exited`: the CLI reports the host as unreachable and
claims nothing about the work on it.

## Alternatives

- **An in-process SSH library (`russh`).** Needed later for passphrase
  prompts in the desktop UI and for hosts without OpenSSH. The system binary
  gives the user's exact configuration, agent and host-key policy today.
- **A relay per SSH host (the old app).** Two daemons and a migration between
  them; rejected now that the server is one binary.
- **Reaching the service through an SSH tunnel instead of the network
  listener.** Possible later for hosts with no network path; this phase uses
  the network listener so the same host serves phones.

## Consequences

- A host needs a Linux build of `massorca`; the client does not build one.
- Uninstalling is one directory and one unit file.
