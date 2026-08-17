use anyhow::Result;

use crate::cli::{ConfigCommands, ConfigSetArgs};
use crate::constants::{APP_DISPLAY_NAME, MAX_PORT, MIN_PORT};
use crate::project::{require_config, save_config};

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
                    println!("Warning: \"{ip}\" does not look like a valid IPv4 address. Skipping.");
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
    println!("Configuration updated successfully.");
    println!("{}", serde_json::to_string_pretty(&cfg.server)?);
    Ok(())
}

fn show() -> Result<()> {
    let cfg = require_config()?;
    println!("\n--- {APP_DISPLAY_NAME} Project Configuration ---");
    println!("Instance Name:  {}", cfg.instance_name);
    println!("Version:        {}", cfg.version);
    println!(
        "Template:       {}",
        cfg.template.as_deref().unwrap_or("None")
    );
    println!("Created At:     {}", cfg.created_at);

    println!("\n--- Server Settings ---");
    let host = if cfg.server.host.is_empty() {
        "Not set".to_string()
    } else {
        cfg.server.host.clone()
    };
    println!("Host:           {host}");
    println!("Port:           {}", cfg.server.port);
    println!("Protocol:       {}", cfg.server.protocol);
    println!("Subnet:         {}", cfg.server.subnet);
    println!("Device:         {}", cfg.server.device);
    println!(
        "Redirect GW:    {}",
        enabled(cfg.server.redirect_gateway)
    );
    println!(
        "Client-to-Client: {}",
        enabled(cfg.server.client_to_client.unwrap_or(false))
    );
    println!(
        "Block Out. DNS: {}",
        enabled(cfg.server.block_outside_dns.unwrap_or(false))
    );
    println!(
        "DNS Servers:    {}",
        cfg.server
            .dns
            .as_ref()
            .map(|d| d.join(", "))
            .unwrap_or_else(|| "Default".to_string())
    );
    println!("Cipher:         {}", cfg.server.cipher);
    println!("Auth:           {}", cfg.server.auth);
    println!("Keepalive:      {}", cfg.server.keepalive);
    println!("Verbosity:      {}", cfg.server.verb);

    println!("\n--- Clients ---");
    if cfg.clients.is_empty() {
        println!("No clients added yet.");
    } else {
        for client in &cfg.clients {
            println!(" - {}", client.name);
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
    parts.iter().all(|p| {
        p.parse::<u8>().is_ok()
    })
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
