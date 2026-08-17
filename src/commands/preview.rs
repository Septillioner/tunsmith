use anyhow::Result;

use crate::cli::{PreviewCommands, SshArgs};
use crate::commands::connect::{open_session, prepare_ssh};
use crate::constants::OPENVPN_NOT_INSTALLED;
use crate::project::{now_rfc3339, save_remote_profile, RemoteProfile};
use crate::remote::VpnManager;
use crate::style;

pub async fn run(cmd: PreviewCommands) -> Result<()> {
    match cmd {
        PreviewCommands::Ssh(args) => {
            let _ = preview_ssh(args).await?;
            Ok(())
        }
    }
}

pub async fn preview_ssh(args: SshArgs) -> Result<Option<(RemoteProfile, RemoteSessionHandle)>> {
    let prepared = prepare_ssh(&args, true)?;
    let session = open_session(&prepared).await?;
    let manager = VpnManager::new(&session);
    style::step(style::STAGE_PROBE, "Analyzing remote environment...");
    let info = manager.analyze_environment(&prepared.target.host).await?;

    style::success(format!("Connected to {}", info.hostname));
    style::heading("System");
    style::field("OS", &info.os);
    style::field("Kernel", &info.kernel);
    style::field("Uptime", &info.uptime);
    style::field("CPU", &info.cpu);
    style::field("RAM", &info.ram);
    style::field("Disk (/)", &info.disk);
    style::heading("Networking");
    style::field("Public IP", &info.public_ip);
    style::field("Local IP", &info.local_ip);
    style::field(
        "IP Forwarding",
        if info.is_forwarding_enabled {
            "Enabled"
        } else {
            "Disabled"
        },
    );
    style::heading("VPN");
    if info.vpn_version == OPENVPN_NOT_INSTALLED {
        style::field("OpenVPN", style::warn_value(&info.vpn_version));
    } else {
        style::field("OpenVPN", &info.vpn_version);
    }

    let profile = RemoteProfile {
        host: prepared.target.host.clone(),
        user: prepared.target.user.clone(),
        port: prepared.target.port,
        hostname: Some(info.hostname),
        os: Some(info.os),
        vpn_version: Some(info.vpn_version),
        last_seen: now_rfc3339(),
        ssh_auth_type: Some(prepared.auth.auth_type.clone()),
        ssh_key_path: prepared
            .auth
            .key_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        setup: prepared.existing_profile.and_then(|p| p.setup),
    };
    save_remote_profile(&profile)?;
    style::success(format!("Profile updated: remotes/{}.json", profile.host));

    Ok(Some((profile, RemoteSessionHandle { session })))
}

pub struct RemoteSessionHandle {
    pub session: crate::ssh::RemoteSession,
}

pub async fn connect_and_inspect(
    args: SshArgs,
) -> Result<(RemoteProfile, crate::ssh::RemoteSession)> {
    let result = preview_ssh(args).await?;
    let (profile, handle) = result.ok_or_else(|| anyhow::anyhow!("SSH discovery failed"))?;
    Ok((profile, handle.session))
}
