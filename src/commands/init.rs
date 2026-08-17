use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::cli::InitArgs;
use crate::constants::{CONFIG_FILE, DEFAULT_INSTANCE_NAME};
use crate::pki::{generate_root_ca, save_ca, Subject};
use crate::project::{
    ensure_dir, pki_dir, pki_exists, save_config, validate_instance_name, ServerConfig,
    TunsmithConfig,
};
use crate::style;
use crate::templates::{find_template, template_names};

pub fn run(args: InitArgs) -> Result<()> {
    if pki_exists() && !args.force {
        style::warn("PKI already initialized. Use --force to re-initialize.");
        return Ok(());
    }

    let folder_name = env::current_dir()?
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| DEFAULT_INSTANCE_NAME.to_string());
    let instance_name = args.name.clone().unwrap_or(folder_name);
    validate_instance_name(&instance_name)?;

    let mut server = ServerConfig::default();
    let mut template_name = args.template.clone();

    if let Some(name) = &args.template {
        let template = find_template(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown template: {name}\nAvailable templates: {}",
                template_names()
            )
        })?;
        server = template.server;
        template_name = Some(template.name.to_string());
    }

    if let Some(schema_path) = &args.schema {
        apply_schema(&mut server, &mut template_name, schema_path)?;
    }

    if args.force && pki_dir().exists() {
        fs::remove_dir_all(pki_dir())?;
    }
    ensure_dir(&pki_dir())?;

    let validity = args.validity;
    style::step(
        style::STAGE_PKI,
        format!("Generating {validity}-year RSA-4096 root CA (this can take a few seconds)..."),
    );
    let org = args.org.clone().unwrap_or_else(|| instance_name.clone());
    let ca = generate_root_ca(
        &Subject {
            common_name: format!("{instance_name} Root CA"),
            organization: Some(org),
            country: args.country.clone(),
        },
        validity,
    )?;
    save_ca(&ca)?;

    let cfg = TunsmithConfig::new(instance_name.clone(), template_name.clone(), server);
    save_config(&cfg)?;

    let template_note = template_name
        .map(|t| format!(" with {t} template"))
        .unwrap_or_default();
    style::success(format!("initialized {instance_name}{template_note}"));
    style::detail(format!("Config saved to: {CONFIG_FILE}"));
    style::detail("CA certificate saved to: pki/ca.crt");
    style::warn("CA private key saved to: pki/ca.key (unencrypted)");
    Ok(())
}

fn apply_schema(
    server: &mut ServerConfig,
    template_name: &mut Option<String>,
    path: &PathBuf,
) -> Result<()> {
    let raw = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;

    if let Some(t) = value.get("template").and_then(|v| v.as_str()) {
        *template_name = Some(t.to_string());
        if let Some(found) = find_template(t) {
            *server = found.server;
        }
    }

    let server_value = value.get("server").cloned().unwrap_or(value);
    merge_server(server, &server_value)?;
    Ok(())
}

fn merge_server(server: &mut ServerConfig, value: &serde_json::Value) -> Result<()> {
    if !value.is_object() {
        bail!("schema file must contain a JSON object");
    }
    if let Some(v) = value.get("host").and_then(|v| v.as_str()) {
        server.host = v.to_string();
    }
    if let Some(v) = value.get("port").and_then(|v| v.as_u64()) {
        server.port = v as u16;
    }
    if let Some(v) = value
        .get("protocol")
        .or(value.get("proto"))
        .and_then(|v| v.as_str())
    {
        server.protocol = v.to_string();
    }
    if let Some(v) = value.get("subnet").and_then(|v| v.as_str()) {
        server.subnet = v.to_string();
    }
    if let Some(v) = value
        .get("device")
        .or(value.get("dev"))
        .and_then(|v| v.as_str())
    {
        server.device = v.to_string();
    }
    if let Some(v) = value
        .get("redirect_gateway")
        .or(value.get("redirectGateway"))
        .and_then(|v| v.as_bool())
    {
        server.redirect_gateway = v;
    }
    if let Some(v) = value.get("cipher").and_then(|v| v.as_str()) {
        server.cipher = v.to_string();
    }
    if let Some(v) = value.get("auth").and_then(|v| v.as_str()) {
        server.auth = v.to_string();
    }
    if let Some(v) = value.get("keepalive").and_then(|v| v.as_str()) {
        server.keepalive = v.to_string();
    }
    if let Some(v) = value.get("verb").and_then(|v| v.as_u64()) {
        server.verb = v as u8;
    }
    if let Some(v) = value.get("dns").and_then(|v| v.as_array()) {
        server.dns = Some(
            v.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect(),
        );
    }
    if let Some(v) = value
        .get("client_to_client")
        .or(value.get("clientToClient"))
        .and_then(|v| v.as_bool())
    {
        server.client_to_client = Some(v);
    }
    if let Some(v) = value
        .get("block_outside_dns")
        .or(value.get("blockOutsideDNS"))
        .and_then(|v| v.as_bool())
    {
        server.block_outside_dns = Some(v);
    }
    Ok(())
}
