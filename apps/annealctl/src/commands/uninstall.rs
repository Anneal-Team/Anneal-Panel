use anyhow::Result;

use crate::{
    cli::UninstallArgs,
    config::{InstallConfig, InstallLayout},
    system::System,
};

pub async fn run(layout: InstallLayout, args: UninstallArgs) -> Result<()> {
    let config = InstallConfig::load(&layout.config_path)?;
    let system = System::new();
    system.require_root()?;
    if config.role.includes_control_plane() {
        system.disable_conflicting_caddy_services()?;
        system.disable_and_stop([
            "anneal-api.service",
            "anneal-worker.service",
            "anneal-caddy.service",
            "anneal-mihomo.service",
        ])?;
        for unit in [
            "anneal-api.service",
            "anneal-worker.service",
            "anneal-caddy.service",
            "anneal-mihomo.service",
        ] {
            system.remove_path(&layout.systemd_dir.join(unit))?;
        }
        if !args.keep_database {
            system.drop_local_database(&config.control_plane.database_url)?;
        }
        system.daemon_reload()?;
    }
    system.remove_path(&layout.utility_path)?;
    system.remove_path(&layout.env_path)?;
    system.remove_path(&layout.summary_path)?;
    system.remove_path(&layout.config_path)?;
    system.remove_path(&layout.state_path)?;
    system.remove_path(&layout.caddyfile_path)?;
    system.remove_path(&layout.install_root)?;
    if !args.keep_data {
        system.remove_path(&layout.data_root)?;
    }
    Ok(())
}
