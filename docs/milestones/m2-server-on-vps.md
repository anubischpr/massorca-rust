# M2: the MassOrca server on the VPS, driven from Windows over Tailscale

_Addresses, host names and user names in this transcript are placeholders._

Run 2026-11-21 by the Phase 3 session. Code: branch `phase3/cli` (the tip of the
Phase 3 chain, which sits on the Phase 2 terminal chain). Design: decisions 0040
to 0047 and `docs/architecture/network.md`.

Sections 1 to 4 are the first run. Section 5 is the redeploy from `main` after
the security fixes, run 2026-11-22, and "Rerun it" lists the commands to repeat it.

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

At the time of this run, an offer without `--admin` gave `read` and `workspace`. Since 2026-11-22 offers give full
access by default, and the reduced device needs `--standard` (decision 0041).

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

## 5. Redeploy from main after the security fixes (2026-11-22)

Sections 1 to 4 ran the build from before the security review. On 2026-11-22
the server was redeployed from `main` at `612dd983` (the merge of the security
fixes) and every check below was run again against it. Only the redeployed
build is described in this section.

### Build

| Role | Build |
|---|---|
| Client | `cargo build --release --locked -p massorca-cli` on Windows 11 |
| Server | `cargo zigbuild --release --locked -p massorca-cli --target x86_64-unknown-linux-gnu.2.31` on Windows 11 (cargo-zigbuild 0.24.1, zig 0.17.1 from `uv tool install ziglang`); the binary needs glibc 2.30 at most |

WSL was not used: it was unavailable on this machine during the run.

### Redeploy

The store's migrations were renumbered when the agent sessions table landed
before the devices table on `main`, so the database written by the old build
cannot be opened by the new one. The service was stopped, the old database and
binary were moved to `~/.massorca/backup-20261122/`, and the server key in
`~/.massorca/home/profiles/default/network` was kept. Paired devices were
therefore paired again, from a new throwaway client home.

```
$ systemctl --user stop massorca                       (on the VPS)
$ mv ~/.massorca/home/profiles/default/store.sqlite3* ~/.massorca/backup-20261122/
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
device: win-desktop (01a203ac-bec9-7265-8725-318b3243a2cd)
scopes: read, workspace, host-admin, device-admin
paired: 2026-11-22 14:33 UTC
[exit 0]
```

The pin is the same as on 2026-11-21: the server key survived the redeploy.

### Results

| Check | Result |
|---|---|
| Connect, `device whoami`, `server status` | passed: connected, listening on `100.64.0.10:7347` only |
| Folder workspace and a terminal | passed (second attempt, see below) |
| Git through the remote: worktree, status, stage, commit, log, branches | passed |
| Pair a second device; replay the same offer | passed: paired; replay refused with `pairing.invalid`, exit 4 |
| Second device without `device-admin` | passed: `permission.denied` |
| Revoke while the device is connected and connecting | passed, with one finding (see below) |
| Restart the server, reconnect | passed: data and revocation kept |
| Tailscale path | passed: `host discover` finds the server listening |

**Folder workspace and terminal.** The first `workspace create demo w1 --path
.../demo/w1` failed with `path.not_found`: `main` now registers a folder
workspace only when the folder exists, and the folder had not been made. With
the folder made first:

```
$ massorca --host vps project add /home/deploy/.massorca/demo --folder --name demo
added project demo (folder) at /home/deploy/.massorca/demo
$ massorca --host vps workspace create demo w1 --path /home/deploy/.massorca/demo/w1
created workspace w1 (folder) at /home/deploy/.massorca/demo/w1
$ massorca --host vps terminal create w1 --shell /bin/bash --title m2
$ massorca --host vps terminal send <id> 'echo M2-from-windows $(uname -sr)'
$ massorca --host vps terminal screen <id>
[deploy@vps w1]$ echo M2-from-windows $(uname -sr)
M2-from-windows Linux 7.2.9-arch1-1
$ massorca --host vps terminal close <id>
```

**Git through the remote.** A repository with one commit was made on the VPS
over SSH (`~/.massorca/demo-git`). Everything else ran from Windows:

```
$ massorca --host vps project add /home/deploy/.massorca/demo-git --git --name demo-git \
    --workspaces-root /home/deploy/.massorca/workspaces
$ massorca --host vps workspace create demo-git w2 --branch m2-check
created workspace w2 (worktree) at /home/deploy/.massorca/workspaces/w2
$ massorca --host vps terminal send <id> 'echo from-m2 > m2.txt'
$ massorca --host vps git status -w w2
on m2-check
?? m2.txt
$ massorca --host vps git stage --all -w w2
staged
$ massorca --host vps git commit -m 'm2: a commit made from Windows' -w w2
[m2-check 2e1581e1b411] m2: a commit made from Windows
$ massorca --host vps git log -w w2
2e1581e1b411 m2 m2: a commit made from Windows
49c33c84b5cc m2 init
$ massorca --host vps git branches -w w2
* m2-check 2e1581e1b411
  main 49c33c84b5cc (checked out elsewhere)
```

Without `--workspaces-root`, the first attempt put the worktree under the
default root `~/massorca/workspaces`, outside the install folder. That
attempt was removed (`workspace rm w2 --force`, `project rm demo-git`, the
directory deleted) before the run above.

**Revocation, including the race fixed by the security review.** The second
device (`read`, `workspace`) held a live `git watch -w w2` stream, and a loop
ran `device whoami` as the second device 60 times in a row, one connection
after another. `win-desktop` revoked the device while both were running:

```
$ massorca --host vps device revoke second-device
revoked second-device (01a203af-a781-7204-b582-4b5ece1c50f8)
git watch ended on its own when the revoke returned
loop: 60 calls, 2 succeeded, 58 refused; started after the revoke returned: 57, of which succeeded: 0
$ massorca --host vps workspace list                  (as second-device)
error: this device's access was revoked; pair it again to reconnect (auth.device_revoked)
[exit 4]
```

No connection that started after the revoke returned was admitted, and the open
stream was closed. The exact interleaving of the race (a revoke between reading
the device row and registering the connection) cannot be forced from outside;
it is covered by the server test
`a_revoke_between_the_row_read_and_registration_refuses_the_device`.

Finding: the revoked `git watch` printed `watching` and then exited **0** with
no message. A stream the server closes because the device was revoked should
end with `auth.device_revoked` and a non-zero exit. Not fixed here.

**Restart.**

```
$ systemctl --user restart massorca                   (on the VPS)
$ massorca host status vps
vps: connected, server 0.1.0, scopes read, workspace, host-admin, device-admin
$ massorca --host vps device whoami
device: win-desktop (01a203ac-bec9-7265-8725-318b3243a2cd)
$ massorca --host vps workspace list
w2  worktree  m2-check  /home/deploy/massorca/workspaces/demo-git/w2  ready
$ massorca --host vps git log -w w2
3d66a07aca82 m2 m2: a commit made from Windows
1863b8b71f84 m2 init
$ massorca --host vps device whoami                   (as second-device)
error: this device's access was revoked; pair it again to reconnect (auth.device_revoked)
[exit 4]
```

The restart ran during the first attempt, so it shows that attempt's `w2` and
commit; that attempt was removed afterwards, as described above. Terminals end
with the service, as documented.

**Tailscale path.**

```
$ massorca host discover
NAME        DNS NAME                    IP           OS     MASSORCA   HOST
vps         vps.example.ts.net          100.64.0.10  linux  listening  vps
other-node  other-node.example.ts.net   100.64.0.11  linux  no         -
$ ss -ltn | grep -E ':(7347|6768|6775) '              (on the VPS)
LISTEN 0  128  100.64.0.10:7347  0.0.0.0:*
LISTEN 0  511      0.0.0.0:6768  0.0.0.0:*
LISTEN 0  511      0.0.0.0:6775  0.0.0.0:*
```

Port 7347 listens on the Tailscale address only. Ports 6768 and 6775 belong to
the production install, which was not touched; no firewall, `sshd` or package
change was made.

### State left on the VPS

| What | Value |
|---|---|
| Binary | `~/.massorca/bin/massorca`, built from `612dd983` |
| Backup of the old build | `~/.massorca/backup-20261122/` (old binary and database) |
| Projects | `demo` (folder) and `demo-git` (git), with workspaces `w1` and `w2` |
| Devices | `win-desktop` (active), `second-device` (revoked) |

## Rerun it

What a second person needs: SSH to the VPS as `deploy` over the tailnet, this
repository at `612dd983` or later, Rust with the `x86_64-unknown-linux-gnu`
target, and `jq`. Run from Git Bash on Windows with `MSYS_NO_PATHCONV=1` (or
from any Linux or macOS shell). Use a new client home so nothing of yours is
touched, and new names (`v1`, `v2`) so the state above stays.

```sh
export MSYS_NO_PATHCONV=1 MASSORCA_HOME=$PWD/validate-home
cargo build --release --locked -p massorca-cli
M=target/release/massorca            # massorca.exe on Windows
SSH="ssh deploy@100.64.0.10"
R=/home/deploy/.massorca

# Pair: the offer travels from the server to the client through a pipe and is never shown.
$SSH "MASSORCA_HOME=$R/home $R/bin/massorca --json pair offer --admin --ttl 120" \
  | jq -r .result.offer | $M pair accept - --name vps --device-name validator
$M host status vps
$M --host vps server status

# Folder workspace and terminal.
$SSH "mkdir -p $R/validate/v1"
$M --host vps project add $R/validate --folder --name validate
$M --host vps workspace create validate v1 --path $R/validate/v1
T=$($M --host vps --json terminal create v1 --shell /bin/bash | jq -r .result.id)
$M --host vps terminal send "$T" 'echo hello-from-validator $(uname -sr)'; sleep 2
$M --host vps terminal screen "$T"
$M --host vps terminal close "$T"

# Git through the remote, in the existing demo-git project.
$M --host vps workspace create demo-git v2 --branch validate
T=$($M --host vps --json terminal create v2 --shell /bin/bash | jq -r .result.id)
$M --host vps terminal send "$T" 'echo v > v.txt'; sleep 2; $M --host vps terminal close "$T"
$M --host vps git status -w v2
$M --host vps git stage --all -w v2
$M --host vps git commit -m 'validate: a commit from the client' -w v2
$M --host vps git log -w v2

# Second device, replay, revocation while connected and connecting.
B=$PWD/validate-second; C=$PWD/validate-replay; mkdir -p "$B" "$C"
# A reduced device (`--standard`), so the permission check below has something to refuse; offers are full access by default.
O=$($M --host vps --json pair offer --standard --ttl 120)
jq -r .result.offer <<<"$O" | MASSORCA_HOME=$B $M pair accept - --name vps --device-name validator-second
jq -r .result.offer <<<"$O" | MASSORCA_HOME=$C $M pair accept - --name vps --device-name replay   # expect pairing.invalid, exit 4
unset O
MASSORCA_HOME=$B $M --host vps device list                                                        # expect permission.denied
(MASSORCA_HOME=$B $M --host vps git watch -w v2; echo "watch exit $?") &
(for i in $(seq 1 60); do s=$(date -u +%s.%N); MASSORCA_HOME=$B $M --host vps device whoami >/dev/null 2>&1; echo "$s $?"; done > loop.log) &
sleep 4; $M --host vps device revoke validator-second; REVOKED=$(date -u +%s.%N); wait
awk -v r="$REVOKED" '$1 > r && $2 == 0 { bad++ } END { print "admitted after the revoke:", bad+0 }' loop.log   # expect 0
MASSORCA_HOME=$B $M --host vps workspace list                                                     # expect auth.device_revoked, exit 4

# Restart and reconnect.
$SSH 'systemctl --user restart massorca'; sleep 3
$M host status vps
$M --host vps git log -w v2
MASSORCA_HOME=$B $M --host vps device whoami                                                      # expect auth.device_revoked

# Tailscale path.
$M host discover
$SSH "ss -ltn | grep 7347"                                                                        # expect the Tailscale address only

# Clean up what this added.
$M --host vps workspace rm v2 --force
$M --host vps workspace rm v1
$M --host vps project rm validate
$SSH "rm -rf $R/validate"
$M host remove vps          # revokes the validator device on the server; the branch `validate` stays in demo-git
```

## Not checked

- `host discover` against the real tailnet with a second MassOrca server (there
  is only one).
- Connecting by the Tailscale DNS name alone: the host stores the IP and the
  name, and only the stored host was used; the name was not tried on its own.
- A reboot of the VPS (the unit is enabled and lingering is on, so it should
  start; not tried, to leave the production machine alone).
- Linux client: only the Windows client was run against the server. WSL was
  unavailable, so Linux coverage is CI plus the VPS server itself.
- Android (M5) and the desktop UI (M4) are later milestones.

## Sign-off

M2 was signed off on 2026-11-22 with open items, after an independent
validation. The validator used a client built from main `ca5685db`, against the
deploy built from `612dd983`. It started from a fresh client home and used the
client protocol only, with no SSH access to the VPS. The validator's device was
paired from a single-use standard offer, with scopes read and workspace.

| Check | Result |
|---|---|
| Pair with a single-use standard offer (19:27 UTC) | pass |
| Remote terminal: create, send, read (`m2-validate-ok`, `Linux`), close | pass |
| Worktree workspace in the `demo-git` project: `git status` before and after a change, then removal with its branch | pass |
| About 15 separate commands, each reconnecting without pairing again | pass |
| `git watch` live, reporting a worktree change | pass |
| A device-admin call refused with `permission.denied`, exit 1 | pass |
| After the revoke at 19:36:22 UTC: `server status`, `terminal list` and a new `git watch` each refused at once with `auth.device_revoked`, exit 4 | pass |

The validator's client home, with its device key, was deleted afterwards.

Open items, not verified independently:

- The server side: the service unit, the listeners, and surviving a restart.
- Reconnecting after a server restart.
- A stream already open when its device is revoked. This is fixed in #162,
  which is not deployed yet.
- The revoked devices `second-device` and `m2-validate` still appear in the
  server's device list.

To close them: once #162, #165 and #166 are on main, redeploy once from main.
Then rerun only two checks: a stream open when its device is revoked, and
reconnecting after a server restart.

## 6. Open items closed (2026-11-22)

The VPS was redeployed once more, from main `672f2653`, which contains #162,
#165 and #166. The two checks left open were rerun from the Windows client,
built from the same commit.

| Check | Result |
|---|---|
| A stream open when its device is revoked | passed: `git watch` printed `error: this device's access was revoked; pair it again to reconnect (auth.device_revoked)` and exited 1, within 3 s of the revoke. The device is refused afterwards with exit 4 |
| Reconnecting after a server restart | passed: `git watch --json` printed `unverifiable` (attempt 1, retry in 236 ms), then `live`, then `ready` again, then the change made after the restart, and exited 0 |

### Redeploy

The store only gained new migrations, so the existing database was kept and
upgrades in place. The service was stopped, and the database and the old
binary were copied to `~/.massorca/backup-<UTC time>/` before the new binary
went in. The device list and the server pin did not change. The service was
down for about a minute, because the first binary copy failed and was redone.
It then listened on `100.64.0.10:7347` only. The production ports were not
touched.

### The revoke check

1. Pair a throwaway client home as device `m2-live-revoke` with a standard
   offer.
2. Start `massorca --host vps git watch -w w2` there.
3. Revoke the device from the admin device while the watch runs.

```text
$ massorca --host vps git watch -w w2          (as m2-live-revoke)
watching
error: this device's access was revoked; pair it again to reconnect (auth.device_revoked)
[exit 1]
$ massorca --host vps device whoami            (as m2-live-revoke)
error: this device's access was revoked; pair it again to reconnect (auth.device_revoked)
[exit 4]
```

A Windows client home has to be private before it can pair. A folder made
with `mkdir` is not, so the throwaway home was made private with
`massorca profile secure --yes` first.

### The restart check

```text
$ massorca --host vps --json git watch -w w2 --max-events 1
{"ok":true,"result":{"ready":true}}
$ systemctl --user restart massorca            (on the VPS)
{"ok":true,"result":{"attempt":1,"link":"unverifiable","reason":"…","retry_in_ms":236}}
{"ok":true,"result":{"link":"live"}}
{"ok":true,"result":{"ready":true}}
(a file is written in the w2 worktree on the VPS)
{"ok":true,"result":{"kinds":["worktree"]}}
[exit 0]
```

The file written for the check was removed afterwards.

### State left on the VPS

The projects and workspaces are as before. In the device list,
`win-desktop` is active, and `second-device`, `m2-validate` and
`m2-live-revoke` are revoked. Revoked devices stay listed: the product has no
way to delete a device.
