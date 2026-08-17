# Remote deploy

SSH commands talk to one host at a time. Automatic OpenVPN **install** supports Debian and Ubuntu only (`PRETTY_NAME` in `/etc/os-release` must contain `Debian` or `Ubuntu`). Discovery and file upload are not OS-specific, but `apt-get` will refuse anything else.

You need SSH access (default user `root`) and, for upload, an SFTP subsystem (`sftp-server`). OpenVPN 2.4+ is required on the host; Tunsmith can install the `openvpn` package. Shared flags: [commands.md](commands.md).

## Authentication: key vs `--password`

Default is public-key auth.

1. `--key <path>` if given (`~` expanded).
2. Else `ssh_key_path` from `remotes/<host>.json` if that profile exists.
3. Else `~/.ssh/id_ed25519` when that file exists, else `~/.ssh/id_rsa` when that file exists, else Tunsmith still tries `~/.ssh/id_ed25519`.

Encrypted OpenSSH keys prompt for a passphrase. The passphrase is not stored.

`--password` prompts for an SSH password (dialoguer, hidden input). The password is not written to `remotes/`. If a profile already has `"ssh_auth_type": "password"` and you omit `--key`, Tunsmith prompts for a password again.

`remotes/<host>.json` may store `ssh_auth_type` and `ssh_key_path`. It never stores a password. User and port in that file are informational; the next command uses `-u` / `-p` (defaults `root` and `22`) unless you pass `user@host`.

## Host keys (`known_hosts`)

Server host keys are checked against `~/.ssh/known_hosts`.

- Known matching key: connect.
- Known but different key: refuse (possible MITM). Tunsmith does not offer to replace it.
- Unknown host: print a SHA-256 fingerprint and ask. Yes appends a line to `known_hosts`. No aborts.

Hashed `known_hosts` lines (`|1|...`) are skipped, so you may be prompted even if OpenSSH would have matched. `@revoked` / similar marked lines are skipped. Non-default SSH ports use the `[host]:port` form.

## SFTP upload

Files go over SFTP, then `chmod 600` on the remote path. Parent directories are created with `mkdir -p`. If the SFTP subsystem is missing, the command fails.

## `remote setup ssh`

After the same inspection as `preview ssh`:

**apt** — if `openvpn --version` / `dpkg -s openvpn` already shows a version, install is skipped. Otherwise, on Debian/Ubuntu only:

```text
apt-get update && apt-get install -y openvpn
```

Other distros error with “Unsupported OS … Install OpenVPN manually.” There is an RPM version probe for display; there is no `yum`/`dnf` installer.

**version gate** — after apt, Tunsmith parses live `openvpn --version` and compares it to `dist/build.json`. OpenVPN below 2.4, or not installed, fails. Missing stamp is treated as dialect `openvpn-2.4` with a warning. Configs are not re-rendered over SSH; run `preview ssh` then `build` if the dialect cannot run. `dist/build.json` is not uploaded.

**sysctl** — only when `redirect_gateway` is true in `tunsmith.json`. If `/proc/sys/net/ipv4/ip_forward` is not `1`:

```text
sysctl -w net.ipv4.ip_forward=1
```

Then existing `net.ipv4.ip_forward` lines are deleted from `/etc/sysctl.conf` and `net.ipv4.ip_forward=1` is appended. Split-tunnel projects skip this.

**NAT** — only when `redirect_gateway` is true, and only after Confirm (default no). Tunsmith parses `ip -4 route show default` (never guesses `eth0`):

- One `dev`: UPPERCASE warning with iface and subnet, then Confirm.
- Several (or none): `--nat-interface` or an interactive Select. Flag does not skip Confirm. No TTY without a flag: error (several/none) or skip NAT (one iface).
- Yes: idempotent iptables MASQUERADE + FORWARD tagged `tunsmith:<instance>`, scripts in `/etc/openvpn/server/<instance>/nat.sh`, oneshot `tunsmith-nat@<instance>.service`.
- No: setup continues without internet for full-tunnel clients.

If UFW is active, Tunsmith still applies iptables and warns that `DEFAULT_FORWARD_POLICY=DROP` may drop forwarded packets. It does not edit `/etc/default/ufw`. Cloud security groups stay yours.

**Files** — from `dist/server/` to `/etc/openvpn/server/<instance>/`:

| Local | Remote |
| --- | --- |
| `server.conf` | `<instance>.conf` (also copied to `/etc/openvpn/server/<instance>.conf`) |
| `ca.crt` | `ca.crt` |
| `server.crt` | `server.crt` |
| `server.key` | `server.key` |
| `tls-crypt.key` | `tls-crypt.key` |

`<instance>` is `instance_name` from `tunsmith.json` (restricted to ASCII alnum, `-`, `_` so the later `rm -rf` cannot walk `..`).

**systemd** — Debian/Ubuntu OpenVPN 2.4+ template unit:

```text
systemctl enable openvpn-server@<instance>
systemctl restart openvpn-server@<instance>
```

If `systemctl is-active` is not `active`, Tunsmith prints the last 20 lines of `journalctl -xeu openvpn-server@<instance>` and fails. On success it records `setup` in `remotes/<host>.json`.

`server.conf` drops privileges to `nobody`/`nogroup` after start. Status logs are `/var/log/openvpn/openvpn-status.log` and `/var/log/openvpn/ipp.txt`.

## `remote update ssh` / `remote clean ssh`

Update uploads only `server.conf` and restarts the unit. Certificates on the server are left as they were. The same live-version gate as `setup` runs first. If `redirect_gateway` is on, NAT Confirm runs again (idempotent apply).

Clean stops NAT (`tunsmith-nat@<instance>`) and the OpenVPN unit, deletes `/etc/openvpn/server/<instance>.conf` and `/etc/openvpn/server/<instance>/` (including `nat.sh`), and clears `setup` in the profile. It does not `apt-get remove openvpn`, does not revert sysctl, does not flush the whole NAT table, and does not delete `remotes/<host>.json`. Confirmation prompt defaults to no.

## `preview ssh`

Read-only aside from writing `remotes/<host>.json` and possibly appending `known_hosts`. Remote commands include `cat /etc/os-release`, `uname`, `hostname`, `free`, `df`, `uptime`, `hostname -I`, `curl https://ifconfig.me`, and OpenVPN version checks. Failures of those probes print `Unknown` (or `Not installed` for OpenVPN) instead of aborting.
