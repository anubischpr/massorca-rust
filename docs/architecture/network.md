# Network architecture

This is the network half of the core: how a service is reached from another
machine, how devices pair and authenticate, and how the product uses Tailscale
and SSH. Each choice has a record in `docs/decisions/` (0040 to 0047); this
page shows how they fit and states the threat model.

## Shape

```
 client machine                                  server machine (VPS)
 massorca --host vps ...                         massorca service run --listen tailscale
   client crate                                    server crate
     hosts/vps/{host.json, device.key}               network/{identity.key, identity.crt}
     net: TCP -> TLS 1.3 (pin, client cert)  ====>   net: TLS 1.3 (any client cert)
          -> WebSocket /massorca/v1                       -> WebSocket
          -> frames (0002, one per message)               -> frames -> handshake -> dispatcher
                                                       store.sqlite3: devices table
                                                       local socket (owner) as before
```

The connection code above the frame transport is the same for the local
socket and the network. The only network-specific steps are TLS, the
WebSocket upgrade and how the handshake sets the principal.

## A network connection, step by step

1. The client resolves `--host vps` to `hosts/vps/host.json`: addresses, the
   server pin, its device key.
2. TCP to the first address that answers, then TLS 1.3. The client accepts
   the server only if the SHA-256 of its certificate's public key equals the
   pin. The client presents its own certificate.
3. WebSocket upgrade on `/massorca/v1`. Each binary message is one frame of
   0002.
4. `hello` with `auth: {"method":"device"}`. The server maps the client
   certificate's key hash to a device row. Unknown, revoked or missing
   credentials are refused with `rejected` and the connection closes.
5. `welcome` with the capabilities and `max_frame_len` 1 MiB. Requests run
   with principal `Device { id, scopes }`; every request passes
   `authorize(principal, permission)` as before.

Pairing is the same sequence with the pin and address taken from the offer,
a freshly generated device key, and `auth: {"method":"pair", ...}` (0041).

## Permissions

| Class | Owner (local socket) | Device, default | Device, `--admin` |
|---|---|---|---|
| `read` | yes | yes | yes |
| `workspace` | yes | yes | yes |
| `host-admin` | yes | no | yes |
| `device-admin` | yes | no | yes |

A device may always call `device.whoami` and revoke itself.

## Threat model

**Assets.** The ability to run commands on the server (terminals run shells,
so a workspace permission is code execution as the service user); the
content of terminals and repositories; device credentials; the server key.

**Adversaries considered.**

| Adversary | Defence |
|---|---|
| A network attacker who reads, changes or injects traffic, including on the first pairing | TLS 1.3 with the server pinned from the offer: no unpinned connection is ever made, so there is no first-use window |
| Someone who replays a recorded connection or a pairing | TLS session keys are fresh per connection; device authentication is a signature over the handshake; offers are single use and expire |
| A client that was never paired, or sends random secrets | `auth.unknown_device`, `pairing.invalid`; pairing attempts are rate limited per address, and the overall limit never locks out an address that has not failed; unauthenticated connections are cut after 10 seconds, capped overall and per address, and read one small frame |
| A lost or stolen device | `device revoke` closes its connections at once and refuses it afterwards |
| A paired device that should do less | scopes chosen at pairing, never widened; each request is authorized |
| Other users on the server machine | the local socket is owner-only (0003); key files are owner-only; a device key never leaves its client |
| Another tailnet member | the service still requires a paired device; Tailscale is transport, not identity (0042) |
| Exposure on a public interface by mistake | the default binding is the tailnet address; unspecified addresses need an explicit flag (0045) |

**Out of scope.** A compromised client machine or server account (it holds
the keys); a malicious process of the same user on the server (0003 known
limit); denial of service by a party that can drop traffic; a hostile
Tailscale coordination server beyond what TLS pinning already covers.

**Rules that keep this true.**

- Pairing offers, secrets and private keys are never written to logs, test
  fixtures or the repository; `Debug` of every type that holds one prints
  `<redacted>`.
- Secret comparisons are constant time.
- Only `net` handles keys, certificates and TLS configuration.
- Tests use generated throwaway keys and offers.

## Tailscale and SSH

- `--listen tailscale` binds to the node's tailnet IPv4 address, read from
  `tailscale status --json`. `host discover` lists tailnet peers (0043).
- `host add-ssh` installs or upgrades the service on a Linux host through the
  system `ssh`, as a systemd user unit, and pairs with it through the SSH
  channel (0044, 0045). An unreachable host is `unreachable`; the work on it
  is `unverifiable`, never `exited`.

## Limits

| Limit | Value |
|---|---|
| Network frame | 1 MiB |
| Network connections | 128 (closed before TLS over that) |
| Unauthenticated connections | 32, and 4 per address |
| Frame read before `welcome` | one, at most 64 KiB |
| Time to complete TLS, upgrade and `hello` | 10 s |
| Ping interval, silence before close | 20 s, 60 s |
| Pairing offer lifetime | 10 min default, 1 h maximum |
| Pending offers | 16 |
| Failed pairing attempts | 5 per address (IPv6: per /64) in 5 minutes; past 30 overall, addresses that failed are limited too |
| Default port | 7347 |
