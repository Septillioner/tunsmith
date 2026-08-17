use anyhow::{bail, Result};
use std::fs;

use crate::constants::{
    APP_NAME, CA_CERT_FILE, DEFAULT_CLIENT_VALIDITY_YEARS, DEFAULT_SERVER_VALIDITY_YEARS,
    SERVER_CERT_FILE, SERVER_CONF_NAME, SERVER_KEY_FILE, TLS_CRYPT_FILE,
};
use crate::ovpn::{
    build_client_config, build_server_config, generate_tls_crypt_key, ClientBundle, VpnCrypto,
    VpnEndpoints, VpnNetwork, VpnPkiPaths,
};
use crate::pki::{issue_client_cert, issue_server_cert, load_ca};
use crate::project::{
    ca_cert_path, client_cert_path, client_key_path, dist_clients_dir, dist_dir, dist_server_dir,
    ensure_dir, pki_exists, require_config, server_cert_path, server_key_path, server_pki_dir,
    tls_crypt_path, write_public_file, write_secret_file,
};

pub fn run() -> Result<()> {
    if !pki_exists() {
        bail!("Project not initialized. Run \"{APP_NAME} init\" first.");
    }
    let cfg = require_config()?;
    if cfg.server.host.is_empty() {
        bail!("Server not configured. Run \"{APP_NAME} config set --host <host>\" first.");
    }

    println!("Building configurations...");
    let ca = load_ca()?;

    if dist_dir().exists() {
        fs::remove_dir_all(dist_dir())?;
    }
    ensure_dir(&dist_server_dir())?;
    ensure_dir(&dist_clients_dir())?;
    ensure_dir(&server_pki_dir())?;

    if !server_cert_path().is_file() {
        println!("Issuing server certificate...");
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
            (fs::read_to_string(&cert_path)?, fs::read_to_string(&key_path)?)
        } else {
            println!("Issuing client certificate for {}...", client.name);
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
        );
        write_secret_file(
            &dist_clients_dir().join(format!("{}.ovpn", client.name)),
            &ovpn,
        )?;
    }

    println!("Build completed. Files are in the \"dist\" folder.");
    Ok(())
}
