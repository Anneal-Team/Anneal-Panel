mod bootstrap;
mod cli;
mod commands;
mod config;
mod i18n;
mod release;
mod render;
mod state;
mod system;
mod ui;

use anyhow::Result;
use clap::Parser;

use crate::{
    cli::{Cli, Command},
    config::InstallLayout,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let layout = InstallLayout::default();
    match cli.command {
        Command::Install(args) => commands::install::run(layout, *args).await,
        Command::Resume(args) => commands::install::resume(layout, args).await,
        Command::Status => commands::status::run(layout).await,
        Command::Doctor => commands::doctor::run(layout).await,
        Command::Restart => commands::restart::run(layout).await,
        Command::Update(args) => commands::update::run(layout, args).await,
        Command::Uninstall(args) => commands::uninstall::run(layout, args).await,
        Command::Manage => commands::status::run(layout).await,
    }
}
