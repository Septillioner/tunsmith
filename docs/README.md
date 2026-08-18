# Tunsmith documentation

Tunsmith is a 0.1.0 alpha CLI (AGPL-3.0-only). It generates a local OpenVPN PKI in the current working directory and can deploy it over SSH to Debian or Ubuntu. Install from [crates.io](https://crates.io/crates/tunsmith): `cargo install tunsmith`.

| Page | Contents |
| --- | --- |
| [Your own internet VPN on Ubuntu](ubuntu-internet-vpn.md) | Full-tunnel `gateway-vpn` on an Ubuntu VPS |
| [Getting started](getting-started.md) | Install, `init` / `config` / `client` / `build`, files on disk, NAT |
| [Commands](commands.md) | CLI reference: every command and flag, including `tui` |
| [PKI](pki.md) | RSA sizes, PKCS#8, `pki/` layout, revocation gap |
| [Remote](remote.md) | SSH auth, `known_hosts`, SFTP, apt, sysctl, systemd |
| [Security](security.md) | Threat model, key layout, SSH, OpenVPN |

Vulnerability reports: [SECURITY.md](../SECURITY.md). Versions: [CHANGELOG.md](../CHANGELOG.md).
