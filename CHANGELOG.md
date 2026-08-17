# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Public name is Tunsmith (`tunsmith` binary, crate, and `tunsmith.json`)
- `block-outside-dns` is no longer written into `server.conf`. When enabled, client profiles get `setenv opt block-outside-dns`.
- CLI output is TTY-aware (respects `NO_COLOR`): status roles, accent labels for build Target/Dialect, and stage prefixes on preview/build/remote.

### Added

- `tunsmith tui`: interactive terminal menu for status, init, config, clients, and build (not a web UI; no remote)
- OpenVPN compatibility table in README (expected 2.4+, recommended 2.6, untested until marked)
- `build --host` and `build --openvpn-version`: default compile target from `preview` remotes; stamp `dist/build.json`; `remote setup`/`update` refuse OpenVPN below 2.4

## [0.1.0] - 2026-08-17

Initial public Rust CLI. Alpha. Not a finished PKI product.

### Added

- `tunsmith` binary: `init`, `config`, `client`, `build`, `template`, `preview`, `remote`
- In-process RSA PKCS#8 keys and certificates (`rcgen`); no OpenSSL CLI
- OpenVPN server.conf and unified `.ovpn` profiles with `dh none`, TLS-Crypt, AES-256-GCM
- Templates: `gateway-vpn`, `cloud-vpn`, `gateway-cloud-vpn`
- SSH deploy to Debian/Ubuntu (`russh` + SFTP, `known_hosts` check, systemd `openvpn-server@`)

### Known gaps

- Private keys are written unencrypted
- No certificate revocation list (CRL); `client remove` is list-only
- No web GUI (terminal `tui` is Unreleased)
- Full-tunnel (`redirect_gateway`) does not configure NAT
