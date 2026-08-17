use anyhow::Result;

use crate::cli::{ConfigCommands, ConfigSetArgs};
use crate::constants::{APP_DISPLAY_NAME, MAX_PORT, MIN_PORT};
use crate::project::{require_config, save_config};
use crate::style;

pub fn run(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Set(args) => set(args),
        ConfigCommands::Show => show(),
    }
}

fn set(args: ConfigSetArgs) -> Result<()> {
    let mut cfg = require_config()?;

    if let Some(host) = args.host {
        cfg.server.host = host;
    }
    if let Some(port) = args.port {
        if port < MIN_PORT {
            anyhow::bail!("port must be between {MIN_PORT} and {MAX_PORT}");
        }
        cfg.server.port = port;
    }
    if let Some(proto) = args.proto {
        cfg.server.protocol = proto;
    }
    if let Some(subnet) = args.subnet {
        cfg.server.subnet = subnet;
    }
    if let Some(dev) = args.dev {
        cfg.server.device = dev;
    }
    if let Some(cipher) = args.cipher {
        cfg.server.cipher = cipher;
    }
    if let Some(auth) = args.auth {
        cfg.server.auth = auth;
    }
    if let Some(keepalive) = args.keepalive {
        cfg.server.keepalive = keepalive;
    }
    if let Some(verb) = args.verb {
        cfg.server.verb = verb;
    }
    if let Some(dns) = args.dns {
        let parsed: Vec<String> = dns
            .split(|c: char| c == ',' || c == ' ' || c == ';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|ip| {
                if looks_like_ipv4(ip) {
                    Some(ip.to_string())
                } else {
                    style::warn(format!(
                        "\"{ip}\" does not look like a valid IPv4 address. Skipping."
                    ));
                    None
                }
            })
            .collect();
        if !parsed.is_empty() {
            cfg.server.dns = Some(parsed);
        }
    }
    if args.redirect {
        cfg.server.redirect_gateway = true;
    }
    if args.c2c {
        cfg.server.client_to_client = Some(true);
    }
    if args.block_dns {
        cfg.server.block_outside_dns = Some(true);
    }
    if args.allow_dns {
        cfg.server.block_outside_dns = Some(false);
    }

    save_config(&cfg)?;
    style::success("Configuration updated successfully.");
    println!("{}", serde_json::to_string_pretty(&cfg.server)?);
    Ok(())
}

fn show() -> Result<()> {
    let cfg = require_config()?;
    style::heading(&format!("{APP_DISPLAY_NAME} project"));
    style::field("Instance", &cfg.instance_name);
    style::field("Version", &cfg.version);
    style::field("Template", cfg.template.as_deref().unwrap_or("None"));
    style::field("Created", &cfg.created_at);

    style::heading("Server");
    if cfg.server.host.is_empty() {
        style::field("Host", style::warn_value("Not set"));
    } else {
        style::field("Host", &cfg.server.host);
    }
    style::field("Port", cfg.server.port);
    style::field("Protocol", &cfg.server.protocol);
    style::field("Subnet", &cfg.server.subnet);
    style::field("Device", &cfg.server.device);
    style::field("Redirect GW", enabled(cfg.server.redirect_gateway));
    style::field(
        "Client-to-Client",
        enabled(cfg.server.client_to_client.unwrap_or(false)),
    );
    style::field(
        "Block Out. DNS",
        enabled(cfg.server.block_outside_dns.unwrap_or(false)),
    );
    style::field(
        "DNS Servers",
        cfg.server
            .dns
            .as_ref()
            .map(|d| d.join(", "))
            .unwrap_or_else(|| "Default".to_string()),
    );
    style::field("Cipher", &cfg.server.cipher);
    style::field("Auth", &cfg.server.auth);
    style::field("Keepalive", &cfg.server.keepalive);
    style::field("Verbosity", cfg.server.verb);

    style::heading("Clients");
    if cfg.clients.is_empty() {
        style::info("No clients added yet.");
    } else {
        for client in &cfg.clients {
            style::detail(&client.name);
        }
    }
    println!();
    Ok(())
}

fn enabled(value: bool) -> &'static str {
    if value {
        "Enabled"
    } else {
        "Disabled"
    }
}

fn looks_like_ipv4(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u8>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::looks_like_ipv4;
    use crate::constants::DEFAULT_DNS_PRIMARY;

    #[test]
    fn looks_like_ipv4_accepts_dotted_quad() {
        assert!(looks_like_ipv4(DEFAULT_DNS_PRIMARY));
        assert!(looks_like_ipv4("10.0.0.1"));
    }

    #[test]
    fn looks_like_ipv4_rejects_malformed() {
        assert!(!looks_like_ipv4("10.0.0"));
        assert!(!looks_like_ipv4("10.0.0.1.2"));
        assert!(!looks_like_ipv4("256.0.0.1"));
        assert!(!looks_like_ipv4("vpn.example.com"));
    }
}
