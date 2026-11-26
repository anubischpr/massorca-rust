# 0130: Readiness record of the service

Status: accepted, 2026-11-25.

## Context

The inventory's SRV-02 describes a readiness block printed when the service starts, with a JSON form named
`orca_server_ready` (schema version 1). This service prints a record of the same purpose, once its
listeners have settled (0045). This decision maps each documented field to what it prints.

## Decision

`service run` prints one record, and with `--json` one line `{"ok":true,"result":{...}}`. Field by field:

| Inventory field | Here | Status |
|---|---|---|
| schema `orca_server_ready`, version 1 | `schema`, `schema_version` | kept |
| runtime id | none. The service identity is `profile`: one service per profile (0003). On the network it is also `server_pin` (0041). `pid` and `started_at_ms` name this process | changed |
| bound endpoints | `endpoint` (local socket or pipe), `bound_endpoints` (network) | kept |
| advertised endpoints | `advertised_endpoints`: what an offer lists when it names no address (0041) | kept |
| managed-WSL reconciliation state | none. A distro runs its own service, reached as any host (0015, 0044) | changed |
| pairing object | `pairing`: `available`, `reason` when not, `pending_offers` as `{offer_id, expires_at_ms, scopes}`, never a secret | kept |
| pairing QR and pairing URL | not printed at startup. No offer exists yet, and a URL carries the secret. Offers are made by `pairing.create` (0041) | changed |
| optional health | `health.live_terminals`: the terminals alive in the service's own terminal host | kept |
| terminal-daemon health, when reported | no daemon: the terminal host runs in the service process, so only its live count is reported | changed |
| web client URL | none. No browser client exists; 0040 considers one only as a possibility that may never return | changed |
| build identity | `version`, `protocol` | kept |
| recipe mode (`--recipe-json`) | none. Environment recipes are an extension kind (0001) that this service does not build | changed |

Printing waits for every network listener to make its first bind attempt. A listener still waiting, for
example for a Tailscale address, is printed as `waiting` with its reason. The record is printed once per
process.

## Consequences

- A consumer that needs the inventory's schema reads `schema`, `schema_version`, `bound_endpoints`,
  `advertised_endpoints`, `pairing` and `health`, and treats the changed fields as absent.
- Adding a runtime id, a WSL state or a web URL later is an additive field under the same schema version.
- Pairing URLs and QR codes stay where 0041 puts them: created on request, shown once, never at startup.
