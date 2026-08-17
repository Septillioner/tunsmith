# Your own internet VPN on Ubuntu

Have an Ubuntu VPS and want your laptop's internet to leave from that box? This is that path. Template: `gateway-vpn` (full tunnel). Not split-tunnel `cloud-vpn`. Not WireGuard. Not a hosted VPN.

This is **0.1.0** alpha. Private keys are written unencrypted. There is no CRL. Read [security.md](security.md) before `remote setup`. OpenVPN 2.4+ is the expected floor; those versions are not proven yet. See the [compatibility table](../README.md#compatibility).

## You need

- A Rust toolchain on the machine that runs Tunsmith (`cargo install --path .` from this repo)
- An Ubuntu host with SSH (default user `root`)
- An OpenVPN 2.4+ client to import the `.ovpn`

`remote setup` can install the distro `openvpn` package on the host. It does not pin a version.

## 1. Project

All commands use the current working directory. Start in an empty folder.

```bash
tunsmith init --template gateway-vpn
tunsmith config set --host vpn.example.com
tunsmith client add laptop
```

`--host` is the DNS name or IP that clients will dial. It is also the server certificate SAN. `client add` only records the name; certificates are issued on `build`.

## 2. Preview, then build

```bash
tunsmith preview ssh root@203.0.113.10
tunsmith build
```

`preview ssh` writes `remotes/<host>.json`, including the remote OpenVPN version. `build` uses that as the compile target and writes `dist/server/`, `dist/clients/laptop.ovpn`, and `dist/build.json`.

## 3. Deploy

```bash
tunsmith remote setup ssh root@203.0.113.10
```

That installs OpenVPN if needed, uploads `dist/server/`, enables `openvpn-server@<instance>`, and turns on IPv4 forwarding.

Full tunnel then asks to apply NAT masquerade. The prompt defaults to **no**. Confirm it, or clients reach the VPN subnet and not the internet. If several default IPv4 interfaces (or none), pass the egress NIC; Confirm still runs:

```bash
tunsmith remote setup ssh root@203.0.113.10 --nat-interface ens3
```

No TTY: NAT is skipped. Persistence is `tunsmith-nat@<instance>` plus `nat.sh` on the host. Details: [remote.md](remote.md).

## 4. Firewall

Tunsmith does not open cloud security groups or UFW. Allow **UDP 1194** (the `gateway-vpn` default) to the VPS.

If UFW is active, NAT can still be applied while `DEFAULT_FORWARD_POLICY=DROP` drops forwarded packets. Tunsmith warns and does not edit `/etc/default/ufw`.

## 5. Connect

Import `dist/clients/laptop.ovpn` in an OpenVPN 2.4+ client. Do not commit that file; it contains the client key.

Check: your public IP should be the VPS. `preview ssh` may already have printed that address via `ifconfig.me`.

## Not this guide

Split tunnel, client-to-client, extra flags, and Debian-vs-Ubuntu install details live in [getting-started.md](getting-started.md) and [remote.md](remote.md). `client remove` does not revoke certificates.
