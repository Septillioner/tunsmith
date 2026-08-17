# Commands

Binary: `tunsmith`. No subcommand prints help. `--version` / `-V` prints 0.1.0.

There is no web GUI, no `config get`, no `config reset`, and no CRL command. `tunsmith tui` is a terminal menu over the same local commands.

## `init`

Create `tunsmith.json` and a root CA under `pki/`. If `pki/ca.crt` and `pki/ca.key` already exist, prints a message and exits unless `--force`.

```text
tunsmith init [OPTIONS]
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `-n`, `--name <NAME>` | current folder name | Instance name (ASCII alnum, `-`, `_`). Used as CA CN suffix, remote directory, and systemd instance. |
| `-o`, `--org <ORG>` | the instance name | Organization in the CA subject |
| `-c`, `--country <CODE>` | omitted | Two ASCII letters; stored uppercase on the CA. Invalid codes fail. |
| `-v`, `--validity <YEARS>` | `10` | CA notAfter = now + years × 365 days |
| `-t`, `--template <NAME>` | none | `gateway-vpn`, `cloud-vpn`, or `gateway-cloud-vpn` |
| `-s`, `--schema <PATH>` | none | JSON file merged into server settings (see below) |
| `--force` | off | Delete `pki/` and write a new CA and `tunsmith.json` |

`--force` does not delete `dist/` or `remotes/`. The new `tunsmith.json` has an empty client list.

`--schema` must be a JSON object. A top-level `"template"` string is applied first (unknown names are stored but only known templates change settings). Then either the `"server"` object or the root object is merged. Accepted keys: `host`, `port`, `protocol` / `proto`, `subnet`, `device` / `dev`, `redirect_gateway` / `redirectGateway`, `cipher`, `auth`, `keepalive`, `verb`, `dns` (array of strings), `client_to_client` / `clientToClient`, `block_outside_dns` / `blockOutsideDNS`. Schema values override the template.

## `config set`

Requires `tunsmith.json`. Writes the file and prints the `server` object. Flags not passed are left unchanged. Boolean flags only turn features **on** (except `--allow-dns`).

```text
tunsmith config set [OPTIONS]
```

| Flag | Sets |
| --- | --- |
| `--host <HOST>` | Client `remote` address and server cert SAN (on next server cert issue) |
| `--port <PORT>` | VPN listen port (1–65535) |
| `--proto <PROTO>` | Stored as-is (configs use this string; typical values `udp` / `tcp`) |
| `--subnet <CIDR>` | VPN pool, e.g. `10.8.0.0/24` |
| `--dev <DEV>` | OpenVPN device, e.g. `tun` |
| `--redirect` | `redirect_gateway = true` |
| `--cipher <CIPHER>` | e.g. `AES-256-GCM` |
| `--auth <DIGEST>` | e.g. `SHA384` |
| `--keepalive <STR>` | Two integers, e.g. `10 120` |
| `--verb <N>` | OpenVPN verbosity (`u8`) |
| `--dns <LIST>` | IPv4 addresses separated by comma, space, or `;`. Non-IPv4 tokens are skipped with a warning. |
| `--c2c` | `client_to_client = true` |
| `--block-dns` | `block_outside_dns = true` (writes `setenv opt block-outside-dns` in client `.ovpn`) |
| `--allow-dns` | `block_outside_dns = false` |

There is no flag to turn `redirect_gateway` or `client_to_client` off. Edit `tunsmith.json` or re-`init`.

Defaults when `init` ran without a template: host empty, port `1194`, proto `udp`, subnet `10.8.0.0/24`, device `tun`, redirect off, cipher `AES-256-GCM`, auth `SHA384`, keepalive `10 120`, verb `3`.

## `config show`

Prints instance metadata, server settings, and the client name list. Empty host is shown as `Not set`. DNS `null` is shown as `Default` (build then uses 8.8.8.8 and 8.8.4.4).

## `client add <NAME>`

Appends `{ "name": "<NAME>" }` to `tunsmith.json`. Duplicate names are ignored. Does not write certificates; run `build`.

## `client remove <NAME>`

Drops the name from `tunsmith.json`. Does **not** delete `pki/clients/<name>/`, does **not** revoke, and does **not** rebuild `dist/`. See [pki.md](pki.md).

## `client list`

Prints the names in `tunsmith.json`.

## `build`

Requires an initialized PKI and a non-empty VPN `--host` in `tunsmith.json` (`config set --host`). Deletes `dist/` if present, then writes:

- `dist/build.json` — compile stamp (OpenVPN target, dialect `openvpn-2.4`, version source)
- `dist/server/server.conf`
- copies of `ca.crt`, `server.crt`, `server.key`, `tls-crypt.key` into `dist/server/`
- `dist/clients/<name>.ovpn` for each listed client

```text
tunsmith build
tunsmith build --host 203.0.113.10
tunsmith build --openvpn-version 2.6
tunsmith build --host 203.0.113.10 --openvpn-version 2.4.12
```

| Flag | Meaning |
| --- | --- |
| `--host <HOST>` | Use `remotes/<HOST>.json` from `preview ssh`. Required when several remotes exist. |
| `--openvpn-version <X.Y\|X.Y.Z>` | Compile for this OpenVPN version. Overrides the preview. Do not use `-V` (that is the Tunsmith binary). Below 2.4 fails. |

Default version is the preview server (`remotes/*.json`). One remote is selected automatically. No remotes: baseline OpenVPN 2.4 on Linux. This slice still emits 2.4 syntax (`cipher`, not `data-ciphers`) for every allowed target.

`remote setup` / `remote update` upload `dist/` unchanged. After OpenVPN is installed they parse the live version and refuse if it cannot run the stamp dialect (below 2.4, or missing OpenVPN). Missing `dist/build.json` is treated as `openvpn-2.4` with a warning.

Issues `pki/server/` material and each missing client cert on first need. Reuses existing PEM files. Paths inside `server.conf` are relative to `/etc/openvpn/server/` (`<instance>/ca.crt`, and so on).

## `template`

Lists the three templates with subnet, redirect, client-to-client, and DNS when the template sets DNS.

## `tui`

Interactive `dialoguer` menu. Not a web UI. No remote/SSH actions.

```text
tunsmith tui
```

Loop: show status (`config show`), initialize / re-initialize, configure server (host, port, subnet, DNS), manage clients (list via `config show`, add), build, exit. Configure / clients / build are listed as unavailable until `pki/` exists.

Init asks for name, template (including none), and confirmation. If a CA already exists, a second prompt asks whether to overwrite; `--force` is used only after that yes. Does not run in non-interactive CI.

## SSH arguments

Shared by `preview ssh`, `remote setup ssh`, `remote update ssh`, and `remote clean ssh`.

```text
tunsmith <command> ssh [HOST] [OPTIONS]
```

| Argument / flag | Default | Meaning |
| --- | --- | --- |
| `[HOST]` | prompted (except `remote update`, which picks the first `remotes/*.json`) | Hostname, IP, or `user@host` |
| `-u`, `--user <USER>` | `root` | Used when `HOST` has no `@` |
| `-p`, `--port <PORT>` | `22` | SSH port. Saved profiles do not override this. |
| `--key <PATH>` | `~/.ssh/id_ed25519` if that file exists, else `~/.ssh/id_rsa` if it exists, else `~/.ssh/id_ed25519` | Private key. `~` is expanded. Encrypted keys prompt for a passphrase. |
| `--password` | off | Prompt for a password. Not written to disk. |

`user@host` wins over `--user`. If a `remotes/<host>.json` exists with `ssh_auth_type: "password"` and you do not pass `--key`, Tunsmith prompts for a password again. Auth details: [remote.md](remote.md).

## `preview ssh`

SSH in, print OS / kernel / uptime / CPU / RAM / disk / public IP / local IP / IP forwarding / OpenVPN version, and write `remotes/<host>.json`. Public IP is `curl -s https://ifconfig.me` on the remote host. Does not install or change OpenVPN.

## `remote setup ssh`

Runs the same discovery as `preview ssh`, then:

1. Installs OpenVPN with apt if missing (Debian/Ubuntu only).
2. If `redirect_gateway` is set, enables IPv4 forwarding.
3. Uploads `dist/server/` and enables `openvpn-server@<instance>`.

Requires `tunsmith.json` and `dist/server/`. Prints an iptables NAT example when `redirect_gateway` is on; does not apply it.

## `remote update ssh`

Uploads `dist/server/server.conf` to `/etc/openvpn/server/<instance>.conf` and `systemctl restart openvpn-server@<instance>`. Does not re-upload certificates or keys. Requires an existing remote profile. If `HOST` is omitted, uses the first name in `remotes/` (sorted).

## `remote clean ssh`

Prompts for confirmation, then `systemctl stop` / `disable` `openvpn-server@<instance>`, `rm -f /etc/openvpn/server/<instance>.conf`, and `rm -rf /etc/openvpn/server/<instance>/`. Does not uninstall OpenVPN or revert sysctl. Clears the `setup` field in `remotes/<host>.json`. Instance name comes from that profile if present, otherwise from `tunsmith.json`.

## `remote clean local`

Deletes `dist/` in the current directory. Leaves `pki/`, `tunsmith.json`, and `remotes/` alone.
