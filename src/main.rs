mod cli;
mod commands;
mod constants;
mod ovpn;
mod pki;
mod project;
mod remote;
mod ssh;
mod templates;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run().await
}
