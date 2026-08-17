use anyhow::Result;

use crate::cli::{PreviewCommands, SshArgs};
use crate::commands::connect::{open_session, prepare_ssh};
use crate::project::{now_rfc3339, save_remote_profile, RemoteProfile};
use crate::remote::VpnManager;

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
    println!("Analyzing remote environment...");
    let info = manager.analyze_environment(&prepared.target.host).await?;

    println!("Connected to {}", info.hostname);
    println!("\n--- System Overview ---");
    println!("OS:             {}", info.os);
    println!("Kernel:         {}", info.kernel);
    println!("Uptime:         {}", info.uptime);
    println!("CPU:            {}", info.cpu);
    println!("RAM:            {}", info.ram);
    println!("Disk (/):       {}", info.disk);
    println!("\n--- Networking ---");
    println!("Public IP:      {}", info.public_ip);
    println!("Local IP:       {}", info.local_ip);
    println!(
        "IP Forwarding:  {}",
        if info.is_forwarding_enabled {
            "Enabled"
        } else {
            "Disabled"
        }
    );
    println!("\n--- VPN Status ---");
    println!("OpenVPN:        {}", info.vpn_version);

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
    println!("\nProfile updated: remotes/{}.json", profile.host);

    Ok(Some((profile, RemoteSessionHandle { session })))
}

pub struct RemoteSessionHandle {
    pub session: crate::ssh::RemoteSession,
}

pub async fn connect_and_inspect(args: SshArgs) -> Result<(RemoteProfile, crate::ssh::RemoteSession)> {
    let result = preview_ssh(args).await?;
    let (profile, handle) = result.ok_or_else(|| anyhow::anyhow!("SSH discovery failed"))?;
    Ok((profile, handle.session))
}
