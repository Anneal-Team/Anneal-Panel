use anyhow::Result;

use crate::{
    config::{InstallConfig, InstallLayout},
    system::System,
};

pub async fn run(layout: InstallLayout) -> Result<()> {
    let config = InstallConfig::load(&layout.config_path)?;
    let system = System::new();
    system.require_root()?;
    if config.role.includes_control_plane() {
        system.restart([
            "anneal-api.service",
            "anneal-worker.service",
            "anneal-caddy.service",
            "anneal-mihomo.service",
        ])?;
    }
    Ok(())
}
