use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::constants::{
    APP_NAME, BUILD_STAMP_FILE, CA_CERT_FILE, CA_KEY_FILE, CLIENTS_DIR, CLIENT_CERT_FILE,
    CLIENT_KEY_FILE, CONFIG_FILE, DEFAULT_AUTH, DEFAULT_CIPHER, DEFAULT_DEVICE, DEFAULT_KEEPALIVE,
    DEFAULT_PROTOCOL, DEFAULT_SUBNET, DEFAULT_VERB, DEFAULT_VPN_PORT, DIST_CLIENTS_DIR, DIST_DIR,
    DIST_SERVER_DIR, PKI_DIR, PROJECT_VERSION, REMOTES_DIR, SERVER_CERT_FILE, SERVER_DIR,
    SERVER_KEY_FILE, TLS_CRYPT_FILE,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunsmithConfig {
    pub instance_name: String,
    pub version: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub server: ServerConfig,
    pub clients: Vec<ClientEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub protocol: String,
    pub subnet: String,
    pub device: String,
    pub redirect_gateway: bool,
    pub cipher: String,
    pub auth: String,
    pub keepalive: String,
    pub verb: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_to_client: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_outside_dns: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEntry {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteProfile {
    pub host: String,
    pub user: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpn_version: Option<String>,
    pub last_seen: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_auth_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_key_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<RemoteSetup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSetup {
    pub instance_name: String,
    pub config_dir: String,
    pub main_config_path: String,
    pub deployed_at: String,
    pub status: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: DEFAULT_VPN_PORT,
            protocol: DEFAULT_PROTOCOL.to_string(),
            subnet: DEFAULT_SUBNET.to_string(),
            device: DEFAULT_DEVICE.to_string(),
            redirect_gateway: false,
            cipher: DEFAULT_CIPHER.to_string(),
            auth: DEFAULT_AUTH.to_string(),
            keepalive: DEFAULT_KEEPALIVE.to_string(),
            verb: DEFAULT_VERB,
            dns: None,
            client_to_client: None,
            block_outside_dns: None,
        }
    }
}

impl TunsmithConfig {
    pub fn new(instance_name: String, template: Option<String>, server: ServerConfig) -> Self {
        Self {
            instance_name,
            version: PROJECT_VERSION.to_string(),
            created_at: now_rfc3339(),
            template,
            server,
            clients: Vec::new(),
        }
    }
}

pub fn now_rfc3339() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| now.to_string())
}

pub fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}

pub fn pki_dir() -> PathBuf {
    PathBuf::from(PKI_DIR)
}

pub fn ca_cert_path() -> PathBuf {
    pki_dir().join(CA_CERT_FILE)
}

pub fn ca_key_path() -> PathBuf {
    pki_dir().join(CA_KEY_FILE)
}

pub fn server_pki_dir() -> PathBuf {
    pki_dir().join(SERVER_DIR)
}

pub fn client_pki_dir(name: &str) -> PathBuf {
    pki_dir().join(CLIENTS_DIR).join(name)
}

pub fn server_cert_path() -> PathBuf {
    server_pki_dir().join(SERVER_CERT_FILE)
}

pub fn server_key_path() -> PathBuf {
    server_pki_dir().join(SERVER_KEY_FILE)
}

pub fn tls_crypt_path() -> PathBuf {
    server_pki_dir().join(TLS_CRYPT_FILE)
}

pub fn client_cert_path(name: &str) -> PathBuf {
    client_pki_dir(name).join(CLIENT_CERT_FILE)
}

pub fn client_key_path(name: &str) -> PathBuf {
    client_pki_dir(name).join(CLIENT_KEY_FILE)
}

pub fn remotes_dir() -> PathBuf {
    PathBuf::from(REMOTES_DIR)
}

pub fn remote_profile_path(host: &str) -> PathBuf {
    remotes_dir().join(format!("{host}.json"))
}

pub fn dist_dir() -> PathBuf {
    PathBuf::from(DIST_DIR)
}

pub fn dist_server_dir() -> PathBuf {
    dist_dir().join(DIST_SERVER_DIR)
}

pub fn dist_clients_dir() -> PathBuf {
    dist_dir().join(DIST_CLIENTS_DIR)
}

pub fn dist_build_stamp_path() -> PathBuf {
    dist_dir().join(BUILD_STAMP_FILE)
}

pub fn pki_exists() -> bool {
    ca_cert_path().is_file() && ca_key_path().is_file()
}

pub fn load_config() -> Result<Option<TunsmithConfig>> {
    let path = config_path();
    if !path.is_file() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let cfg = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(cfg))
}

pub fn require_config() -> Result<TunsmithConfig> {
    load_config()?
        .ok_or_else(|| anyhow::anyhow!("Project not initialized. Run \"{APP_NAME} init\" first."))
}

pub fn save_config(config: &TunsmithConfig) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    fs::write(config_path(), json + "\n")?;
    Ok(())
}

pub fn load_remote_profile(host: &str) -> Result<Option<RemoteProfile>> {
    let path = remote_profile_path(host);
    if !path.is_file() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(Some(serde_json::from_str(&raw)?))
}

pub fn save_remote_profile(profile: &RemoteProfile) -> Result<()> {
    fs::create_dir_all(remotes_dir())?;
    let json = serde_json::to_string_pretty(profile)?;
    fs::write(remote_profile_path(&profile.host), json + "\n")?;
    Ok(())
}

pub fn list_remote_hosts() -> Result<Vec<String>> {
    let dir = remotes_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut hosts = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(host) = name.strip_suffix(".json") {
            hosts.push(host.to_string());
        }
    }
    hosts.sort();
    Ok(hosts)
}

pub fn validate_instance_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("instance name must not be empty");
    }
    let valid = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        bail!("instance name must contain only ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))
}

pub fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    restrict_file_mode(path)?;
    Ok(())
}

pub fn write_public_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn restrict_file_mode(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    let _ = path;
    Ok(())
}

pub const AUTH_TYPE_KEY: &str = "key";
pub const AUTH_TYPE_PASSWORD: &str = "password";
pub const SETUP_STATUS_DEPLOYED: &str = "deployed";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::PROJECT_VERSION;
    use crate::templates::TEMPLATE_GATEWAY_VPN;

    #[test]
    fn validate_instance_name_allows_ascii_alnum_hyphen_underscore() {
        assert!(validate_instance_name("vpn").is_ok());
        assert!(validate_instance_name("vpn-1").is_ok());
        assert!(validate_instance_name("vpn_1").is_ok());
        assert!(validate_instance_name("VPN1").is_ok());
    }

    #[test]
    fn validate_instance_name_rejects_spaces_slash_and_empty() {
        assert!(validate_instance_name("").is_err());
        assert!(validate_instance_name("foo bar").is_err());
        assert!(validate_instance_name("foo/bar").is_err());
        assert!(validate_instance_name("../etc").is_err());
        assert!(validate_instance_name("foo;rm").is_err());
        assert!(validate_instance_name("foo.bar").is_err());
    }

    #[test]
    fn tunsmith_config_serde_roundtrip() {
        let mut server = ServerConfig::default();
        server.host = "vpn.example.com".to_string();
        server.dns = Some(crate::constants::default_dns());
        server.client_to_client = Some(true);
        server.block_outside_dns = Some(false);

        let mut config = TunsmithConfig::new(
            "lab-vpn".to_string(),
            Some(TEMPLATE_GATEWAY_VPN.to_string()),
            server,
        );
        config.clients.push(ClientEntry {
            name: "laptop".to_string(),
        });

        let json = serde_json::to_string_pretty(&config).unwrap();
        let restored: TunsmithConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.instance_name, config.instance_name);
        assert_eq!(restored.version, PROJECT_VERSION);
        assert_eq!(restored.template.as_deref(), Some(TEMPLATE_GATEWAY_VPN));
        assert_eq!(restored.created_at, config.created_at);
        assert_eq!(restored.server.host, config.server.host);
        assert_eq!(restored.server.port, config.server.port);
        assert_eq!(restored.server.protocol, config.server.protocol);
        assert_eq!(restored.server.subnet, config.server.subnet);
        assert_eq!(restored.server.cipher, config.server.cipher);
        assert_eq!(restored.server.auth, config.server.auth);
        assert_eq!(restored.server.dns, config.server.dns);
        assert_eq!(
            restored.server.client_to_client,
            config.server.client_to_client
        );
        assert_eq!(
            restored.server.block_outside_dns,
            config.server.block_outside_dns
        );
        assert_eq!(restored.clients.len(), 1);
        assert_eq!(restored.clients[0].name, "laptop");
    }

    #[test]
    fn tunsmith_config_omits_none_optional_fields() {
        let config = TunsmithConfig::new("lab".to_string(), None, ServerConfig::default());
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("\"dns\""));
        assert!(!json.contains("\"template\""));
        assert!(!json.contains("\"client_to_client\""));
        assert!(!json.contains("\"block_outside_dns\""));

        let restored: TunsmithConfig = serde_json::from_str(&json).unwrap();
        assert!(restored.template.is_none());
        assert!(restored.server.dns.is_none());
        assert!(restored.server.client_to_client.is_none());
        assert!(restored.server.block_outside_dns.is_none());
    }
}
