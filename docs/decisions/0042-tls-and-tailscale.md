# 0042: TLS on the network, also inside Tailscale

Status: accepted, 2026-11-21.

## Context

The owner reaches the VPS over Tailscale, which already encrypts and
authenticates traffic between machines of the tailnet. Three options: our own
TLS with pinning (0041), relying only on Tailscale, or both.

## Decision

Both. Every network connection uses TLS with the pinned server key and a
device certificate, whether or not it runs inside a tailnet. Tailscale is the
default way to reach the server (0043), and the listener binds to the
tailnet address by default, so the service is not exposed on the public
interface.

Why not Tailscale alone: Tailscale proves which machine is calling, not which
paired device or user is calling, and a tailnet can hold shared nodes, other
users' devices and tagged servers. Revoking one phone must not mean editing
tailnet ACLs. The same build must also work on a LAN or behind an SSH tunnel
where there is no Tailscale.

**What an operator does.** Nothing for certificates: the key and certificate
are generated on the first `service run --listen`, the pin travels inside the
pairing offer, and there is no renewal. Rotating the identity (after a
suspected leak) is deleting `<profile>/network/` and pairing the devices
again. The operator chooses where the service listens (0045) and keeps
firewalls closed: the default binding needs no open port.

## Alternatives

- **Tailscale only, plain WebSocket.** Less code, but no per-device identity
  or revocation, and plaintext on any network that is not a tailnet.
- **Tailscale HTTPS certificates (`tailscale cert`).** Gives a browser-trusted
  name, but needs MagicDNS and HTTPS enabled in the tailnet admin, renewal
  every 90 days, and still no device identity.

## Consequences

- Double encryption inside the tailnet costs a few percent of CPU on bulk
  terminal output; acceptable.
- The security of the product does not depend on how the tailnet is
  configured.
