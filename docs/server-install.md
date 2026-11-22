# Installing a MassOrca server on Linux

This is the procedure for running the MassOrca service headless on a Linux
machine you reach over SSH, and pairing a desktop with it. The design is in
`docs/architecture/network.md` and decisions 0040 to 0047.

Nothing here needs root, opens a firewall port, or changes system packages.

## The short way: `host add-ssh`

From the desktop, with a Linux build of `massorca` at hand:

```sh
massorca host add-ssh user@server --name vps --binary ./massorca-linux-x86_64
```

This uses your own `ssh` (your `~/.ssh/config`, agent and `known_hosts`), and:

1. checks the remote system is Linux on x86_64 or aarch64;
2. uploads the binary to `~/.massorca/bin/massorca`;
3. writes `~/.config/systemd/user/massorca.service` and starts it, listening on
   the server's Tailscale address, port 7347 (`--listen` changes it);
4. asks the new service for a pairing offer through the SSH channel and pairs
   this desktop as an administrator.

Afterwards every command takes `--host vps`:

```sh
massorca --host vps workspace list
```

Running `host add-ssh` again upgrades the binary and restarts the service;
paired devices keep their access.

## The manual way

On the server:

```sh
mkdir -p ~/.massorca/bin ~/.massorca/home ~/.config/systemd/user
install -m 700 massorca ~/.massorca/bin/massorca
~/.massorca/bin/massorca service unit --listen tailscale > ~/.config/systemd/user/massorca.service
systemctl --user daemon-reload
systemctl --user enable --now massorca.service
```

The service must keep running after you log out. Check with
`loginctl show-user "$USER" -p Linger`; if it says `no`, run
`loginctl enable-linger "$USER"` (allowed for yourself by the default policy;
an administrator can do it otherwise).

Create a pairing offer on the server, as its owner:

```sh
MASSORCA_HOME=~/.massorca/home ~/.massorca/bin/massorca pair offer --admin
```

The offer is a credential valid for 10 minutes and usable once. Copy it to the
desktop through a channel you trust, and there:

```sh
massorca pair accept '<offer>' --name vps
```

## Where things are

| What | Where |
|---|---|
| Binary | `~/.massorca/bin/massorca` |
| Data (store, logs, server key) | `~/.massorca/home/profiles/default/` |
| Server key and certificate | `~/.massorca/home/profiles/default/network/` |
| Unit | `~/.config/systemd/user/massorca.service` |
| Log | `journalctl --user -u massorca` and `~/.massorca/home/profiles/default/logs/` |

## Listening

`--listen tailscale` (the default of `host add-ssh`) binds only to the
server's Tailscale IPv4 address. If the service starts before Tailscale, it
retries every 5 seconds. Other forms: `localhost:<port>`, `<ip>:<port>`. An
address on every interface (`0.0.0.0`) needs `--allow-all-interfaces`, and you
should not need it.

To keep the listener on loopback and reach it through Tailscale anyway, use TCP
pass-through: `tailscale serve --bg --tcp=7347 tcp://127.0.0.1:7347`. Do not use
HTTPS mode: it ends TLS at Tailscale and breaks device certificates.

## Devices

```sh
massorca --host vps device list
massorca --host vps device revoke <id-or-name>
```

A revoked device is disconnected at once and refused afterwards.

## Upgrading, stopping and removing

- Upgrade: replace `~/.massorca/bin/massorca`, then
  `systemctl --user restart massorca`.
- Stop: `systemctl --user stop massorca`.
- Remove: `massorca host remove vps --uninstall` from the desktop, or on the
  server `systemctl --user disable --now massorca`, delete the unit file,
  `systemctl --user daemon-reload`, and `rm -rf ~/.massorca`.
- Start over with a new server identity (every device pairs again): stop the
  service and delete `~/.massorca/home/profiles/default/network/`.
