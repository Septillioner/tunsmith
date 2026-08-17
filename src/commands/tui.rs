use anyhow::Result;
use dialoguer::{Confirm, Input, Select};
use std::env;

use crate::cli::{ClientCommands, ConfigCommands, ConfigSetArgs, InitArgs};
use crate::commands::{build, client, config, init};
use crate::constants::{APP_DISPLAY_NAME, DEFAULT_CA_VALIDITY_YEARS, DEFAULT_INSTANCE_NAME};
use crate::project::pki_exists;
use crate::templates::all_templates;

const MENU_SEPARATOR_WIDTH: usize = 40;
const TEMPLATE_NONE: &str = "None (Default)";
const UNAVAILABLE_SUFFIX: &str = " (initialize first)";

const ACTION_STATUS: usize = 0;
const ACTION_INIT: usize = 1;
const ACTION_CONFIG: usize = 2;
const ACTION_CLIENTS: usize = 3;
const ACTION_BUILD: usize = 4;
const ACTION_EXIT: usize = 5;
const MENU_LEN: usize = 6;

pub fn run() -> Result<()> {
    println!("\n--- {APP_DISPLAY_NAME} TUI ---");
    loop {
        let initialized = pki_exists();
        let items = menu_labels(initialized);
        let index = Select::new()
            .with_prompt("What would you like to do?")
            .items(&items)
            .default(ACTION_STATUS)
            .interact()?;

        match index {
            ACTION_STATUS => config::run(ConfigCommands::Show)?,
            ACTION_INIT => handle_init(initialized)?,
            ACTION_CONFIG if initialized => handle_config()?,
            ACTION_CLIENTS if initialized => handle_clients()?,
            ACTION_BUILD if initialized => build::run()?,
            ACTION_EXIT => break,
            ACTION_CONFIG | ACTION_CLIENTS | ACTION_BUILD => {
                println!("Initialize the project first.");
            }
            _ => {}
        }

        println!("{}", "-".repeat(MENU_SEPARATOR_WIDTH));
    }
    Ok(())
}

fn menu_labels(initialized: bool) -> [String; MENU_LEN] {
    let init_label = if initialized {
        "Re-initialize project".to_string()
    } else {
        "Initialize project".to_string()
    };
    let maybe = |label: &str| {
        if initialized {
            label.to_string()
        } else {
            format!("{label}{UNAVAILABLE_SUFFIX}")
        }
    };
    [
        "Show status".to_string(),
        init_label,
        maybe("Configure server"),
        maybe("Manage clients"),
        maybe("Build configurations"),
        "Exit".to_string(),
    ]
}

fn handle_init(initialized: bool) -> Result<()> {
    let default_name = env::current_dir()?
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| DEFAULT_INSTANCE_NAME.to_string());

    let name: String = Input::new()
        .with_prompt("Project name")
        .default(default_name)
        .interact_text()?;

    let templates = all_templates();
    let mut template_labels = vec![TEMPLATE_NONE.to_string()];
    template_labels.extend(
        templates
            .iter()
            .map(|t| format!("{} ({})", t.name, t.description)),
    );
    let template_index = Select::new()
        .with_prompt("Select a template")
        .items(&template_labels)
        .default(0)
        .interact()?;
    let template = if template_index == 0 {
        None
    } else {
        Some(templates[template_index - 1].name.to_string())
    };

    let confirm = Confirm::new()
        .with_prompt("This will initialize the PKI. Continue?")
        .default(true)
        .interact()?;
    if !confirm {
        return Ok(());
    }

    let force = if initialized {
        Confirm::new()
            .with_prompt("Overwrite existing CA?")
            .default(false)
            .interact()?
    } else {
        false
    };
    if initialized && !force {
        println!("Initialization cancelled.");
        return Ok(());
    }

    init::run(InitArgs {
        name: Some(name),
        org: None,
        country: None,
        validity: DEFAULT_CA_VALIDITY_YEARS,
        template,
        schema: None,
        force,
    })
}

fn handle_config() -> Result<()> {
    let keys = ["Server host", "Port", "Subnet", "DNS servers", "Back to menu"];
    let index = Select::new()
        .with_prompt("What would you like to configure?")
        .items(&keys)
        .default(0)
        .interact()?;
    if index == keys.len() - 1 {
        return Ok(());
    }

    let value: String = Input::new()
        .with_prompt(format!("Enter new value for {}", keys[index].to_lowercase()))
        .interact_text()?;
    if value.trim().is_empty() {
        return Ok(());
    }

    let mut args = empty_set_args();
    match index {
        0 => args.host = Some(value),
        1 => {
            args.port = Some(value.parse().map_err(|_| {
                anyhow::anyhow!("port must be an integer between 1 and 65535")
            })?);
        }
        2 => args.subnet = Some(value),
        3 => args.dns = Some(value),
        _ => return Ok(()),
    }
    config::run(ConfigCommands::Set(args))
}

fn handle_clients() -> Result<()> {
    let items = ["List clients", "Add client", "Back to menu"];
    let index = Select::new()
        .with_prompt("Client management")
        .items(&items)
        .default(0)
        .interact()?;
    match index {
        0 => config::run(ConfigCommands::Show),
        1 => {
            let name: String = Input::new()
                .with_prompt("Client name")
                .interact_text()?;
            if name.trim().is_empty() {
                return Ok(());
            }
            client::run(ClientCommands::Add { name })
        }
        _ => Ok(()),
    }
}

fn empty_set_args() -> ConfigSetArgs {
    ConfigSetArgs {
        host: None,
        port: None,
        proto: None,
        subnet: None,
        dev: None,
        redirect: false,
        cipher: None,
        auth: None,
        keepalive: None,
        verb: None,
        dns: None,
        c2c: false,
        block_dns: false,
        allow_dns: false,
    }
}
