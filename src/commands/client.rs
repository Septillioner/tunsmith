use anyhow::Result;

use crate::cli::ClientCommands;
use crate::constants::APP_NAME;
use crate::project::{require_config, save_config, ClientEntry};

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
        println!("Client \"{name}\" already exists.");
        return Ok(());
    }
    cfg.clients.push(ClientEntry {
        name: name.to_string(),
    });
    save_config(&cfg)?;
    println!("Client \"{name}\" added. Run \"{APP_NAME} build\" to generate profile.");
    Ok(())
}

fn remove(name: &str) -> Result<()> {
    let mut cfg = require_config()?;
    let before = cfg.clients.len();
    cfg.clients.retain(|c| c.name != name);
    if cfg.clients.len() == before {
        println!("Client \"{name}\" not found.");
        return Ok(());
    }
    save_config(&cfg)?;
    println!("Client \"{name}\" removed from the project list. The certificate is not revoked.");
    Ok(())
}

fn list() -> Result<()> {
    let cfg = require_config()?;
    if cfg.clients.is_empty() {
        println!("No clients added yet.");
    } else {
        println!("Clients:");
        for client in &cfg.clients {
            println!(" - {}", client.name);
        }
    }
    Ok(())
}
