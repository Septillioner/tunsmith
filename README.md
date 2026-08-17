# Tunsmith

Strike an OpenVPN PKI. Deploy it over SSH.

OpenVPN still needs a CA, server and client certificates, a `server.conf`, and a way to get those files onto a Linux host. Tunsmith does that from one working directory: it generates the PKI in-process, writes OpenVPN configs, and can install over SSH. It is not WireGuard, not a hosted VPN, and not a general-purpose CA.

This is **0.1.0**. Treat it as alpha. Private keys are written unencrypted. Read [docs/security.md](docs/security.md) before using it on a real host.

## Install

Requires a Rust toolchain. For `remote` commands: a Debian or Ubuntu host with SSH, and OpenVPN 2.4+ on that host (Tunsmith can install OpenVPN there). No OpenSSL CLI. Keys are RSA + `rcgen`. VPN configs use `dh none`.

```bash
cargo install --path .
```

Or run from the repo:

```bash
cargo run -- --help
```

## Quick start

```bash
tunsmith init --template gateway-vpn
tunsmith config set --host vpn.example.com
tunsmith client add laptop
tunsmith build
tunsmith remote setup ssh root@203.0.113.10
```

`build` writes `dist/server/` and `dist/clients/*.ovpn`. Import the `.ovpn` on the client. If you enabled full-tunnel (`redirect_gateway`), you still need NAT on the server; Tunsmith prints an `iptables` example and does not apply it.

## Commands

| Command | What it does |
| --- | --- |
| `init` | Create `tunsmith.json` and `pki/` (RSA-4096 root CA) |
| `config set` / `config show` | VPN server settings |
| `client add\|remove\|list` | Client names. `remove` does not revoke certificates |
| `build` | Server conf + client profiles into `dist/` |
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
- `dist/` — generated OpenVPN files
- `remotes/` — SSH host profiles (no passwords)

Those paths are in `.gitignore`. Do not commit `pki/` or `dist/clients/*.ovpn`.

## Documentation

Index: [docs/README.md](docs/README.md).

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
