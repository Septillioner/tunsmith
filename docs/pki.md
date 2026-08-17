# PKI

Tunsmith is a project-local CA, not a general-purpose PKI. All paths are relative to the working directory. Keys are RSA, PKCS#8 PEM (LF line endings), unencrypted. Signing is RSA-SHA256 via `rcgen`. There is no OpenSSL CLI and no passphrase wrapping in 0.1.0.

On Unix, secret files are created with mode `600`. On Windows that chmod is skipped.

## Key sizes and validity

| Material | RSA bits | Default validity | Clock |
| --- | --- | --- | --- |
| Root CA | 4096 | 10 years (`init --validity`) | `not_before` = now (UTC); `not_after` = now + years × 365 days |
| Server | 2048 | 2 years | same day-count rule |
| Client | 2048 | 1 year | same |

Validity is not leap-year aware.

## Subjects and extensions

**CA** (`init`): CN `{instance} Root CA`, O = `--org` or the instance name, C = `--country` if set. `CA:TRUE` (unconstrained), keyCertSign + cRLSign. Self-signed.

**Server** (first `build` that finds no `pki/server/server.crt`): CN `server`, O = instance name. SAN is an IP address if `--host` parses as one, otherwise a DNS name. EKU serverAuth. Digital signature + key encipherment. Not a CA.

**Client** (first `build` for a name with no `pki/clients/<name>/client.crt`): CN = client name. EKU clientAuth. Digital signature. Not a CA. No SAN.

## Layout under `pki/`

```text
pki/
  ca.crt
  ca.key                 # unencrypted PKCS#8
  server/
    server.crt
    server.key           # unencrypted PKCS#8
    tls-crypt.key        # OpenVPN static key V1, 256 random bytes
  clients/
    <name>/
      client.crt
      client.key         # unencrypted PKCS#8
```

`dist/server/` is a copy of the server files plus generated `server.conf`. `dist/clients/<name>.ovpn` inlines CA, client cert, client key, and TLS-Crypt.

`build` does not overwrite existing CA, server, client, or TLS-Crypt files. Changing `--host` after the server cert exists does **not** reissue it; delete `pki/server/` (or `init --force`) if the SAN must change. `init --force` deletes the entire `pki/` tree and creates a new CA.

## TLS-Crypt

`pki/server/tls-crypt.key` is generated once, on first `build`. It is a 256-byte OpenVPN static key, not a certificate. The same key is embedded in every client profile.

## `client remove` does not revoke

`tunsmith client remove <name>` only removes the name from `tunsmith.json`. It leaves `pki/clients/<name>/` on disk. The certificate remains valid until `not_after`.

There is no CRL, no `crl-verify` in generated configs, and no OCSP. A removed client can still connect if they keep their `.ovpn` (or if you `client add` the same name and `build` reuses the existing PEM files).

Treat a removed client as still trusted until you replace the CA (`init --force` and a full rebuild/redeploy) or add revocation yourself. That gap is also in [security.md](security.md).
