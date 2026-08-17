use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::{
    build, client, config, init, preview, remote, template, tui,
};
use crate::constants::{APP_NAME, DEFAULT_SSH_PORT, DEFAULT_SSH_USER};

const AFTER_HELP: &str = "\
Quick start:
  $ tunsmith init --template gateway-vpn
  $ tunsmith config set --host vpn.example.com
  $ tunsmith client add laptop
  $ tunsmith build
  $ tunsmith tui
  $ tunsmith remote setup ssh root@203.0.113.10

Documentation: README.md and docs/security.md
";

#[derive(Parser)]
#[command(
    name = APP_NAME,
    version,
    about = "Strike an OpenVPN PKI. Deploy it over SSH.",
    after_help = AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the PKI environment (Root CA)
    Init(InitArgs),
    /// Manage Tunsmith configuration
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Manage VPN clients
    #[command(subcommand)]
    Client(ClientCommands),
    /// Generate configuration files into dist/
    Build,
    /// List configuration templates
    Template,
    /// Interactive terminal menu (not a web UI)
    Tui,
    /// Discover a remote environment over SSH
    #[command(subcommand)]
    Preview(PreviewCommands),
    /// Manage remote VPN servers
    #[command(subcommand)]
    Remote(RemoteCommands),
}

#[derive(Parser)]
pub struct InitArgs {
    /// Project/instance name (defaults to folder name)
    #[arg(short, long)]
    pub name: Option<String>,
    /// Organization name
    #[arg(short, long)]
    pub org: Option<String>,
    /// Country code (2 letters). Omitted from the CA if unset.
    #[arg(short, long)]
    pub country: Option<String>,
    /// CA validity in years
    #[arg(short, long, default_value_t = crate::constants::DEFAULT_CA_VALIDITY_YEARS)]
    pub validity: u32,
    /// Configuration template (gateway-vpn, cloud-vpn, gateway-cloud-vpn)
    #[arg(short, long)]
    pub template: Option<String>,
    /// Initialize from a JSON schema/config file
    #[arg(short, long)]
    pub schema: Option<std::path::PathBuf>,
    /// Overwrite existing PKI
    #[arg(long)]
    pub force: bool,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Set VPN server configuration values
    Set(ConfigSetArgs),
    /// Show current project configuration
    Show,
}

#[derive(Parser)]
pub struct ConfigSetArgs {
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub proto: Option<String>,
    #[arg(long)]
    pub subnet: Option<String>,
    #[arg(long)]
    pub dev: Option<String>,
    #[arg(long)]
    pub redirect: bool,
    #[arg(long)]
    pub cipher: Option<String>,
    #[arg(long)]
    pub auth: Option<String>,
    #[arg(long)]
    pub keepalive: Option<String>,
    #[arg(long)]
    pub verb: Option<u8>,
    #[arg(long)]
    pub dns: Option<String>,
    #[arg(long)]
    pub c2c: bool,
    #[arg(long)]
    pub block_dns: bool,
    #[arg(long)]
    pub allow_dns: bool,
}

#[derive(Subcommand)]
pub enum ClientCommands {
    /// Add a new client to the project
    Add { name: String },
    /// Remove a client from the project (does not revoke the certificate)
    Remove { name: String },
    /// List all clients
    List,
}

#[derive(Subcommand)]
pub enum PreviewCommands {
    /// Connect via SSH to discover the remote server environment
    Ssh(SshArgs),
}

#[derive(Subcommand)]
pub enum RemoteCommands {
    /// Install and configure OpenVPN on a remote server
    #[command(subcommand)]
    Setup(SshTransport),
    /// Update remote VPN configuration
    #[command(subcommand)]
    Update(SshTransport),
    /// Clean local build files or a remote instance
    #[command(subcommand)]
    Clean(CleanCommands),
}

#[derive(Subcommand)]
pub enum SshTransport {
    Ssh(SshArgs),
}

#[derive(Subcommand)]
pub enum CleanCommands {
    /// Remove a VPN instance from a remote server
    Ssh(SshArgs),
    /// Clean local build files (dist/)
    Local,
}

#[derive(Parser)]
pub struct SshArgs {
    /// SSH host (e.g. root@203.0.113.10)
    pub host: Option<String>,
    /// SSH user
    #[arg(short, long, default_value = DEFAULT_SSH_USER)]
    pub user: String,
    /// SSH port
    #[arg(short, long, default_value_t = DEFAULT_SSH_PORT)]
    pub port: u16,
    /// SSH private key path
    #[arg(long)]
    pub key: Option<std::path::PathBuf>,
    /// Use password authentication instead of a key
    #[arg(long)]
    pub password: bool,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            None => {
                <Self as clap::CommandFactory>::command().print_help()?;
                println!();
                Ok(())
            }
            Some(Commands::Init(args)) => init::run(args),
            Some(Commands::Config(cmd)) => config::run(cmd),
            Some(Commands::Client(cmd)) => client::run(cmd),
            Some(Commands::Build) => build::run(),
            Some(Commands::Template) => template::run(),
            Some(Commands::Tui) => tui::run(),
            Some(Commands::Preview(cmd)) => preview::run(cmd).await,
            Some(Commands::Remote(cmd)) => remote::run(cmd).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{DEFAULT_CA_VALIDITY_YEARS, DEFAULT_SSH_PORT, DEFAULT_SSH_USER};
    use crate::templates::TEMPLATE_GATEWAY_VPN;
    use clap::Parser;

    #[test]
    fn parses_build_command() {
        let cli = Cli::try_parse_from(["tunsmith", "build"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Build)));
    }

    #[test]
    fn parses_tui_command() {
        let cli = Cli::try_parse_from(["tunsmith", "tui"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Tui)));
    }

    #[test]
    fn parses_init_with_template() {
        let cli = Cli::try_parse_from(["tunsmith", "init", "--template", TEMPLATE_GATEWAY_VPN])
            .unwrap();
        match cli.command {
            Some(Commands::Init(args)) => {
                assert_eq!(args.template.as_deref(), Some(TEMPLATE_GATEWAY_VPN));
                assert_eq!(args.validity, DEFAULT_CA_VALIDITY_YEARS);
                assert!(!args.force);
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn parses_client_add() {
        let cli = Cli::try_parse_from(["tunsmith", "client", "add", "laptop"]).unwrap();
        match cli.command {
            Some(Commands::Client(ClientCommands::Add { name })) => {
                assert_eq!(name, "laptop");
            }
            _ => panic!("expected Client::Add"),
        }
    }

    #[test]
    fn parses_remote_clean_local() {
        let cli = Cli::try_parse_from(["tunsmith", "remote", "clean", "local"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Remote(RemoteCommands::Clean(CleanCommands::Local)))
        ));
    }

    #[test]
    fn ssh_args_default_to_constants() {
        let cli = Cli::try_parse_from(["tunsmith", "preview", "ssh", "203.0.113.10"]).unwrap();
        match cli.command {
            Some(Commands::Preview(PreviewCommands::Ssh(args))) => {
                assert_eq!(args.host.as_deref(), Some("203.0.113.10"));
                assert_eq!(args.user, DEFAULT_SSH_USER);
                assert_eq!(args.port, DEFAULT_SSH_PORT);
            }
            _ => panic!("expected Preview::Ssh"),
        }
    }
}
