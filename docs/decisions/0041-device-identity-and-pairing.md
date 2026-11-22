# 0041: Device identity, pairing and authentication

Status: accepted, 2026-11-21.

## Context

A network connection must say who is calling, because the `Principal` of 0002
decides what each request may do. The local socket proves "same user"
through the operating system (0003); the network has no such proof. A device
(a CLI on another machine, a desktop, a phone) must be paired once, keep a
credential, and be revocable on its own. A network attacker must not be able
to pose as the server during pairing or afterwards, and there is no trust on
first use without a code the owner saw.

## Decision

**Server identity.** Each profile that listens on the network has a long-term
key pair (ECDSA P-256, generated with `rcgen`) and a self-signed certificate
for it, in `<profile>/network/identity.key` and `identity.crt` (owner-only).
Clients pin the SHA-256 of the certificate's SubjectPublicKeyInfo, written
`sha256:<base64url>`. The pin, not a CA, authenticates the server; the
certificate's names and dates are not checked. A new certificate for the same
key keeps every pin valid; a new key means pairing again.

**Device identity: client certificates.** Every client generates its own key
pair and self-signed certificate for each server it pairs with, and presents
it in the TLS handshake (mutual TLS). The server identifies a device by the
SHA-256 of that certificate's public key. The private key never leaves the
client and is never sent; TLS proves possession on every connection, so a
recorded handshake cannot be replayed. The server stores only the public key
hash.

**Pairing offer.** The owner (or a device holding `device-admin`) asks the
server for an offer: `massorca pair offer`. The server returns one string:
`mop1.` followed by base64url JSON with the server name, its addresses, the
server pin, a 256-bit one-time secret, the scopes and the expiry. The server
keeps only the SHA-256 of the secret, in memory. The string is shown once to
the person who asked; it is a credential and is never logged.

**Pairing.** The client parses the offer, connects to an address in it with
the pin from the offer (so it cannot reach an impostor even on first contact),
presents its new certificate, and sends `hello` with
`auth: {"method":"pair","secret":...,"name":...}`. The server compares the
secret hash in constant time, consumes the offer (one use), and records the
device: id, name, public key hash, scopes, paired time. The connection then
continues as that device. The client stores the host (0046).

- An offer expires after 10 minutes by default, 1 hour at most. A used,
  unknown or wrong secret is `pairing.invalid`; an expired one is
  `pairing.expired`. At most 16 offers are pending.
- Failed pairing attempts are limited per source (5 in 5 minutes; an IPv4
  address, or an IPv6 /64). Past the limit every pairing attempt from that
  source, right or wrong, is `pairing.rate_limited` until the window passes.
  Once 30 failures from all sources together are in the window, any source
  that has failed at least once is limited too; a source with no failures
  can always pair, so nobody can lock pairing for everyone.
- An offer is spent only when its device is stored. If storing the device
  fails, the offer stays usable.
- A device connection is registered for revocation before its row is checked
  a second time, so a revocation that lands while the device authenticates
  either closes the connection or refuses it. With a 256-bit secret, guessing is not the threat; the limit bounds
  noise and work.
- Possession of the offer is the owner's approval: only the owner or a
  device-admin can create one, and it reaches the device through a channel
  the owner chose (copy and paste, SSH output, later a QR code). There is no
  second approval prompt in this phase.

**Every later connection.** `hello` carries `auth: {"method":"device"}`. The
server looks up the client certificate's key hash: unknown is
`auth.unknown_device`, revoked is `auth.device_revoked`, and a network
connection without a client certificate or `auth` is `auth.required`. All of
them answer `rejected` and close. On success the connection's principal is
`Device { id, scopes }`, its last-seen time is updated, and `device.whoami`
returns the device row.

**Scopes.** Permission classes are `read`, `workspace`, `host-admin` and the
new `device-admin` (offers, device list and revoke). The local owner holds all
of them. A device holds the classes chosen when its offer was made: by default
`read` and `workspace`; `--admin` adds `host-admin` and `device-admin`.
Scopes never widen after pairing; a device that needs more is paired again.

**Device list and revoke.** `device.list` and `device.revoke` need
`device-admin`; any device may revoke itself (unpairing). Revoking marks the
row revoked, closes every open connection of that device at once, and later
handshakes fail with `auth.device_revoked`. Rows stay listed as revoked.

**Where credentials live.**

| Platform | Client key | Server key |
|---|---|---|
| Linux, macOS | `<profile>/hosts/<host>/device.key`, mode `0600`, directory `0700` | `<profile>/network/identity.key`, `0600` |
| Windows | the same path under `%APPDATA%\MassOrca`, which only the user can read | the same |
| Android (Phase 6) | app-private storage; moving the key into the Android Keystore is a Phase 6 decision | not applicable |

## Alternatives

- **Short typed codes with a PAKE (SPAKE2).** Nicer to type, but needs a PAKE
  bound to the TLS session and a crate with less review behind it. The long
  offer is pasted, read from SSH output, or scanned. A typed code can be added
  later on top of the same device model.
- **Bearer tokens instead of client certificates.** Simpler and pass through
  TLS-terminating proxies, but a stolen token works from anywhere and must be
  stored as a secret on the server too. Client certificates keep the secret on
  the device.
- **Trust on first use.** Rejected: an attacker on the first connection would
  be trusted forever.
- **A public CA (Let's Encrypt, Tailscale HTTPS certificates).** Needs a DNS
  name and renewal, and still does not identify devices. A pin is simpler and
  does not depend on outside services.

## Consequences

- Pairing never sends anything secret unencrypted, and never connects to an
  unpinned server.
- A TLS-terminating proxy in front of the server breaks device
  authentication; only TCP pass-through works (0043).
- Losing the server key means every device pairs again; this is the documented
  recovery.
