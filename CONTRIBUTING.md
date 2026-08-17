# Contributing

Please read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Security issues go through [SECURITY.md](SECURITY.md), not public issues.

## Build

```bash
cargo build
```

Run without installing:

```bash
cargo run -- --help
```

Run `cargo test` for the unit suite (pure logic; no remote host).

## Command surface

Keep new work on the existing CLI. Entry is `src/cli.rs` (clap). Commands:

- `init` — create `tunsmith.json` and `pki/`
- `config set` / `config show`
- `client add` / `client remove` / `client list` (`remove` does not revoke)
- `build` — write `dist/`
- `template` — `gateway-vpn`, `cloud-vpn`, `gateway-cloud-vpn`
- `preview ssh`
- `remote setup ssh` / `remote update ssh` / `remote clean ssh` / `remote clean local`
- `tui` — terminal menu over local commands; not a web UI

Do not add a web GUI. Do not invent a GitHub URL in docs or `Cargo.toml`. Runtime artifacts (`pki/`, `tunsmith.json`, `remotes/`, `dist/`) stay gitignored.

## Patches

- Match the existing Rust style in `src/`.
- Do not commit private keys, `.ovpn` profiles, or `pki/`.
- Document user-facing behavior in `README.md` and `docs/`. If it changes risk, update `docs/security.md`.
