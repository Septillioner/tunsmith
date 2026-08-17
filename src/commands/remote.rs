use anyhow::{bail, Result};
use dialoguer::Confirm;
use std::fs;

use crate::cli::{CleanCommands, RemoteCommands, SshArgs, SshTransport};
use crate::commands::preview::connect_and_inspect;
use crate::constants::{APP_NAME, REMOTE_OPENVPN_SERVER_DIR, SERVER_CONF_NAME};
use crate::project::{
    dist_dir, dist_server_dir, list_remote_hosts, load_remote_profile, require_config,
    save_remote_profile, RemoteSetup, SETUP_STATUS_DEPLOYED,
};
use crate::remote::{deployed_at, VpnManager};

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

    println!("Ensuring dependencies...");
    manager
        .ensure_dependencies(|msg| println!("{msg}"))
        .await?;
    println!("Dependencies satisfied.");

    if cfg.server.redirect_gateway {
        println!("Checking IP forwarding...");
        if manager.check_ip_forwarding().await {
            println!("IP forwarding already enabled.");
        } else {
            println!("Enabling IP forwarding...");
            manager.enable_ip_forwarding().await?;
            println!("IP forwarding enabled.");
        }
    }

    let local_dist = dist_server_dir();
    if !local_dist.is_dir() {
        bail!("Build files not found. Run \"{APP_NAME} build\" first.");
    }

    println!("Deploying configurations...");
    manager
        .setup_vpn(&cfg.instance_name, &local_dist, |msg| println!("{msg}"))
        .await?;
    println!("Deployment complete.");

    profile.setup = Some(RemoteSetup {
        instance_name: cfg.instance_name.clone(),
        config_dir: format!("{REMOTE_OPENVPN_SERVER_DIR}/{}", cfg.instance_name),
        main_config_path: format!("{REMOTE_OPENVPN_SERVER_DIR}/{}.conf", cfg.instance_name),
        deployed_at: deployed_at(),
        status: SETUP_STATUS_DEPLOYED.to_string(),
    });
    save_remote_profile(&profile)?;
    println!(
        "Profile updated with setup details: remotes/{}.json",
        profile.host
    );
    println!(
        "\nVPN server \"{}\" is now live on {}.",
        cfg.instance_name, profile.host
    );

    if cfg.server.redirect_gateway {
        println!("\nACTION REQUIRED: Internet access (NAT)");
        println!("redirect_gateway is enabled. Configure NAT on the server, for example:");
        println!(
            "  iptables -t nat -A POSTROUTING -s {} -o eth0 -j MASQUERADE",
            cfg.server.subnet
        );
        println!("Change 'eth0' to the public network interface name (ens3, eth1, ...).");
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
        println!("Auto-selected remote: {}", hosts[0]);
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
    println!("Updating server configuration...");
    manager
        .update_config(&cfg.instance_name, &local_config, |msg| println!("{msg}"))
        .await?;
    println!(
        "Configuration updated and service restarted on {}.",
        profile.host
    );
    Ok(())
}

async fn clean_ssh(args: SshArgs) -> Result<()> {
    let cfg = require_config()?;
    let prepared_host = args.host.clone();
    let (mut host, _) = crate::ssh::parse_ssh_host(prepared_host.as_deref(), &args.user)?;
    if host.is_empty() {
        host = dialoguer::Input::new()
            .with_prompt("Remote server IP/Domain")
            .interact_text()?;
    }

    let existing = load_remote_profile(&host)?;
    let instance = existing
        .as_ref()
        .and_then(|p| p.setup.as_ref().map(|s| s.instance_name.clone()))
        .unwrap_or(cfg.instance_name.clone());

    let confirm = Confirm::new()
        .with_prompt(format!(
            "Remove instance \"{instance}\" from {host}?"
        ))
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
    manager
        .cleanup_vpn(&instance, |msg| println!("{msg}"))
        .await?;
    profile.setup = None;
    save_remote_profile(&profile)?;
    println!("Instance \"{instance}\" removed from {host}.");
    Ok(())
}

fn clean_local() -> Result<()> {
    if dist_dir().exists() {
        fs::remove_dir_all(dist_dir())?;
        println!("Local build files cleaned.");
    } else {
        println!("Dist folder does not exist.");
    }
    Ok(())
}
