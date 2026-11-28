# 0133: Dialing a paired host through SSH

Status: accepted, 2026-11-26.

## Context

`host add-ssh` reaches a host behind a bastion or a ProxyCommand, because it
runs the system `ssh` with the user's `~/.ssh/config` and `--ssh-arg` values
(0044). Pairing and every later connection then dial the host's network
listener directly (0040, 0046). A host that only the bastion can reach is
installed, but never reached afterwards. 0044 left "reaching the service
through an SSH tunnel" for later; REM-05 needs it now.

## Decision

**A route per host.** A paired host may carry an SSH route: the ssh target,
extra ssh options (such as `-J bastion`), and an optional forward address.
Without a route the host is dialed directly, as before.

```sh
massorca host route vps --ssh deploy@vps.example.ts.net --ssh-arg -J --ssh-arg bastion
massorca host route vps --direct
massorca host add-ssh vps --binary ./massorca --tunnel   # pairs through ssh and keeps the route
```

The route is kept in `<profile>/hosts/<name>/route.json`, beside the host
file, as the install note is. `--direct` writes `null`. The host record does
not change, so older records, older builds and every `HostRecord` literal keep
working, and the phone's host store, which cannot run ssh, keeps the trait's
default: no route.

**The dial.** With a route, the client runs

```text
ssh -o BatchMode=yes -o ConnectTimeout=15 <ssh args>... -W <forward> -- <target>
```

(the system `ssh`, found as `host add-ssh` finds it, or the absolute path in
`MASSORCA_SSH`, as `MASSORCA_GIT` names git) through `massorca_process`, with
piped stdin, stdout and stderr and no TTY
(`-W` implies `-T`, no remote command and no other forwardings). `<forward>`
defaults to the host's first address: the server reported it, and its own
machine reaches it, whether it listens on the tailnet address or on
`localhost`. `--forward HOST:PORT` overrides it. Every value is one argument;
no shell reads any of them. The target and the forward are refused when ssh
could read them as an option or split them.

The existing TLS handshake with the pinned server key and the device
certificate then runs over the child's stdin and stdout, followed by the
WebSocket upgrade and the protocol handshake. **SSH only carries bytes.** It is
not trusted for identity: a different server key behind the route is refused
with `host.identity_mismatch`, exactly as on a direct connection.

The route lives at the one dial seam, `Client::connect_host`, so `--host`,
`host status`, `host status --watch`, the desktop's links and their backoff
(REM-07) all use it without change.

**Failures.** ssh missing, a refused login (batch mode never prompts) and a
jump host or forward that fails are `host.unreachable` (exit 4). The message
names the route as typed and ends with what ssh printed last, for example:

```text
the host is unreachable: through the SSH route `ssh -J bastion deploy@vps`: Permission denied (publickey).
```

Messages carry no key and no password the client holds; the route is echoed as
the user typed it, so an ssh option written with a secret in it shows up.
What ssh printed is quoted as printable text only: control characters, such as
the ANSI sequences a banner or a ProxyCommand may send, are dropped. A relative
`MASSORCA_SSH` is refused rather than ignored. `-f`, alone or in a cluster, and
`-o ForkAfterAuthentication` are refused in a route, since a backgrounded ssh
would outlive its connection. An unreachable route is retried with
the network backoff like any unreachable host; a key mismatch is final.

**The child's life.** The ssh process belongs to the connection: dropping the
connection kills and reaps it. On Windows it starts suspended inside a
kill-on-close job (0062's `spawn_in_job`), so a `-J` or ProxyCommand child
ends with it. On Unix the child is killed; what it started for `-J` sees its
pipes close and exits, as OpenSSH's own does.

## Alternatives

- **The route as a field of `HostRecord`.** One read instead of two, but a new
  field breaks every struct literal across the client, the desktop, the phone
  bindings and the tests, and older builds rewriting the record would drop it.
- **A local port forward (`ssh -L`).** Needs a free local port, leaves a
  listener any local user can reach, and a race between the forward coming up
  and the dial. The stdio forward has neither.
- **An in-process SSH library (`russh`).** Still later, as 0044 says; the
  system binary keeps the user's config, agent and host-key policy.
- **Always forwarding to `127.0.0.1:<port>`.** The default listener binds the
  tailnet address (0045), which loopback does not reach; the first address
  does, and `--forward` covers the rest.

## Consequences

- A host behind a bastion is reached after pairing; REM-05 is done.
- Each connection to a routed host costs one ssh process for its lifetime.
- The phone cannot use a route; a routed host is reached there only when it
  also has a direct path.
- The tests name a fake `ssh -W` through `MASSORCA_SSH` instead of copying it
  to `ssh.exe` on `PATH`: real-time antivirus held a fresh `ssh.exe` in a
  temporary folder suspended, and the client then waited on it.
