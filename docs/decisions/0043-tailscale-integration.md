# 0043: Tailscale integration

Status: accepted, 2026-11-21.

## Context

The owner uses Tailscale to reach the VPS and wants the product to use it to
reach and configure a remote server. Tailscale is optional: many users do not
have it, and the product must work without it.

## Decision

**How we read Tailscale.** Through the local `tailscale` CLI, never with
credentials or the admin API: `tailscale status --json` gives the backend
state, this node's addresses and DNS name, and the peers with their addresses,
host names, operating system and online state. The CLI is looked up on `PATH`,
then in its standard install locations (`C:\Program Files\Tailscale\tailscale.exe`
on Windows, `/Applications/Tailscale.app/Contents/MacOS/Tailscale` on macOS).
Reading status needs no root. A missing CLI, a stopped daemon or a backend
state other than `Running` all mean "no tailnet", with the reason kept for
display; nothing fails because Tailscale is absent.

**What the product does with it.**

1. **Listen on the tailnet by default.** `service run --listen tailscale` (or
   `tailscale:<port>`) binds to this node's Tailscale IPv4 address. A
   headless install made by `host add-ssh` uses it. When the address is not
   there yet (the service started before `tailscaled`), the listener retries
   every 5 seconds while the local socket already serves.
2. **Offer tailnet peers as hosts.** `massorca host discover` lists the online
   peers and marks those that answer on the MassOrca port. A peer becomes a
   host by pairing (0041) or by SSH bootstrap (0044).
3. **Advertise tailnet addresses.** A pairing offer lists the addresses the
   listener is bound to; with `--listen tailscale` that is the tailnet
   address, and the MagicDNS name is added when Tailscale reports one.
4. **Report it.** `massorca server status` shows the Tailscale state, this
   node's tailnet addresses and whether the listener is on them.

**`tailscale serve`.** Not used by default. An operator who prefers the
listener on loopback can expose it with TCP pass-through
(`tailscale serve --bg --tcp <port> tcp://127.0.0.1:<port>`). HTTPS mode must
not be used, because it terminates TLS and breaks device certificates (0041).
This is documented, not automated.

## Alternatives

- **The Tailscale local API socket directly.** Same data without a process
  spawn, but its location and permissions differ per platform; the CLI hides
  that and is what users already have.
- **`tsnet` (embedding Tailscale).** Go only, and would make the product a
  tailnet node of its own with its own login. Rejected.

## Consequences

- No Tailscale credentials, keys or admin settings are ever touched.
- A Tailscale outage leaves a `--listen tailscale` service local-only until the
  address returns; clients see the host as unreachable, never as stopped.
