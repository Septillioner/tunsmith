# Tunsmith

Strike an OpenVPN PKI. Deploy it over SSH.

OpenVPN still needs a CA, server and client certificates, a `server.conf`, and a way to get those files onto a Linux host. Tunsmith does that from one working directory: it generates the PKI in-process, writes OpenVPN configs, and can install over SSH. It is not WireGuard, not a hosted VPN, and not a general-purpose CA.

This is **0.1.0**. Treat it as alpha. Private keys are written unencrypted. Read [docs/security.md](docs/security.md) before using it on a real host.

## Start here

Have an Ubuntu VPS and want your traffic to leave from that box? Follow [Your own internet VPN on Ubuntu](docs/ubuntu-internet-vpn.md).

## Install

Requires a Rust toolchain. For `remote` commands: a Debian or Ubuntu host with SSH. OpenVPN 2.4+ is the expected floor on that host (Tunsmith can install the distro package; it does not pin a version). See [Compatibility](#compatibility). No OpenSSL CLI. Keys are RSA + `rcgen`. VPN configs use `dh none`.

```bash
cargo install --path .
```

Or run from the repo:

```bash
cargo run -- --help
```

## Compatibility

Generated profiles target classic OpenVPN 2.x: `tls-crypt`, `dh none`, AES-256-GCM, TLS 1.2+. That is why **2.4+** is the expected floor on both server and client. **2.6** is the recommended line: it is what apt ships on current Debian/Ubuntu LTS, which is what `remote setup` installs.

Nothing in this table has been proven on a live tunnel yet. A row moves to tested only after we actually run it. Recommended is not a test result.

- **tested** (`:white_check_mark:`) — we ran this version
- **not tested** — expected from the config, not proven
- **unsupported** — these configs cannot load
- **n/a** — that product does not fill that role

| OpenVPN | Server | Client | Notes |
| --- | --- | --- | --- |
| 2.3 and older | unsupported | unsupported | no `tls-crypt`, no `dh none` |
| 2.4 | not tested | not tested | expected minimum |
| 2.5 | not tested | not tested | expected; `--cipher` is deprecated but still accepted |
| 2.6 (recommended) | not tested | not tested | typical Debian 12 / Ubuntu 24.04 apt |
| 2.7 | not tested | not tested | expected; current community line |
| Connect 3.x | n/a | not tested | client-only; `.ovpn` import untested |

## Quick start

```bash
tunsmith init --template gateway-vpn
tunsmith config set --host vpn.example.com
tunsmith client add laptop
tunsmith preview ssh root@203.0.113.10
tunsmith build
tunsmith remote setup ssh root@203.0.113.10
```

`build` writes `dist/server/`, `dist/clients/*.ovpn`, and `dist/build.json`. Default OpenVPN version comes from `preview ssh` (`remotes/<host>.json`); override with `build --openvpn-version X.Y`. Import the `.ovpn` on the client. Full-tunnel (`redirect_gateway`) asks to apply NAT on `remote setup` / `update` (Confirm, default no).

## Commands

| Command | What it does |
| --- | --- |
| `init` | Create `tunsmith.json` and `pki/` (RSA-4096 root CA) |
| `config set` / `config show` | VPN server settings |
| `client add\|remove\|list` | Client names. `remove` does not revoke certificates |
| `build` | Server conf + client profiles into `dist/` (default OpenVPN target from `preview`) |
| `template` | List `gateway-vpn`, `cloud-vpn`, `gateway-cloud-vpn` |
| `preview ssh` | SSH in, print host facts, save `remotes/<host>.json` |
| `remote setup ssh` | Install OpenVPN if needed, upload files, enable systemd |
| `remote update ssh` | Upload `server.conf` and restart |
| `remote clean ssh` | Stop the instance and delete its remote files |
| `remote clean local` | Delete `dist/` |
| `tui` | Interactive terminal menu (not a web UI) |

`init --country` is optional (two-letter code). There is no web GUI. There is no CRL.

## Project files

Created in the current working directory:

- `tunsmith.json` — project config
- `pki/` — CA, server, and client PEM (keys are unencrypted)
- `dist/` — generated OpenVPN files and `build.json` stamp
- `remotes/` — SSH host profiles (no passwords)

Those paths are in `.gitignore`. Do not commit `pki/` or `dist/clients/*.ovpn`.

## Documentation

Index: [docs/README.md](docs/README.md).

- [Your own internet VPN on Ubuntu](docs/ubuntu-internet-vpn.md) — full tunnel on your VPS (`gateway-vpn`)
- [Getting started](docs/getting-started.md) — install, init, first build, NAT
- [Commands](docs/commands.md) — CLI reference
- [PKI](docs/pki.md) — certificates, `pki/` layout, revocation gap
- [Remote](docs/remote.md) — SSH deploy to Debian/Ubuntu
- [Security](docs/security.md) — threat model, key layout, SSH, OpenVPN
- [SECURITY.md](SECURITY.md) — how to report a vulnerability
- [CHANGELOG.md](CHANGELOG.md) — versions

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

[AGPL-3.0-only](LICENSE).
