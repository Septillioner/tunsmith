use anyhow::{bail, Result};
use dialoguer::{Confirm, Select};
use std::fs;
use std::io::IsTerminal;

use crate::cli::{BuildArgs, CleanCommands, RemoteCommands, SshArgs, SshTransport};
use crate::commands::build;
use crate::commands::preview::connect_and_inspect;
use crate::constants::{APP_NAME, REMOTE_OPENVPN_SERVER_DIR, SERVER_CONF_NAME};
use crate::ovpn_target::{can_run_on, load_build_stamp, parse_openvpn_version, BuildStamp};
use crate::project::{
    dist_build_stamp_path, dist_dir, dist_server_dir, list_remote_hosts, load_remote_profile,
    require_config, save_remote_profile, RemoteSetup, SETUP_STATUS_DEPLOYED,
};
use crate::remote::{deployed_at, validate_iface, validate_ipv4_cidr, VpnManager};
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
    let nat_flag = args.nat_interface.clone();
    let cfg = require_config()?;
    let (mut profile, session) = connect_and_inspect(args).await?;
    let manager = VpnManager::new(&session);

    style::step(style::STAGE_DEPS, "Ensuring dependencies...");
    manager
        .ensure_dependencies(|msg| style::detail(msg))
        .await?;
    style::success("Dependencies satisfied.");

    let live = manager.vpn_version().await;
    profile.vpn_version = Some(live.clone());
    save_remote_profile(&profile)?;
    build::run(BuildArgs {
        host: Some(profile.host.clone()),
        openvpn_version: None,
    })?;

    let mut pending_nat_iface = None;
    if cfg.server.redirect_gateway {
        style::step(style::STAGE_NET, "Checking IP forwarding...");
        if manager.check_ip_forwarding().await {
            style::detail("IP forwarding already enabled.");
        } else {
            style::step(style::STAGE_NET, "Enabling IP forwarding...");
            manager.enable_ip_forwarding().await?;
            style::success("IP forwarding enabled.");
        }
        pending_nat_iface =
            confirm_gateway_nat(&manager, &cfg.server.subnet, nat_flag.as_deref()).await?;
    }

    let local_dist = dist_server_dir();
    if !local_dist.is_dir() {
        bail!("Build did not write dist/server.");
    }

    require_deploy_compatible(&live)?;

    style::step(style::STAGE_DEPLOY, "Deploying configurations...");
    manager
        .setup_vpn(&cfg.instance_name, &local_dist, |msg| style::detail(msg))
        .await?;
    style::success("Deployment complete.");

    let nat_interface = if let Some(iface) = pending_nat_iface {
        apply_confirmed_nat(&manager, &cfg.instance_name, &cfg.server.subnet, &iface).await?;
        Some(iface)
    } else {
        None
    };

    profile.setup = Some(RemoteSetup {
        instance_name: cfg.instance_name.clone(),
        config_dir: format!("{REMOTE_OPENVPN_SERVER_DIR}/{}", cfg.instance_name),
        main_config_path: format!("{REMOTE_OPENVPN_SERVER_DIR}/{}.conf", cfg.instance_name),
        deployed_at: deployed_at(),
        status: SETUP_STATUS_DEPLOYED.to_string(),
        nat_interface,
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

    Ok(())
}

async fn update(args: SshArgs) -> Result<()> {
    let cfg = require_config()?;
    let nat_flag = args.nat_interface.clone();
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

    let (mut profile, session) = connect_and_inspect(args).await?;
    let manager = VpnManager::new(&session);
    let live = manager.vpn_version().await;
    require_deploy_compatible(&live)?;
    style::step(style::STAGE_UPDATE, "Updating server configuration...");
    manager
        .update_config(&cfg.instance_name, &local_config, |msg| style::detail(msg))
        .await?;
    if cfg.server.redirect_gateway {
        if let Some(iface) =
            confirm_gateway_nat(&manager, &cfg.server.subnet, nat_flag.as_deref()).await?
        {
            apply_confirmed_nat(&manager, &cfg.instance_name, &cfg.server.subnet, &iface).await?;
            if let Some(setup) = profile.setup.as_mut() {
                setup.nat_interface = Some(iface);
                save_remote_profile(&profile)?;
            }
        }
    }
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

async fn confirm_gateway_nat(
    manager: &VpnManager<'_>,
    subnet: &str,
    flag_iface: Option<&str>,
) -> Result<Option<String>> {
    validate_ipv4_cidr(subnet)?;
    let detected = manager.default_ipv4_ifaces().await;
    let iface = resolve_nat_iface(&detected, flag_iface)?;
    style::warn(nat_uppercase_warning(&iface, subnet));
    if !stdin_is_tty() {
        style::warn("No TTY; NAT not applied. Re-run remote setup on a terminal to confirm.");
        return Ok(None);
    }
    let ok = Confirm::with_theme(&style::theme())
        .with_prompt(format!("Apply NAT masquerade on {iface} for {subnet}?"))
        .default(false)
        .interact()?;
    if !ok {
        style::warn(
            "NAT skipped. Full-tunnel clients will not reach the internet until NAT is applied.",
        );
        return Ok(None);
    }
    Ok(Some(iface))
}

async fn apply_confirmed_nat(
    manager: &VpnManager<'_>,
    instance_name: &str,
    subnet: &str,
    iface: &str,
) -> Result<()> {
    if manager.ufw_is_active().await {
        style::warn(
            "UFW is active. DEFAULT_FORWARD_POLICY=DROP may still block forwarded packets.",
        );
    }
    style::step(style::STAGE_NET, format!("Applying NAT on {iface}..."));
    manager
        .apply_gateway_nat(instance_name, subnet, iface, |msg| style::detail(msg))
        .await?;
    style::success(format!("NAT masquerade on {iface} for {subnet}."));
    Ok(())
}

fn resolve_nat_iface(detected: &[String], flag: Option<&str>) -> Result<String> {
    if let Some(flag) = flag {
        validate_iface(flag)?;
        if detected.is_empty() || detected.iter().any(|iface| iface == flag) {
            return Ok(flag.to_string());
        }
        bail!(
            "Interface {flag} is not a default IPv4 route device. Detected: {}.",
            detected.join(", ")
        );
    }
    match detected {
        [] => bail!("No default IPv4 route found. Specify --nat-interface."),
        [one] => {
            validate_iface(one)?;
            Ok(one.clone())
        }
        many => {
            style::info(format!(
                "Multiple default IPv4 interfaces: {}",
                many.join(", ")
            ));
            if !stdin_is_tty() {
                bail!(
                    "Multiple default IPv4 interfaces ({}). Specify --nat-interface.",
                    many.join(", ")
                );
            }
            let index = Select::with_theme(&style::theme())
                .with_prompt("Select NAT egress interface")
                .items(many)
                .default(0)
                .interact()?;
            let chosen = &many[index];
            validate_iface(chosen)?;
            Ok(chosen.clone())
        }
    }
}

fn nat_uppercase_warning(iface: &str, subnet: &str) -> String {
    format!(
        "WARNING: ABOUT TO SNAT VPN CLIENTS ON {} FOR {}. THIS MASQUERADES ALL FULL-TUNNEL TRAFFIC OUT THAT INTERFACE.",
        iface.to_ascii_uppercase(),
        subnet.to_ascii_uppercase()
    )
}

fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}
