# Security

Tunsmith writes unencrypted private keys to disk and can run privileged commands on a remote Linux host. Open source does not make that safe. Read [docs/security.md](docs/security.md) before `remote setup`.

## Reporting a vulnerability

Once this project has a public GitHub repository, report security issues through a **private GitHub security advisory**. Do not open a public issue with exploit details, private keys, or `.ovpn` profiles that embed keys.

Until the repository exists on GitHub, there is no public disclosure channel. Do not use a placeholder email.

## Threat model

The trust model, on-disk key layout, SSH behavior, and known gaps (unencrypted keys, no CRL) are in [docs/security.md](docs/security.md).
