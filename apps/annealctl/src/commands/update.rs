use anyhow::Result;

use crate::{cli::UpdateArgs, commands::install, config::InstallLayout};

pub async fn run(layout: InstallLayout, args: UpdateArgs) -> Result<()> {
    install::update_existing(layout, args.bundle_root).await
}
