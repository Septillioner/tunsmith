mod cli;
mod commands;
mod constants;
mod ovpn;
mod ovpn_target;
mod pki;
mod project;
mod remote;
mod ssh;
mod style;
mod templates;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run().await
}
