# 0046: How a client addresses a server

Status: accepted, 2026-11-21.

## Context

The CLI and the desktop talk to the local service of their profile (0003,
0004). After Phase 3 they also talk to remote servers. A command must say
which one it means, and the client must remember the servers it paired with.

## Decision

**Hosts.** A client profile keeps a list of hosts: the remote servers it
paired with. Each host has a name (the same rules as a profile name), the
addresses from its pairing offer, the server pin, the device id the server
gave this client, and the device key. They live in the profile directory:

```
<profile>/hosts/<name>/host.json    name, addresses, pin, device id, paired time
<profile>/hosts/<name>/device.key   this client's key for that host (owner-only)
<profile>/hosts/<name>/device.crt   its certificate
```

A file per host keeps each host independent: removing one is deleting its
directory, and a damaged file affects only that host.

**Choosing the target.** Every command that talks to a service takes the
global `--host <name>` (environment `MASSORCA_HOST`). Without it the command
uses the local service of the profile, as before, including autostart. With
it the command connects over the network to that host and never autostarts
anything. `--host local` means the local service explicitly.

**Commands.**

| Command | Where it runs |
|---|---|
| `pair offer` | on the server (local owner, or a device with `device-admin` through `--host`) |
| `pair accept <offer> --name <host>` | on the client; stores the host |
| `device list`, `device revoke`, `device whoami` | against the chosen service |
| `host list`, `host show`, `host remove` | the client's own host list |
| `host status [<name>]` | connects to each host and reports `connected`, `unreachable` or a refusal |
| `host discover` | tailnet peers (0043) |
| `host add-ssh` | SSH bootstrap (0044) |
| `server status` | the chosen service's network state |

A host that cannot be reached is `unreachable`, and nothing is inferred about
the work on it (0044).

## Alternatives

- **One `hosts.json` file.** One damaged write loses every host.
- **Hosts in the SQLite store.** The store belongs to the service; the hosts
  belong to the client and are read without a service running.
- **A URL per command (`--server wss://...`).** Leaves the pin and key with
  nowhere to live; a name is what people type.

## Consequences

- The desktop and the phone reuse the same host list through the `client`
  crate.
- Scripts address a host by name and get the same exit codes as locally, with
  code 4 when the host cannot be reached.
