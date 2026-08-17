# Getting started

Requires a Rust toolchain. Remote commands also need a Debian or Ubuntu host with SSH. OpenVPN 2.4+ on that host (Tunsmith can install it). There is no OpenSSL CLI dependency; keys are RSA via `rcgen`. VPN configs use `dh none`.

## Install

From a clone of this tree:

```bash
cargo install --path .
```

Or run without installing:

```bash
cargo run -- --help
```

`cargo run --` takes the same arguments as `tunsmith`. Example: `cargo run -- init --template gateway-vpn`.

For a terminal menu over the same local commands (`init` / `config` / `client` / `build`):

```bash
tunsmith tui
```

That is not a web UI. Remote setup is still the `remote` commands.

## First project

All commands operate on the current working directory. Create an empty folder and work there.

```bash
tunsmith init --template gateway-vpn
tunsmith config set --host vpn.example.com
tunsmith client add laptop
tunsmith preview ssh root@203.0.113.10
tunsmith build
```

`init` writes `tunsmith.json` and a 4096-bit root CA under `pki/`. `--template` is optional; without it you get the default server settings (UDP 1194, `10.8.0.0/24`, split tunnel). Instance name defaults to the folder name and must be ASCII letters, digits, `-`, or `_`.

`config set --host` is required before `build`. Use the DNS name or IP that clients will dial. That value is also the server certificate SAN.

`client add` only appends a name to `tunsmith.json`. Certificates are issued on `build`.

`preview ssh` writes `remotes/<host>.json` including the remote OpenVPN version. `build` uses that as the default compile target. Override with `build --openvpn-version X.Y` (or `X.Y.Z`). No preview: baseline OpenVPN 2.4. This slice still writes 2.4 syntax (`cipher`, `tls-crypt`, `dh none`).

`build` writes `dist/build.json`, `dist/server/` (OpenVPN server files) and `dist/clients/*.ovpn` (unified client profiles). Import the `.ovpn` in an OpenVPN 2.4+ client.

To install on a remote host:

```bash
tunsmith remote setup ssh root@203.0.113.10
```

That command needs `dist/` already built. After apt it checks the live OpenVPN version against `dist/build.json` and refuses below 2.4. Details: [remote.md](remote.md). Full flag list: [commands.md](commands.md).

## Files that appear

| Path | When | What |
| --- | --- | --- |
| `tunsmith.json` | `init` | Project config, client list |
| `pki/ca.crt`, `pki/ca.key` | `init` | Root CA (key unencrypted) |
| `pki/server/` | first `build` | Server cert, key, TLS-Crypt |
| `pki/clients/<name>/` | first `build` for that client | Client cert and key |
| `dist/build.json` | `build` | Compile stamp: OpenVPN target and dialect |
| `dist/server/` | `build` | `server.conf` plus copies of CA, server cert/key, TLS-Crypt |
| `dist/clients/<name>.ovpn` | `build` | Unified profile, includes the client key |
| `remotes/<host>.json` | `preview ssh` or `remote * ssh` | SSH host profile (no passwords) |

`build` deletes and recreates `dist/` every run. Existing files under `pki/` are reused (server and client certs are not reissued if they already exist).

Those paths are gitignored. Do not commit `pki/` or `dist/clients/*.ovpn`.

## Templates

`tunsmith template` lists the three built-in templates. `init --template` copies their server defaults into `tunsmith.json`:

| Template | Tunnel | Subnet | Notes |
| --- | --- | --- | --- |
| `gateway-vpn` | Full (`redirect_gateway`) | `10.8.0.0/24` | Pushes 8.8.8.8 / 8.8.4.4; client `.ovpn` gets `setenv opt block-outside-dns` |
| `cloud-vpn` | Split | `10.10.0.0/24` | Client-to-client |
| `gateway-cloud-vpn` | Full | `10.12.0.0/24` | Full tunnel plus client-to-client |

`build` always pushes DNS. If `tunsmith.json` has no `dns` list, it uses 8.8.8.8 and 8.8.4.4.

## Full tunnel and NAT

`redirect_gateway` (templates `gateway-vpn` and `gateway-cloud-vpn`, or `config set --redirect`) pushes `redirect-gateway def1 bypass-dhcp` so clients send all IPv4 through the VPN.

`remote setup` will enable IPv4 forwarding on the host (`sysctl` + `/etc/sysctl.conf`). It does **not** configure NAT. Without masquerade, clients typically reach the VPN subnet and nothing else.

Tunsmith prints an example and stops:

```text
iptables -t nat -A POSTROUTING -s <subnet> -o eth0 -j MASQUERADE
```

Replace `eth0` with the server's public interface. Persist that rule yourself (iptables-persistent, nftables, cloud security groups, and UDP/TCP 1194 are all outside Tunsmith).

## Next

- [commands.md](commands.md) — flags and defaults
- [pki.md](pki.md) — what `pki/` contains and what `client remove` does not do
- [remote.md](remote.md) — SSH, apt, systemd
- [security.md](security.md) — threat model
