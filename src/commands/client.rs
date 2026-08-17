use anyhow::Result;

use crate::cli::ClientCommands;
use crate::constants::APP_NAME;
use crate::project::{require_config, save_config, ClientEntry};
use crate::style;

pub fn run(cmd: ClientCommands) -> Result<()> {
    match cmd {
        ClientCommands::Add { name } => add(&name),
        ClientCommands::Remove { name } => remove(&name),
        ClientCommands::List => list(),
    }
}

fn add(name: &str) -> Result<()> {
    let mut cfg = require_config()?;
    if cfg.clients.iter().any(|c| c.name == name) {
        style::warn(format!("Client \"{name}\" already exists."));
        return Ok(());
    }
    cfg.clients.push(ClientEntry {
        name: name.to_string(),
    });
    save_config(&cfg)?;
    style::success(format!(
        "Client \"{name}\" added. Run \"{APP_NAME} build\" to generate profile."
    ));
    Ok(())
}

fn remove(name: &str) -> Result<()> {
    let mut cfg = require_config()?;
    let before = cfg.clients.len();
    cfg.clients.retain(|c| c.name != name);
    if cfg.clients.len() == before {
        style::error(format!("Client \"{name}\" not found."));
        return Ok(());
    }
    save_config(&cfg)?;
    style::warn(format!(
        "Client \"{name}\" removed from the project list. The certificate is not revoked."
    ));
    Ok(())
}

fn list() -> Result<()> {
    let cfg = require_config()?;
    if cfg.clients.is_empty() {
        style::info("No clients added yet.");
    } else {
        style::heading("Clients");
        for client in &cfg.clients {
            style::detail(&client.name);
        }
    }
    Ok(())
}
