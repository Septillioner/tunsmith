use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

use crate::constants::{
    APP_NAME, DIALECT_OPENVPN_2_4, MIN_OPENVPN_MAJOR, MIN_OPENVPN_MINOR, OPENVPN_NOT_INSTALLED,
    PROJECT_VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OpenVpnVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl OpenVpnVersion {
    pub fn baseline() -> Self {
        Self {
            major: MIN_OPENVPN_MAJOR,
            minor: MIN_OPENVPN_MINOR,
            patch: 0,
        }
    }

    pub fn meets_minimum(self) -> bool {
        self >= Self::baseline()
    }
}

impl fmt::Display for OpenVpnVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerRole {
    Server,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerOs {
    Linux,
    Windows,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerTarget {
    pub role: PeerRole,
    pub os: PeerOs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<OpenVpnVersion>,
}

impl PeerTarget {
    pub fn linux_server(version: Option<OpenVpnVersion>) -> Self {
        Self {
            role: PeerRole::Server,
            os: PeerOs::Linux,
            version,
        }
    }

    pub fn mixed_client() -> Self {
        Self {
            role: PeerRole::Client,
            os: PeerOs::Unknown,
            version: None,
        }
    }

    pub fn capabilities(self) -> CapabilitySet {
        CapabilitySet { target: self }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilitySet {
    target: PeerTarget,
}

impl CapabilitySet {
    pub fn min_ok(self) -> bool {
        match self.target.version {
            None => true,
            Some(version) => version.meets_minimum(),
        }
    }

    pub fn has_setenv_opt(self) -> bool {
        self.min_ok()
    }

    pub fn has_block_outside_dns(self) -> bool {
        matches!(self.target.role, PeerRole::Client)
            && matches!(self.target.os, PeerOs::Windows | PeerOs::Unknown)
    }

    pub fn block_outside_dns_policy(self, intent: bool) -> LinePolicy {
        if !intent {
            return LinePolicy::Skip;
        }
        match self.target.role {
            PeerRole::Server => LinePolicy::Skip,
            PeerRole::Client if self.has_block_outside_dns() && self.has_setenv_opt() => {
                LinePolicy::OptionalWrap
            }
            PeerRole::Client => LinePolicy::Skip,
        }
    }

    pub fn require_minimum(self) -> Result<()> {
        if let Some(version) = self.target.version {
            if !version.meets_minimum() {
                bail!(
                    "OpenVPN {version} is below {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}. tls-crypt and dh none require OpenVPN {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}+."
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LinePolicy {
    Emit,
    OptionalWrap,
    Skip,
    Refuse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionSource {
    Flag,
    Preview,
    Baseline,
}

impl fmt::Display for VersionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag => write!(f, "--openvpn-version"),
            Self::Preview => write!(f, "preview"),
            Self::Baseline => write!(f, "baseline {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStamp {
    pub tunsmith: String,
    pub server_target: PeerTarget,
    pub client_target: PeerTarget,
    pub dialect: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_host: Option<String>,
    pub version_source: VersionSource,
    pub warnings: Vec<String>,
}

impl BuildStamp {
    pub fn new(
        server_target: PeerTarget,
        client_target: PeerTarget,
        source_host: Option<String>,
        version_source: VersionSource,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            tunsmith: PROJECT_VERSION.to_string(),
            server_target,
            client_target,
            dialect: DIALECT_OPENVPN_2_4.to_string(),
            source_host,
            version_source,
            warnings,
        }
    }

    pub fn legacy_openvpn_2_4() -> Self {
        Self::new(
            PeerTarget::linux_server(Some(OpenVpnVersion::baseline())),
            PeerTarget::mixed_client(),
            None,
            VersionSource::Baseline,
            vec!["dist/build.json missing; assumed openvpn-2.4 dialect".to_string()],
        )
    }
}

pub fn parse_openvpn_version(raw: &str) -> Option<OpenVpnVersion> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(OPENVPN_NOT_INSTALLED) {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            if let Some(version) = parse_dotted_at(trimmed, index) {
                return Some(version);
            }
        }
        index += 1;
    }
    None
}

pub fn parse_cli_openvpn_version(raw: &str) -> Result<OpenVpnVersion> {
    let trimmed = raw.trim();
    let version = parse_exact_version(trimmed).ok_or_else(|| {
        anyhow::anyhow!(
            "invalid --openvpn-version {trimmed:?}; expected X.Y or X.Y.Z (OpenVPN {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}+)"
        )
    })?;
    if !version.meets_minimum() {
        bail!(
            "OpenVPN {version} is below {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}. tls-crypt and dh none require OpenVPN {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}+."
        );
    }
    Ok(version)
}

pub fn can_run_on(stamp: &BuildStamp, live: Option<OpenVpnVersion>) -> Result<()> {
    let Some(live) = live else {
        bail!(
            "OpenVPN is not installed on the remote host. Install OpenVPN {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}+, then run \"{APP_NAME} preview ssh\" and \"{APP_NAME} build\"."
        );
    };
    if !live.meets_minimum() {
        bail!(
            "Remote OpenVPN {live} is below {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}. tls-crypt and dh none require OpenVPN {MIN_OPENVPN_MAJOR}.{MIN_OPENVPN_MINOR}+. Upgrade the host, then run \"{APP_NAME} preview ssh\" and \"{APP_NAME} build\"."
        );
    }
    if stamp.dialect == DIALECT_OPENVPN_2_4 {
        return Ok(());
    }
    bail!(
        "Build dialect {} cannot run on OpenVPN {live}. Run \"{APP_NAME} preview ssh\" then \"{APP_NAME} build\" and retry.",
        stamp.dialect
    );
}

pub fn load_build_stamp(path: &Path) -> Result<Option<BuildStamp>> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

fn parse_exact_version(raw: &str) -> Option<OpenVpnVersion> {
    let version = parse_dotted_at(raw, 0)?;
    let expected = match raw.bytes().filter(|b| *b == b'.').count() {
        1 => format!("{}.{}", version.major, version.minor),
        2 => format!("{}.{}.{}", version.major, version.minor, version.patch),
        _ => return None,
    };
    if raw == expected {
        Some(version)
    } else {
        None
    }
}

fn parse_dotted_at(input: &str, start: usize) -> Option<OpenVpnVersion> {
    let rest = &input[start..];
    let bytes = rest.as_bytes();
    let mut index = 0;
    let mut parts = [0u16; 3];
    let mut count = 0usize;
    loop {
        let begin = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == begin {
            return None;
        }
        parts[count] = rest[begin..index].parse().ok()?;
        count += 1;
        if count == 3 {
            break;
        }
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            continue;
        }
        break;
    }
    if count < 2 {
        return None;
    }
    Some(OpenVpnVersion {
        major: parts[0],
        minor: parts[1],
        patch: if count == 3 { parts[2] } else { 0 },
    })
}
