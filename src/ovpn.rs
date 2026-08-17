use anyhow::{bail, Result};
use rand::RngCore;

use crate::constants::{
    DEFAULT_AUTH, DEFAULT_CIPHER, DEFAULT_KEEPALIVE_INTERVAL, DEFAULT_KEEPALIVE_TIMEOUT,
    DEFAULT_VPN_PORT, EXPLICIT_EXIT_NOTIFY, GENERATED_BY_COMMENT, IPV4_MAX_PREFIX, LINUX_GROUP,
    LINUX_USER, MIN_TLS_VERSION_LINE, MUTE_LOG_REPEAT, OPEN_VPN_STATUS_FILE,
    OPEN_VPN_STATUS_LOG_PATH, STATUS_LOG_INTERVAL, TLS_CRYPT_HEX_LINE_CHARS, TLS_CRYPT_KEY_BYTES,
    TOPOLOGY,
};

pub struct VpnEndpoints {
    pub host: String,
    pub port: u16,
    pub protocol: String,
    pub device: String,
}

pub struct VpnCrypto {
    pub cipher: String,
    pub auth: String,
}

pub struct VpnNetwork {
    pub subnet: String,
    pub redirect_gateway: bool,
    pub client_to_client: bool,
    pub dns: Vec<String>,
    pub block_outside_dns: bool,
}

pub struct VpnPkiPaths {
    pub ca: String,
    pub cert: String,
    pub key: String,
    pub tls_crypt: String,
}

pub struct ClientBundle {
    pub ca_cert: String,
    pub client_cert: String,
    pub client_key: String,
    pub tls_crypt_key: String,
}

pub fn build_server_config(
    endpoints: &VpnEndpoints,
    crypto: &VpnCrypto,
    network: &VpnNetwork,
    pki_paths: &VpnPkiPaths,
    keepalive: &str,
    verb: u8,
) -> Result<String> {
    let (net_addr, netmask) = parse_subnet(&network.subnet)?;
    let (ka_interval, ka_timeout) = parse_keepalive(keepalive);
    let cipher = if crypto.cipher.is_empty() {
        DEFAULT_CIPHER
    } else {
        crypto.cipher.as_str()
    };
    let auth = if crypto.auth.is_empty() {
        DEFAULT_AUTH
    } else {
        crypto.auth.as_str()
    };
    let port = if endpoints.port == 0 {
        DEFAULT_VPN_PORT
    } else {
        endpoints.port
    };

    let mut lines = Vec::new();
    lines.push("# OpenVPN Server Configuration".to_string());
    lines.push(GENERATED_BY_COMMENT.to_string());
    lines.push(String::new());
    lines.push(format!("port {port}"));
    lines.push(format!("proto {}", endpoints.protocol));
    lines.push(format!("dev {}", endpoints.device));
    lines.push(format!("topology {TOPOLOGY}"));
    lines.push(String::new());

    lines.push("# PKI Configuration".to_string());
    lines.push(format!("ca {}", pki_paths.ca));
    lines.push(format!("cert {}", pki_paths.cert));
    lines.push(format!("key {}", pki_paths.key));
    lines.push("dh none".to_string());
    lines.push(String::new());

    lines.push("# Network Configuration".to_string());
    lines.push(format!("server {net_addr} {netmask}"));
    lines.push(format!("ifconfig-pool-persist {OPEN_VPN_STATUS_LOG_PATH}"));
    lines.push(String::new());

    lines.push("# TLS Security".to_string());
    lines.push(format!("tls-crypt {}", pki_paths.tls_crypt));
    lines.push(MIN_TLS_VERSION_LINE.to_string());
    lines.push(String::new());

    lines.push("# Client Push Options".to_string());
    lines.push(format!("push \"route {net_addr} {netmask}\""));
    if network.redirect_gateway {
        lines.push("push \"redirect-gateway def1 bypass-dhcp\"".to_string());
    }
    for dns in &network.dns {
        lines.push(format!("push \"dhcp-option DNS {dns}\""));
    }
    lines.push(String::new());

    if network.client_to_client {
        lines.push("# Client-to-Client".to_string());
        lines.push("client-to-client".to_string());
        lines.push(String::new());
    }

    if network.block_outside_dns {
        lines.push("block-outside-dns".to_string());
        lines.push(String::new());
    }

    lines.push("# Performance & Security".to_string());
    lines.push(format!("keepalive {ka_interval} {ka_timeout}"));
    lines.push(format!("cipher {cipher}"));
    lines.push(format!("auth {auth}"));
    lines.push(String::new());

    lines.push("# Platform-specific (Linux)".to_string());
    lines.push(format!("user {LINUX_USER}"));
    lines.push(format!("group {LINUX_GROUP}"));
    lines.push(String::new());

    lines.push("# Persistence".to_string());
    lines.push("persist-key".to_string());
    lines.push("persist-tun".to_string());
    lines.push(String::new());

    lines.push("# Logging".to_string());
    lines.push(format!(
        "status {OPEN_VPN_STATUS_FILE} {STATUS_LOG_INTERVAL}"
    ));
    lines.push(format!("verb {verb}"));
    lines.push(format!("mute {MUTE_LOG_REPEAT}"));
    lines.push(String::new());

    if endpoints.protocol == "udp" {
        lines.push(format!("explicit-exit-notify {EXPLICIT_EXIT_NOTIFY}"));
        lines.push(String::new());
    }

    Ok(lines.join("\n"))
}

pub fn build_client_config(
    endpoints: &VpnEndpoints,
    crypto: &VpnCrypto,
    bundle: &ClientBundle,
    verb: u8,
) -> String {
    let cipher = if crypto.cipher.is_empty() {
        DEFAULT_CIPHER
    } else {
        crypto.cipher.as_str()
    };
    let auth = if crypto.auth.is_empty() {
        DEFAULT_AUTH
    } else {
        crypto.auth.as_str()
    };

    let mut lines = Vec::new();
    lines.push("# OpenVPN Client Configuration".to_string());
    lines.push(GENERATED_BY_COMMENT.to_string());
    lines.push(String::new());
    lines.push("client".to_string());
    lines.push(format!("dev {}", endpoints.device));
    lines.push(format!("proto {}", endpoints.protocol));
    lines.push(format!("remote {} {}", endpoints.host, endpoints.port));
    lines.push(String::new());

    lines.push("# Security".to_string());
    lines.push("resolv-retry infinite".to_string());
    lines.push("nobind".to_string());
    lines.push("remote-cert-tls server".to_string());
    lines.push(format!("cipher {cipher}"));
    lines.push(format!("auth {auth}"));
    lines.push(MIN_TLS_VERSION_LINE.to_string());
    lines.push(String::new());

    lines.push("# Persistence".to_string());
    lines.push("persist-key".to_string());
    lines.push("persist-tun".to_string());
    lines.push(String::new());

    lines.push(format!("verb {verb}"));
    lines.push(format!("mute {MUTE_LOG_REPEAT}"));
    lines.push(String::new());

    lines.push("<ca>".to_string());
    lines.push(trim_pem(&bundle.ca_cert));
    lines.push("</ca>".to_string());
    lines.push(String::new());

    lines.push("<cert>".to_string());
    lines.push(trim_pem(&bundle.client_cert));
    lines.push("</cert>".to_string());
    lines.push(String::new());

    lines.push("<key>".to_string());
    lines.push(trim_pem(&bundle.client_key));
    lines.push("</key>".to_string());
    lines.push(String::new());

    lines.push("<tls-crypt>".to_string());
    lines.push(trim_pem(&bundle.tls_crypt_key));
    lines.push("</tls-crypt>".to_string());
    lines.push(String::new());

    lines.join("\n")
}

pub fn generate_tls_crypt_key() -> String {
    let mut bytes = vec![0u8; TLS_CRYPT_KEY_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut lines = vec!["-----BEGIN OpenVPN Static key V1-----".to_string()];
    for chunk in hex.as_bytes().chunks(TLS_CRYPT_HEX_LINE_CHARS) {
        lines.push(String::from_utf8_lossy(chunk).into_owned());
    }
    lines.push("-----END OpenVPN Static key V1-----".to_string());
    lines.join("\n")
}

fn parse_subnet(subnet: &str) -> Result<(String, String)> {
    let (addr, prefix) = subnet.split_once('/').unwrap_or((subnet, "24"));
    let bits: u32 = prefix.parse().unwrap_or(24);
    if bits > IPV4_MAX_PREFIX {
        bail!("invalid subnet prefix: {prefix}");
    }
    Ok((addr.to_string(), prefix_to_netmask(bits)))
}

fn prefix_to_netmask(bits: u32) -> String {
    let mask = if bits == 0 {
        0u32
    } else {
        u32::MAX << (IPV4_MAX_PREFIX - bits)
    };
    format!(
        "{}.{}.{}.{}",
        (mask >> 24) & 0xff,
        (mask >> 16) & 0xff,
        (mask >> 8) & 0xff,
        mask & 0xff
    )
}

fn parse_keepalive(keepalive: &str) -> (u32, u32) {
    let mut parts = keepalive.split_whitespace();
    let interval = parts
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_KEEPALIVE_INTERVAL);
    let timeout = parts
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_KEEPALIVE_TIMEOUT);
    (interval, timeout)
}

fn trim_pem(pem: &str) -> String {
    pem.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        CA_CERT_FILE, DEFAULT_AUTH, DEFAULT_CIPHER, DEFAULT_DEVICE, DEFAULT_KEEPALIVE,
        DEFAULT_PROTOCOL, DEFAULT_SUBNET, DEFAULT_VERB, DEFAULT_VPN_PORT, SERVER_CERT_FILE,
        SERVER_KEY_FILE, TLS_CRYPT_FILE, TLS_CRYPT_HEX_LINE_CHARS, TLS_CRYPT_KEY_BYTES,
    };

    const NETMASK_SLASH_24: &str = "255.255.255.0";
    const NETMASK_NO_BITS: &str = "0.0.0.0";
    const NETMASK_ALL_BITS: &str = "255.255.255.255";
    const OPENVPN_STATIC_KEY_HEADER: &str = "-----BEGIN OpenVPN Static key V1-----";
    const OPENVPN_STATIC_KEY_FOOTER: &str = "-----END OpenVPN Static key V1-----";

    fn sample_endpoints() -> VpnEndpoints {
        VpnEndpoints {
            host: "vpn.example.com".to_string(),
            port: DEFAULT_VPN_PORT,
            protocol: DEFAULT_PROTOCOL.to_string(),
            device: DEFAULT_DEVICE.to_string(),
        }
    }

    fn sample_crypto() -> VpnCrypto {
        VpnCrypto {
            cipher: DEFAULT_CIPHER.to_string(),
            auth: DEFAULT_AUTH.to_string(),
        }
    }

    fn sample_network() -> VpnNetwork {
        VpnNetwork {
            subnet: DEFAULT_SUBNET.to_string(),
            redirect_gateway: true,
            client_to_client: false,
            dns: crate::constants::default_dns(),
            block_outside_dns: false,
        }
    }

    fn sample_pki_paths() -> VpnPkiPaths {
        VpnPkiPaths {
            ca: CA_CERT_FILE.to_string(),
            cert: SERVER_CERT_FILE.to_string(),
            key: SERVER_KEY_FILE.to_string(),
            tls_crypt: TLS_CRYPT_FILE.to_string(),
        }
    }

    #[test]
    fn build_server_config_uses_dh_none_tls_crypt_and_cipher() {
        let config = build_server_config(
            &sample_endpoints(),
            &sample_crypto(),
            &sample_network(),
            &sample_pki_paths(),
            DEFAULT_KEEPALIVE,
            DEFAULT_VERB,
        )
        .unwrap();

        assert!(config.contains("dh none"));
        assert!(!config.contains("dh.pem"));
        assert!(!config.contains("dhparam"));
        assert!(config.contains(&format!("tls-crypt {TLS_CRYPT_FILE}")));
        assert!(config.contains(&format!("cipher {DEFAULT_CIPHER}")));
        assert!(config.contains(&format!("auth {DEFAULT_AUTH}")));
        assert!(config.contains(MIN_TLS_VERSION_LINE));
    }

    #[test]
    fn build_server_config_applies_subnet_netmask_and_defaults() {
        let mut endpoints = sample_endpoints();
        endpoints.port = 0;
        let crypto = VpnCrypto {
            cipher: String::new(),
            auth: String::new(),
        };
        let expected_addr = DEFAULT_SUBNET.split_once('/').unwrap().0;

        let config = build_server_config(
            &endpoints,
            &crypto,
            &sample_network(),
            &sample_pki_paths(),
            DEFAULT_KEEPALIVE,
            DEFAULT_VERB,
        )
        .unwrap();

        assert!(config.contains(&format!("port {DEFAULT_VPN_PORT}")));
        assert!(config.contains(&format!("server {expected_addr} {NETMASK_SLASH_24}")));
        assert!(config.contains(&format!("cipher {DEFAULT_CIPHER}")));
        assert!(config.contains(&format!("auth {DEFAULT_AUTH}")));
        assert!(config.contains(&format!("keepalive {DEFAULT_KEEPALIVE}")));
    }

    #[test]
    fn build_client_config_inlines_pki_tags() {
        let bundle = ClientBundle {
            ca_cert: "  -----BEGIN CERTIFICATE-----\nCA_MARKER\n-----END CERTIFICATE-----  "
                .to_string(),
            client_cert: "-----BEGIN CERTIFICATE-----\nCERT_MARKER\n-----END CERTIFICATE-----"
                .to_string(),
            client_key: "-----BEGIN PRIVATE KEY-----\nKEY_MARKER\n-----END PRIVATE KEY-----"
                .to_string(),
            tls_crypt_key:
                "-----BEGIN OpenVPN Static key V1-----\nTLS_MARKER\n-----END OpenVPN Static key V1-----"
                    .to_string(),
        };

        let config = build_client_config(
            &sample_endpoints(),
            &sample_crypto(),
            &bundle,
            DEFAULT_VERB,
        );

        assert!(config.contains("<ca>"));
        assert!(config.contains("</ca>"));
        assert!(config.contains("<cert>"));
        assert!(config.contains("</cert>"));
        assert!(config.contains("<key>"));
        assert!(config.contains("</key>"));
        assert!(config.contains("<tls-crypt>"));
        assert!(config.contains("</tls-crypt>"));
        assert!(config.contains("CA_MARKER"));
        assert!(config.contains("CERT_MARKER"));
        assert!(config.contains("KEY_MARKER"));
        assert!(config.contains("TLS_MARKER"));
        assert!(!config.contains("  -----BEGIN CERTIFICATE-----"));
    }

    #[test]
    fn generate_tls_crypt_key_matches_openvpn_static_key_format() {
        let key = generate_tls_crypt_key();
        let lines: Vec<&str> = key.lines().collect();
        assert_eq!(lines.first().copied(), Some(OPENVPN_STATIC_KEY_HEADER));
        assert_eq!(lines.last().copied(), Some(OPENVPN_STATIC_KEY_FOOTER));

        let hex_lines = &lines[1..lines.len() - 1];
        let expected_hex_chars = TLS_CRYPT_KEY_BYTES * 2;
        assert_eq!(expected_hex_chars % TLS_CRYPT_HEX_LINE_CHARS, 0);
        assert_eq!(hex_lines.len(), expected_hex_chars / TLS_CRYPT_HEX_LINE_CHARS);
        for line in hex_lines {
            assert_eq!(line.len(), TLS_CRYPT_HEX_LINE_CHARS);
            assert!(line.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        }
    }

    #[test]
    fn parse_subnet_splits_default_and_rejects_oversize_prefix() {
        let expected_addr = DEFAULT_SUBNET.split_once('/').unwrap().0;
        let (addr, mask) = parse_subnet(DEFAULT_SUBNET).unwrap();
        assert_eq!(addr, expected_addr);
        assert_eq!(mask, NETMASK_SLASH_24);

        let oversize = format!("10.8.0.0/{}", IPV4_MAX_PREFIX + 1);
        assert!(parse_subnet(&oversize).is_err());
    }

    #[test]
    fn prefix_to_netmask_covers_empty_and_full_masks() {
        assert_eq!(prefix_to_netmask(0), NETMASK_NO_BITS);
        assert_eq!(prefix_to_netmask(IPV4_MAX_PREFIX), NETMASK_ALL_BITS);
        assert_eq!(prefix_to_netmask(24), NETMASK_SLASH_24);
    }
}
