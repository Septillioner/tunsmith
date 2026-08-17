use anyhow::{bail, Result};
use std::fs;

use crate::cli::BuildArgs;
use crate::constants::{
    APP_NAME, CA_CERT_FILE, DEFAULT_CLIENT_VALIDITY_YEARS, DEFAULT_SERVER_VALIDITY_YEARS,
    DIALECT_OPENVPN_2_4, SERVER_CERT_FILE, SERVER_CONF_NAME, SERVER_KEY_FILE, TLS_CRYPT_FILE,
};
use crate::ovpn::{
    build_client_config, build_server_config, generate_tls_crypt_key, ClientBundle, VpnCrypto,
    VpnEndpoints, VpnNetwork, VpnPkiPaths,
};
use crate::ovpn_target::{
    parse_cli_openvpn_version, parse_openvpn_version, BuildStamp, OpenVpnVersion, PeerTarget,
    VersionSource,
};
use crate::pki::{issue_client_cert, issue_server_cert, load_ca};
use crate::project::{
    ca_cert_path, client_cert_path, client_key_path, dist_build_stamp_path, dist_clients_dir,
    dist_dir, dist_server_dir, ensure_dir, list_remote_hosts, load_remote_profile, pki_exists,
    require_config, server_cert_path, server_key_path, server_pki_dir, tls_crypt_path,
    write_public_file, write_secret_file, RemoteProfile,
};
use crate::style;

struct ResolvedBuildTarget {
    server: PeerTarget,
    client: PeerTarget,
    source_host: Option<String>,
    version_source: VersionSource,
    warnings: Vec<String>,
}

pub fn run(args: BuildArgs) -> Result<()> {
    if !pki_exists() {
        bail!("Project not initialized. Run \"{APP_NAME} init\" first.");
    }
    let cfg = require_config()?;
    if cfg.server.host.is_empty() {
        bail!("Server not configured. Run \"{APP_NAME} config set --host <host>\" first.");
    }

    let resolved = resolve_server_target(&args)?;
    print_build_target(&resolved);

    style::step(style::STAGE_BUILD, "Building configurations...");
    let ca = load_ca()?;

    if dist_dir().exists() {
        fs::remove_dir_all(dist_dir())?;
    }
    ensure_dir(&dist_server_dir())?;
    ensure_dir(&dist_clients_dir())?;
    ensure_dir(&server_pki_dir())?;

    if !server_cert_path().is_file() {
        style::step(style::STAGE_CERTS, "Issuing server certificate...");
        let leaf = issue_server_cert(
            &ca,
            &cfg.instance_name,
            &cfg.server.host,
            DEFAULT_SERVER_VALIDITY_YEARS,
        )?;
        write_public_file(&server_cert_path(), &leaf.cert_pem)?;
        write_secret_file(&server_key_path(), &leaf.key_pem)?;
    }

    if !tls_crypt_path().is_file() {
        write_secret_file(&tls_crypt_path(), &generate_tls_crypt_key())?;
    }

    let endpoints = VpnEndpoints {
        host: cfg.server.host.clone(),
        port: cfg.server.port,
        protocol: cfg.server.protocol.clone(),
        device: cfg.server.device.clone(),
    };
    let crypto = VpnCrypto {
        cipher: cfg.server.cipher.clone(),
        auth: cfg.server.auth.clone(),
    };
    let network = VpnNetwork {
        subnet: cfg.server.subnet.clone(),
        redirect_gateway: cfg.server.redirect_gateway,
        client_to_client: cfg.server.client_to_client.unwrap_or(false),
        dns: cfg
            .server
            .dns
            .clone()
            .unwrap_or_else(crate::constants::default_dns),
        block_outside_dns: cfg.server.block_outside_dns.unwrap_or(false),
    };
    let pki_paths = VpnPkiPaths {
        ca: format!("{}/{}", cfg.instance_name, CA_CERT_FILE),
        cert: format!("{}/{}", cfg.instance_name, SERVER_CERT_FILE),
        key: format!("{}/{}", cfg.instance_name, SERVER_KEY_FILE),
        tls_crypt: format!("{}/{}", cfg.instance_name, TLS_CRYPT_FILE),
    };

    let server_conf = build_server_config(
        &endpoints,
        &crypto,
        &network,
        &pki_paths,
        &cfg.server.keepalive,
        cfg.server.verb,
        &resolved.server,
    )?;
    write_public_file(&dist_server_dir().join(SERVER_CONF_NAME), &server_conf)?;
    fs::copy(ca_cert_path(), dist_server_dir().join(CA_CERT_FILE))?;
    fs::copy(server_cert_path(), dist_server_dir().join(SERVER_CERT_FILE))?;
    write_secret_file(
        &dist_server_dir().join(SERVER_KEY_FILE),
        &fs::read_to_string(server_key_path())?,
    )?;
    write_secret_file(
        &dist_server_dir().join(TLS_CRYPT_FILE),
        &fs::read_to_string(tls_crypt_path())?,
    )?;

    let ca_cert = fs::read_to_string(ca_cert_path())?;
    let tls_crypt_key = fs::read_to_string(tls_crypt_path())?;

    for client in &cfg.clients {
        ensure_dir(&crate::project::client_pki_dir(&client.name))?;
        let cert_path = client_cert_path(&client.name);
        let key_path = client_key_path(&client.name);
        let (cert_pem, key_pem) = if cert_path.is_file() {
            (
                fs::read_to_string(&cert_path)?,
                fs::read_to_string(&key_path)?,
            )
        } else {
            style::step(
                style::STAGE_CERTS,
                format!("Issuing client certificate for {}...", client.name),
            );
            let leaf = issue_client_cert(&ca, &client.name, DEFAULT_CLIENT_VALIDITY_YEARS)?;
            write_public_file(&cert_path, &leaf.cert_pem)?;
            write_secret_file(&key_path, &leaf.key_pem)?;
            (leaf.cert_pem, leaf.key_pem)
        };

        let ovpn = build_client_config(
            &endpoints,
            &crypto,
            &ClientBundle {
                ca_cert: ca_cert.clone(),
                client_cert: cert_pem,
                client_key: key_pem,
                tls_crypt_key: tls_crypt_key.clone(),
            },
            cfg.server.verb,
            network.block_outside_dns,
            &resolved.client,
        );
        write_secret_file(
            &dist_clients_dir().join(format!("{}.ovpn", client.name)),
            &ovpn,
        )?;
    }

    let stamp = BuildStamp::new(
        resolved.server,
        resolved.client,
        resolved.source_host,
        resolved.version_source,
        resolved.warnings,
    );
    let stamp_json = serde_json::to_string_pretty(&stamp)?;
    write_public_file(&dist_build_stamp_path(), &(stamp_json + "\n"))?;

    style::success("Build completed. Files are in the \"dist\" folder.");
    Ok(())
}

fn resolve_server_target(args: &BuildArgs) -> Result<ResolvedBuildTarget> {
    let mut warnings = Vec::new();
    let profile = load_selected_profile(args)?;
    let source_host = profile.as_ref().map(|item| item.host.clone());

    let (version, version_source) = if let Some(flag) = &args.openvpn_version {
        let version = parse_cli_openvpn_version(flag)?;
        if let Some(profile) = &profile {
            if let Some(preview) = profile
                .vpn_version
                .as_deref()
                .and_then(parse_openvpn_version)
            {
                if preview != version {
                    style::warn(format!(
                        "OpenVPN version {version} from --openvpn-version overrides preview {preview} on {}.",
                        profile.host
                    ));
                }
            }
        }
        (Some(version), VersionSource::Flag)
    } else if let Some(profile) = &profile {
        match profile
            .vpn_version
            .as_deref()
            .and_then(parse_openvpn_version)
        {
            Some(version) if version.meets_minimum() => (Some(version), VersionSource::Preview),
            Some(version) => {
                bail!(
                    "Preview OpenVPN {version} on {} is below 2.4. Upgrade the host or pass --openvpn-version 2.4 or newer.",
                    profile.host
                );
            }
            None => {
                let message = format!(
                    "OpenVPN version missing or not installed on {}; assuming 2.4. remote setup will apt-install on Debian/Ubuntu.",
                    profile.host
                );
                warnings.push(message.clone());
                style::warn(&message);
                (Some(OpenVpnVersion::baseline()), VersionSource::Baseline)
            }
        }
    } else {
        let message =
            "No remote preview; assuming OpenVPN 2.4 on Linux. Run preview ssh to target a host."
                .to_string();
        warnings.push(message.clone());
        style::warn(&message);
        (Some(OpenVpnVersion::baseline()), VersionSource::Baseline)
    };

    Ok(ResolvedBuildTarget {
        server: PeerTarget::linux_server(version),
        client: PeerTarget::mixed_client(),
        source_host,
        version_source,
        warnings,
    })
}

fn load_selected_profile(args: &BuildArgs) -> Result<Option<RemoteProfile>> {
    if let Some(host) = &args.host {
        let profile = load_remote_profile(host)?.ok_or_else(|| {
            anyhow::anyhow!("No profile found for {host}. Run \"{APP_NAME} preview ssh\" first.")
        })?;
        return Ok(Some(profile));
    }

    let hosts = list_remote_hosts()?;
    match hosts.as_slice() {
        [] => Ok(None),
        [host] => load_remote_profile(host),
        _ => bail!("Multiple remotes found. Specify --host."),
    }
}

fn print_build_target(resolved: &ResolvedBuildTarget) {
    let version = resolved
        .server
        .version
        .map(|item| item.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let host = resolved
        .source_host
        .as_deref()
        .map(|host| format!("remotes/{host}.json"))
        .unwrap_or_else(|| "no preview host".to_string());
    style::key_field(
        "Target",
        format!("OpenVPN {version} (linux server, {host})"),
    );
    style::key_field("Version source", resolved.version_source);
    style::key_field("Dialect", DIALECT_OPENVPN_2_4);
    style::field(
        "Client profiles",
        "mixed 2.4+ (optional directives wrapped)",
    );
}
