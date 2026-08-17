use anyhow::{anyhow, Result};
use dialoguer::Input;

use crate::cli::SshArgs;
use crate::constants::DEFAULT_SSH_USER;
use crate::project::{
    load_remote_profile, AUTH_TYPE_KEY, AUTH_TYPE_PASSWORD, RemoteProfile,
};
use crate::ssh::{
    default_ssh_key_path, expand_tilde, parse_ssh_host, prompt_password, RemoteSession, SshAuth,
    SshTarget,
};

pub struct PreparedSsh {
    pub target: SshTarget,
    pub auth: SshAuth,
    pub existing_profile: Option<RemoteProfile>,
}

pub fn prepare_ssh(args: &SshArgs, prompt_if_missing_host: bool) -> Result<PreparedSsh> {
    let (mut host, mut user) = parse_ssh_host(args.host.as_deref(), &args.user)?;
    if host.is_empty() {
        if !prompt_if_missing_host {
            anyhow::bail!("SSH host is required");
        }
        host = Input::new()
            .with_prompt("Remote server IP/Domain")
            .interact_text()?;
        if args.host.is_none() {
            user = Input::new()
                .with_prompt("SSH user")
                .default(DEFAULT_SSH_USER.to_string())
                .interact_text()?;
        }
    }

    let existing_profile = load_remote_profile(&host)?;
    if existing_profile.is_some() {
        println!("Found existing profile for {host}.");
    }

    let password_auth = args.password
        || existing_profile
            .as_ref()
            .and_then(|p| p.ssh_auth_type.as_deref())
            == Some(AUTH_TYPE_PASSWORD)
            && args.key.is_none();

    let auth = if password_auth {
        SshAuth {
            auth_type: AUTH_TYPE_PASSWORD.to_string(),
            password: Some(prompt_password()?),
            key_path: None,
        }
    } else {
        let key_path = args
            .key
            .clone()
            .or_else(|| {
                existing_profile
                    .as_ref()
                    .and_then(|p| p.ssh_key_path.as_ref().map(|p| expand_tilde(p)))
            })
            .or_else(default_ssh_key_path)
            .ok_or_else(|| anyhow!("no SSH private key found; pass --key or --password"))?;
        let key_path = if key_path.starts_with("~") {
            expand_tilde(&key_path.to_string_lossy())
        } else {
            key_path
        };
        SshAuth {
            auth_type: AUTH_TYPE_KEY.to_string(),
            password: None,
            key_path: Some(key_path),
        }
    };

    Ok(PreparedSsh {
        target: SshTarget {
            host,
            user,
            port: args.port,
        },
        auth,
        existing_profile,
    })
}

pub async fn open_session(prepared: &PreparedSsh) -> Result<RemoteSession> {
    println!("Connecting to {}...", prepared.target.host);
    RemoteSession::open(&prepared.target, &prepared.auth).await
}
