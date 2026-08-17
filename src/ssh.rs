use anyhow::{anyhow, bail, Context, Result};
use dialoguer::Confirm;
use russh::client::Handle;
use russh::keys::{load_secret_key, HashAlg, PrivateKeyWithHashAlg, PublicKey};
use russh::ChannelMsg;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

use crate::constants::{
    DEFAULT_SSH_KEY_ED25519, DEFAULT_SSH_KEY_RSA, DEFAULT_SSH_PORT, JOURNAL_TAIL_LINES,
    KNOWN_HOSTS_FILE, REMOTE_FILE_MODE,
};
use crate::project::AUTH_TYPE_PASSWORD;
use crate::style;

pub struct SshTarget {
    pub host: String,
    pub user: String,
    pub port: u16,
}

pub struct SshAuth {
    pub auth_type: String,
    pub password: Option<String>,
    pub key_path: Option<PathBuf>,
}

pub struct RemoteSession {
    handle: Handle<HostKeyHandler>,
}

struct HostKeyHandler {
    host: String,
    port: u16,
}

impl russh::client::Handler for HostKeyHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        verify_known_host(&self.host, self.port, server_public_key)
    }
}

impl RemoteSession {
    pub async fn open(target: &SshTarget, auth: &SshAuth) -> Result<Self> {
        let config = russh::client::Config::default();
        let handler = HostKeyHandler {
            host: target.host.clone(),
            port: target.port,
        };
        let mut handle = russh::client::connect(
            Arc::new(config),
            (target.host.as_str(), target.port),
            handler,
        )
        .await
        .with_context(|| format!("SSH connect failed: {}:{}", target.host, target.port))?;

        let ok = if auth.auth_type == AUTH_TYPE_PASSWORD {
            let password = auth
                .password
                .as_deref()
                .ok_or_else(|| anyhow!("SSH password is required"))?;
            handle
                .authenticate_password(&target.user, password)
                .await?
                .success()
        } else {
            let key_path = auth
                .key_path
                .clone()
                .ok_or_else(|| anyhow!("SSH private key path is required"))?;
            let key = load_key(&key_path)?;
            let hash = handle.best_supported_rsa_hash().await?.flatten();
            handle
                .authenticate_publickey(
                    &target.user,
                    PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                )
                .await?
                .success()
        };

        if !ok {
            bail!(
                "SSH authentication failed for {}@{}",
                target.user,
                target.host
            );
        }

        Ok(Self { handle })
    }

    pub async fn execute(&self, command: &str) -> Result<String> {
        let (stdout, stderr, code) = self.run(command).await?;
        if code != 0 {
            bail!("Command failed with code {code}: {stderr}");
        }
        Ok(stdout)
    }

    pub async fn execute_or(&self, command: &str, fallback: &str) -> String {
        self.execute(command)
            .await
            .unwrap_or_else(|_| fallback.to_string())
    }

    pub async fn mkdir_p(&self, path: &str) -> Result<()> {
        self.execute(&format!("mkdir -p {path}")).await?;
        Ok(())
    }

    pub async fn upload_file(&self, local: &Path, remote: &str) -> Result<()> {
        let bytes =
            std::fs::read(local).with_context(|| format!("failed to read {}", local.display()))?;
        let parent = remote_parent(remote);
        self.mkdir_p(&parent).await?;

        let channel = self.handle.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .context("failed to start SFTP subsystem (is sftp-server installed?)")?;
        {
            let mut file = sftp
                .open_with_flags(
                    remote,
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                )
                .await
                .with_context(|| format!("SFTP open failed: {remote}"))?;
            file.write_all(&bytes).await?;
            file.flush().await?;
        }
        drop(sftp);

        self.execute(&format!("chmod {REMOTE_FILE_MODE} {remote}"))
            .await?;
        Ok(())
    }

    async fn run(&self, command: &str) -> Result<(String, String, u32)> {
        let mut channel = self.handle.channel_open_session().await?;
        channel.exec(true, command).await?;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut code = 0u32;
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                ChannelMsg::ExtendedData { ref data, ext } if ext == 1 => {
                    stderr.extend_from_slice(data)
                }
                ChannelMsg::ExitStatus { exit_status } => code = exit_status,
                _ => {}
            }
        }
        Ok((
            String::from_utf8_lossy(&stdout).trim().to_string(),
            String::from_utf8_lossy(&stderr).trim().to_string(),
            code,
        ))
    }
}

pub fn parse_ssh_host(host_arg: Option<&str>, default_user: &str) -> Result<(String, String)> {
    match host_arg {
        Some(value) if value.contains('@') => {
            let (user, host) = value.split_once('@').unwrap();
            if user.is_empty() || host.is_empty() {
                bail!("invalid SSH host '{value}'");
            }
            Ok((host.to_string(), user.to_string()))
        }
        Some(host) => Ok((host.to_string(), default_user.to_string())),
        None => Ok((String::new(), default_user.to_string())),
    }
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

pub fn default_ssh_key_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let ed25519 = home.join(DEFAULT_SSH_KEY_ED25519);
    if ed25519.is_file() {
        return Some(ed25519);
    }
    let rsa = home.join(DEFAULT_SSH_KEY_RSA);
    if rsa.is_file() {
        return Some(rsa);
    }
    Some(ed25519)
}

fn load_key(path: &Path) -> Result<russh::keys::PrivateKey> {
    match load_secret_key(path, None) {
        Ok(key) => Ok(key),
        Err(_) => {
            let pass = dialoguer::Password::with_theme(&style::theme())
                .with_prompt(format!("Passphrase for {}", path.display()))
                .allow_empty_password(true)
                .interact()?;
            load_secret_key(path, Some(&pass))
                .with_context(|| format!("failed to load SSH key {}", path.display()))
        }
    }
}

fn verify_known_host(host: &str, port: u16, key: &PublicKey) -> Result<bool> {
    let path = known_hosts_path()?;
    match lookup_known_host(&path, host, port, key)? {
        HostKeyStatus::Match => Ok(true),
        HostKeyStatus::Mismatch => {
            bail!(
                "Host key for {host} does not match {}. Refusing to connect (possible MITM).",
                path.display()
            );
        }
        HostKeyStatus::Unknown => {
            let fp = key.fingerprint(HashAlg::Sha256);
            let prompt = format!(
                "The authenticity of host '{host}' can't be established.\n{fp}\nTrust this host and add it to known_hosts?"
            );
            let yes = Confirm::with_theme(&style::theme())
                .with_prompt(prompt)
                .default(false)
                .interact()?;
            if !yes {
                return Ok(false);
            }
            append_known_host(&path, host, port, key)?;
            Ok(true)
        }
    }
}

enum HostKeyStatus {
    Match,
    Mismatch,
    Unknown,
}

fn known_hosts_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(KNOWN_HOSTS_FILE))
}

fn host_pattern(host: &str, port: u16) -> String {
    if port == DEFAULT_SSH_PORT {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

fn lookup_known_host(path: &Path, host: &str, port: u16, key: &PublicKey) -> Result<HostKeyStatus> {
    if !path.is_file() {
        return Ok(HostKeyStatus::Unknown);
    }
    let body = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let want = host_pattern(host, port);
    let key_openssh = key.to_openssh().unwrap_or_default();
    let mut seen = false;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("@cert-authority ").unwrap_or(line);
        if line.starts_with('@') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(names) = parts.next() else { continue };
        if names.starts_with("|1|") {
            continue;
        }
        if !names.split(',').any(|n| n == want || n == host) {
            continue;
        }
        seen = true;
        let algo = parts.next().unwrap_or("");
        let blob = parts.next().unwrap_or("");
        let stored = format!("{algo} {blob}");
        if key_openssh.starts_with(&stored) || stored == key_openssh {
            return Ok(HostKeyStatus::Match);
        }
    }
    if seen {
        Ok(HostKeyStatus::Mismatch)
    } else {
        Ok(HostKeyStatus::Unknown)
    }
}

fn append_known_host(path: &Path, host: &str, port: u16, key: &PublicKey) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pattern = host_pattern(host, port);
    let openssh = key.to_openssh().context("failed to encode host key")?;
    let line = format!("{pattern} {openssh}\n");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

fn remote_parent(path: &str) -> String {
    match path.rfind('/') {
        Some(idx) if idx > 0 => path[..idx].to_string(),
        _ => ".".to_string(),
    }
}

#[allow(dead_code)]
pub fn journal_tail_command(service: &str) -> String {
    format!("journalctl -xeu {service} --no-pager | tail -n {JOURNAL_TAIL_LINES}")
}

pub fn prompt_password() -> Result<String> {
    Ok(dialoguer::Password::with_theme(&style::theme())
        .with_prompt("SSH password")
        .interact()?)
}
