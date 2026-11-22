# 0074 Live hosts and pairing calls in the desktop

Status: accepted, 2026-11-22.

## Context

The desktop's Settings hosts rows were planned: "Add host" and "Paired phones" had no backing. The service already
answers `device.list`, `device.revoke`, `pairing.create` and `network.status` (decision 0041). The client can pair with
an offer and connect to a paired host (`Client::pair`, `Client::connect_host`). The CLI installs the service over SSH
with a plan, an ssh transport and the bootstrap (decision 0044), but that sequence lived in the CLI crate.

## Decision

- The four service methods get typed helpers in `massorca-client` (`device_calls.rs`), in the style of the secret helpers.
- The SSH install moves into `massorca-core-remote` (`BootstrapPlan::for_ssh_host`, `install_over_ssh`, `DEFAULT_LISTEN`).
  The default device name moves into `massorca-client`. The CLI and the desktop call the same functions.
- Host calls (refresh, connection probes, accepting an offer, adding a host over SSH) run in the bridge outside the
  session with this computer's service. Each opens its own connections, so the hosts work while the local service is
  down.
- Device, offer and network calls go through the local session and are refused while it is down.
- The store changes only from answers: `HostsResync` and `HostLinkChanged` for hosts, `Devices` and `Network` for the
  local service. A revoke that worked re-reads the device list. Nothing is updated optimistically.
- A refused handshake keeps the service's code and sentence. Only a handshake the service never answered means the
  service cannot be reached. Client-side host failures use the CLI's `host.*` codes and wording.

## Alternatives

- The SSH install in `massorca-client`: rejected. The protocol client would gain a process-spawning dependency.
- A new crate for the composition: rejected for now. The shared parts fit existing crates.
- Host calls through the local session: rejected. Remote hosts must stay manageable while this computer's service is down.

## Consequences

- The desktop depends on `massorca-core-remote` (a new workspace edge; the lock change is its own commit).
- A pairing the service refuses now shows the service's own sentence, not "The service cannot be reached right now".
- The SSH add needs the path of a Linux `massorca` program to upload. The design has no field for it yet (owner input).
- `crates/desktop/tests/hosts_bridge.rs` drives a real loopback service through the bridge.
