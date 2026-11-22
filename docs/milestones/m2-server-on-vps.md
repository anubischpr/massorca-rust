# M2: the MassOrca server on the VPS, driven from Windows over Tailscale

_Addresses, host names and user names in this transcript are placeholders._

Run 2026-11-21 by the Phase 3 session. Code: branch `phase3/cli` (the tip of the
Phase 3 chain, which sits on the Phase 2 terminal chain). Design: decisions 0040
to 0047 and `docs/architecture/network.md`.

## Environments

| Role | Machine | Build |
|---|---|---|
| Client | this Windows 11 machine, Git Bash | `cargo build --release -p massorca-cli` on Windows |
| Server | the VPS: Arch Linux x86_64, glibc 2.44, Tailscale 1.104.1, user `deploy` (no root used) | `cargo build --release -p massorca-cli` in WSL Ubuntu 24.04 (needs glibc 2.39) |

The client used a separate, throwaway `MASSORCA_HOME` so the owner's real
profile was not touched. SSH to the Tailscale address used a separate
`known_hosts` file whose ed25519 key was checked against the key read over the
already trusted public address. The production
install (ports 6768 and 6775, `orca-ide`, `~/orca-preview`, `~/orca-prod`) was
not touched; port 7347 was free before the install.

## 1. Install and pair over SSH

```
$ massorca host add-ssh deploy@100.64.0.10 --name vps \
    --binary massorca-linux-x86_64 --device-name win-desktop \
    --ssh-arg=-i --ssh-arg=<key> --ssh-arg=-o --ssh-arg=UserKnownHostsFile=<checked file>
Checking the remote system
Uploading the binary
Writing the systemd user unit
Waiting for the service
Creating a pairing offer
installed massorca on deploy@100.64.0.10 (Linux x86_64)
binary: ~/.massorca/bin/massorca
unit: ~/.config/systemd/user/massorca.service
service version: unknown
host: vps
server: vps
address: 100.64.0.10:7347
address: vps.example.ts.net:7347
pin: sha256:<server-pin>
device: win-desktop (01a1fedd-5932-73ee-b28d-42b3b13b99d1)
scopes: read, workspace, host-admin, device-admin
paired: 2026-11-21 16:08 UTC
[exit 0]
```

The first attempt installed and started the service but stopped at "Creating a
pairing offer" with `host.bootstrap_failed`: the bootstrap read the offer from
the root of the JSON reply, while `massorca --json` wraps it in
`{"ok":true,"result":...}`. Fixed in `fix(core-remote): read the offer and
version from the CLI's JSON envelope`, with the fake shell now answering in the
real envelope. The second run above re-uploaded the binary, restarted the unit
and paired; the server key and pin survived the reinstall. The offer itself was
never printed: it travelled from the server's local socket through the SSH
channel into the client process.

"service version: unknown" is cosmetic: the bootstrap looks for `version` where
`service status --json` does not put it. Not fixed in this phase.

## 2. Drive the server from Windows

```
$ massorca host status vps
vps: connected, server 0.1.0, scopes read, workspace, host-admin, device-admin
$ massorca --host vps server status
REQUESTED       BOUND               STATE
tailscale:7347  100.64.0.10:7347  listening
server pin: sha256:<server-pin>
active devices: 1
pending offers: 0
$ massorca --host vps project add /home/deploy/.massorca/demo --folder --name demo
added project demo (folder) at /home/deploy/.massorca/demo
$ massorca --host vps workspace create demo w1 --path /home/deploy/.massorca/demo/w1
created workspace w1 (folder) at /home/deploy/.massorca/demo/w1
$ massorca --host vps workspace list
ID                                    NAME  KIND    BRANCH  PATH                               STATE
01a1fee5-8a89-7581-b095-53a10396c958  w1    folder  -       /home/deploy/.massorca/demo/w1  ready
$ massorca --host vps terminal create w1 --shell /bin/bash --title m2
id: 01a1fee5-94be-7191-9abe-d3930ddb590a
state: live
shell: /bin/bash
cwd: /home/deploy/.massorca/demo/w1
$ massorca --host vps terminal send 01a1fee5-... 'echo M2-from-windows $(hostname) $(uname -sr)'
$ massorca --host vps terminal screen 01a1fee5-...
[deploy@vps w1]$ echo M2-from-windows $(hostname) $(uname -sr)
bash: command not found: hostname
M2-from-windows Linux 7.2.9-arch1-1
[deploy@vps w1]$
$ massorca --host vps terminal close 01a1fee5-...
closed terminal 01a1fee5-94be-7191-9abe-d3930ddb590a
```

Each line is a separate process, so the terminal outlived the client between
`send` and `screen`.

The first `project add` failed: the CLI made `/home/deploy/...` absolute
against the Windows working directory before sending it. Fixed in
`fix(cli): send paths to a remote host as typed`, with a test. (Git Bash also
rewrites POSIX-looking arguments for Windows programs; the run used
`MSYS_NO_PATHCONV=1`. PowerShell and cmd do not do that.)

## 3. A second device, a replay and a revocation

```
$ massorca --host vps --json pair offer --ttl 120        (offer kept in a variable, never shown)
$ massorca pair accept - --name vps --device-name second-device     (second profile, offer on stdin)
device: second-device (01a1fee6-45b9-733d-b931-188f078b69d0)
scopes: read, workspace
[exit 0]
$ massorca pair accept - --name vps --device-name replay            (third profile, same offer)
error: the pairing offer is not valid here; it may have been used already (pairing.invalid)
[exit 4]
$ massorca --host vps device list                                   (as second-device)
error: device 01a1fee6-... may not use device-admin (permission.denied)
[exit 1]
$ massorca --host vps device list                                   (as win-desktop)
*  01a1fedd-...  win-desktop    read,workspace,host-admin,device-admin  ...  active
   01a1fee6-...  second-device  read,workspace                          ...  active
$ massorca --host vps device revoke second-device
revoked second-device (01a1fee6-45b9-733d-b931-188f078b69d0)
$ massorca --host vps workspace list                                (as second-device)
error: this device's access was revoked; pair it again to reconnect (auth.device_revoked)
[exit 4]
```

## 4. What runs on the VPS

```
$ ss -ltn | grep 7347
LISTEN 0      128      100.64.0.10:7347       0.0.0.0:*
$ systemctl --user status massorca
● massorca.service - MassOrca service
     Loaded: loaded (/home/deploy/.config/systemd/user/massorca.service; enabled)
     Active: active (running)
$ ls -la ~/.massorca/home/profiles/default/network
-rw-r--r--  identity.crt
-rw-------  identity.key
```

| What | Value |
|---|---|
| Binary | `/home/deploy/.massorca/bin/massorca` |
| Data | `/home/deploy/.massorca/home` (profile `default`) |
| Demo folder | `/home/deploy/.massorca/demo` |
| Unit | `/home/deploy/.config/systemd/user/massorca.service`, enabled, user lingering already on |
| Listener | `100.64.0.10:7347`, the Tailscale address only; no firewall change |
| Logs | `journalctl --user -u massorca` |
| Stop | `systemctl --user stop massorca` |
| Remove | `systemctl --user disable --now massorca && rm ~/.config/systemd/user/massorca.service && systemctl --user daemon-reload && rm -rf ~/.massorca`, or `massorca host remove vps --uninstall --ssh deploy@100.64.0.10` from the client |

Paired devices left on the server: `win-desktop` (active, its key is in the
throwaway client home on this machine) and `second-device` (revoked).

## After the security review

The independent security review of the Phase 3 chain found a revocation race
(a device revoked while it was connecting could keep its connection) and
asked for hardening of pairing limits, unauthenticated connections, `PATH`
lookup, Windows key files and logging; the fixes are on
`phase3/security-fixes`. The server on the VPS still runs the build from
before those fixes. It listens on the tailnet address only, so it stays up
until the fixes are merged. Then the new binary replaces
`~/.massorca/bin/massorca` and the unit restarts; the server key and the paired
devices live in `~/.massorca/home` and are kept. This section will record that
redeploy.

## Not checked

- `host discover` against the real tailnet with a second MassOrca server (there
  is only one).
- A reboot of the VPS (the unit is enabled and lingering is on, so it should
  start; not tried, to leave the production machine alone).
- Android (M5) and the desktop UI (M4) are later milestones.
