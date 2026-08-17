# Security

Tunsmith writes private keys to disk and can run privileged commands on a remote Linux host. Open source does not make that safe. Read this before `remote setup`.

## Trust model

You trust:

- This binary and its crate dependencies
- The machine that runs `tunsmith` (it holds the CA key)
- SSH authentication to the VPN server
- The remote host after you accept its host key

Tunsmith does not protect you from a compromised laptop, a stolen `pki/ca.key`, or a wrong host key you confirmed.

## Keys on disk

| Path | Contents |
| --- | --- |
| `pki/ca.key` | Root CA private key, unencrypted |
| `pki/server/server.key` | Server key, unencrypted |
| `pki/server/tls-crypt.key` | OpenVPN static key |
| `pki/clients/<name>/client.key` | Client key, unencrypted |
| `dist/clients/<name>.ovpn` | Unified profile including the client key |

On Unix, secret files are created with mode `600`. Backup `pki/` as you would any CA. There is no passphrase wrapping in 0.1.0.

`client remove` only drops the name from `tunsmith.json`. It does not revoke the certificate. CRL generation is not implemented. Treat a removed client as still valid until you rebuild PKI or add revocation.

## SSH

- Default user is `root`. Default auth is a private key (`~/.ssh/id_ed25519` or `id_rsa`). `--password` prompts; the password is not stored.
- Server host keys are checked against `~/.ssh/known_hosts`. Unknown hosts require confirmation. A mismatched key is refused.
- Hashed `known_hosts` entries are not matched; you may be prompted again.
- File upload uses SFTP, then `chmod 600` on the remote path.
- `remote setup` may run `apt-get install`, change `/etc/sysctl.conf`, write `/etc/openvpn/server/`, and enable `openvpn-server@<instance>`.
- `remote clean ssh` runs `systemctl stop/disable` and `rm -rf` on that instance directory only. Instance names are restricted to ASCII letters, digits, `-`, and `_`.

## OpenVPN

Configs use AES-256-GCM, SHA384, TLS-Crypt, and `dh none` (ECDHE). Compression is off. Full-tunnel mode (`redirect_gateway`) does **not** configure NAT; you must add masquerade yourself.

`preview ssh` may call `curl https://ifconfig.me` on the remote host to print a public IP.

## Disclosure

Once this project has a public GitHub repository, report vulnerabilities through a private GitHub security advisory. Do not file public issues with exploit details or copies of private keys. Until then, there is no public disclosure channel and no project email.

The short process is in [SECURITY.md](../SECURITY.md). This file is the threat model.
