# 0045: Running the service headless

Status: accepted, 2026-11-21.

## Context

0003 says the headless server is the same `service run` with a network
listener. It must run unattended on a Linux server, survive logouts and
reboots, keep its data in a known place, and never expose itself on a public
interface by accident.

## Decision

**Command.** `massorca service run --listen <ADDR>` adds the network
listener; the local socket is always there too. `--listen` may be repeated and
also reads `MASSORCA_LISTEN` (comma separated). `<ADDR>` is one of:

| Form | Binds to |
|---|---|
| `tailscale` or `tailscale:<port>` | this node's Tailscale IPv4 address (0043) |
| `localhost:<port>` | `127.0.0.1` |
| `<ip>:<port>` | that address, which must belong to this machine |

The default port is 7347. An unspecified address (`0.0.0.0`, `::`) is refused
unless `--allow-all-interfaces` is also given, so the public interface is
never the default. A service started by the CLI's autostart has no network
listener.

**Data.** Unchanged from 0004: one profile directory under `MASSORCA_HOME`.
The network adds `<profile>/network/` (server key and certificate) and the
`devices` table in the store.

**Logs.** The service log directory of 0006, plus stderr under systemd when
`--log-stderr` is given (the unit passes it, so `journalctl --user -u
massorca` shows the log). Pairing secrets, offers and keys are never logged.

**Linux service.** A systemd user unit, so no root is needed:

```ini
[Unit]
Description=MassOrca service
After=network-online.target

[Service]
Environment=MASSORCA_HOME=%h/.massorca/home
ExecStart=%h/.massorca/bin/massorca service run --listen tailscale --log-stderr
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

`massorca service unit` prints this unit for the given `--listen` and paths;
`host add-ssh` installs it. To run without a login session the user needs
lingering (`loginctl enable-linger`); where that is not allowed without root,
the documented step is for the administrator. Exit code 3 (another service
holds the profile) is a stable answer for supervisors (0003).

**Upgrades.** Replace the binary, then `systemctl --user restart massorca`.
Clients reconnect; paired devices keep their access because the key and the
store stay. The protocol rules of 0002 cover mixed versions.

## Alternatives

- **A system unit running as a dedicated user.** Better isolation for shared
  servers, but needs root to install. The same `service run` fits one later.
- **A config file for the listener.** Flags and an environment variable are
  enough for one setting and keep the unit self-describing.

## Consequences

- An install never opens a firewall port: the tailnet address is reachable
  only through Tailscale.
- Removing an install is: stop and disable the unit, delete it, delete
  `~/.massorca`.
