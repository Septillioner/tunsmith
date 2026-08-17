use anyhow::{bail, Result};
use dialoguer::Confirm;
use std::fs;

use crate::cli::{CleanCommands, RemoteCommands, SshArgs, SshTransport};
use crate::commands::preview::connect_and_inspect;
use crate::constants::{APP_NAME, REMOTE_OPENVPN_SERVER_DIR, SERVER_CONF_NAME};
use crate::ovpn_target::{can_run_on, load_build_stamp, parse_openvpn_version, BuildStamp};
use crate::project::{
    dist_build_stamp_path, dist_dir, dist_server_dir, list_remote_hosts, load_remote_profile,
    require_config, save_remote_profile, RemoteSetup, SETUP_STATUS_DEPLOYED,
};
use crate::remote::{deployed_at, VpnManager};
use crate::style;

pub async fn run(cmd: RemoteCommands) -> Result<()> {
    match cmd {
        RemoteCommands::Setup(SshTransport::Ssh(args)) => setup(args).await,
        RemoteCommands::Update(SshTransport::Ssh(args)) => update(args).await,
        RemoteCommands::Clean(CleanCommands::Ssh(args)) => clean_ssh(args).await,
        RemoteCommands::Clean(CleanCommands::Local) => clean_local(),
    }
}

async fn setup(args: SshArgs) -> Result<()> {
    let cfg = require_config()?;
    let (mut profile, session) = connect_and_inspect(args).await?;
    let manager = VpnManager::new(&session);

    style::step(style::STAGE_DEPS, "Ensuring dependencies...");
    manager
        .ensure_dependencies(|msg| style::detail(msg))
        .await?;
    style::success("Dependencies satisfied.");

    if cfg.server.redirect_gateway {
        style::step(style::STAGE_NET, "Checking IP forwarding...");
        if manager.check_ip_forwarding().await {
            style::detail("IP forwarding already enabled.");
        } else {
            style::step(style::STAGE_NET, "Enabling IP forwarding...");
            manager.enable_ip_forwarding().await?;
            style::success("IP forwarding enabled.");
        }
    }

    let local_dist = dist_server_dir();
    if !local_dist.is_dir() {
        bail!("Build files not found. Run \"{APP_NAME} build\" first.");
    }

    let live = manager.vpn_version().await;
    require_deploy_compatible(&live)?;

    style::step(style::STAGE_DEPLOY, "Deploying configurations...");
    manager
        .setup_vpn(&cfg.instance_name, &local_dist, |msg| style::detail(msg))
        .await?;
    style::success("Deployment complete.");

    profile.setup = Some(RemoteSetup {
        instance_name: cfg.instance_name.clone(),
        config_dir: format!("{REMOTE_OPENVPN_SERVER_DIR}/{}", cfg.instance_name),
        main_config_path: format!("{REMOTE_OPENVPN_SERVER_DIR}/{}.conf", cfg.instance_name),
        deployed_at: deployed_at(),
        status: SETUP_STATUS_DEPLOYED.to_string(),
    });
    save_remote_profile(&profile)?;
    style::info(format!(
        "Profile updated with setup details: remotes/{}.json",
        profile.host
    ));
    style::success(format!(
        "VPN server \"{}\" is now live on {}.",
        cfg.instance_name, profile.host
    ));

    if cfg.server.redirect_gateway {
        style::warn("ACTION REQUIRED: Internet access (NAT)");
        style::detail("redirect_gateway is enabled. Configure NAT on the server, for example:");
        style::detail(format!(
            "iptables -t nat -A POSTROUTING -s {} -o eth0 -j MASQUERADE",
            cfg.server.subnet
        ));
        style::detail("Change 'eth0' to the public network interface name (ens3, eth1, ...).");
    }

    Ok(())
}

async fn update(args: SshArgs) -> Result<()> {
    let cfg = require_config()?;
    let mut args = args;
    if args.host.is_none() {
        let hosts = list_remote_hosts()?;
        if hosts.is_empty() {
            bail!("No remote profiles found. Run \"{APP_NAME} remote setup ssh\" first.");
        }
        args.host = Some(hosts[0].clone());
        style::info(format!("Auto-selected remote: {}", hosts[0]));
    }
    let host = args.host.clone().unwrap();
    if load_remote_profile(&host)?.is_none() {
        bail!("No profile found for {host}. Run \"{APP_NAME} remote setup ssh\" first.");
    }

    let local_config = dist_server_dir().join(SERVER_CONF_NAME);
    if !local_config.is_file() {
        bail!("Build files not found. Run \"{APP_NAME} build\" first.");
    }

    let (profile, session) = connect_and_inspect(args).await?;
    let manager = VpnManager::new(&session);
    let live = manager.vpn_version().await;
    require_deploy_compatible(&live)?;
    style::step(style::STAGE_UPDATE, "Updating server configuration...");
    manager
        .update_config(&cfg.instance_name, &local_config, |msg| style::detail(msg))
        .await?;
    style::success(format!(
        "Configuration updated and service restarted on {}.",
        profile.host
    ));
    Ok(())
}

async fn clean_ssh(args: SshArgs) -> Result<()> {
    let cfg = require_config()?;
    let prepared_host = args.host.clone();
    let (mut host, _) = crate::ssh::parse_ssh_host(prepared_host.as_deref(), &args.user)?;
    if host.is_empty() {
        host = dialoguer::Input::with_theme(&style::theme())
            .with_prompt("Remote server IP/Domain")
            .interact_text()?;
    }

    let existing = load_remote_profile(&host)?;
    let instance = existing
        .as_ref()
        .and_then(|p| p.setup.as_ref().map(|s| s.instance_name.clone()))
        .unwrap_or(cfg.instance_name.clone());

    let confirm = Confirm::with_theme(&style::theme())
        .with_prompt(format!("Remove instance \"{instance}\" from {host}?"))
        .default(false)
        .interact()?;
    if !confirm {
        return Ok(());
    }

    let mut args = args;
    if args.host.is_none() {
        args.host = Some(host.clone());
    }
    let (mut profile, session) = connect_and_inspect(args).await?;
    let manager = VpnManager::new(&session);
    style::step(
        style::STAGE_CLEAN,
        format!("Removing instance \"{instance}\" from {host}..."),
    );
    manager
        .cleanup_vpn(&instance, |msg| style::detail(msg))
        .await?;
    profile.setup = None;
    save_remote_profile(&profile)?;
    style::success(format!("Instance \"{instance}\" removed from {host}."));
    Ok(())
}

fn clean_local() -> Result<()> {
    if dist_dir().exists() {
        fs::remove_dir_all(dist_dir())?;
        style::success("Local build files cleaned.");
    } else {
        style::info("Dist folder does not exist.");
    }
    Ok(())
}

fn require_deploy_compatible(live_raw: &str) -> Result<()> {
    let stamp = match load_build_stamp(&dist_build_stamp_path())? {
        Some(stamp) => stamp,
        None => {
            style::warn("dist/build.json is missing. Treating dialect as openvpn-2.4.");
            BuildStamp::legacy_openvpn_2_4()
        }
    };
    can_run_on(&stamp, parse_openvpn_version(live_raw))
}
