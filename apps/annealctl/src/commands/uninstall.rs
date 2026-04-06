use anyhow::Result;

use crate::{
    cli::UninstallArgs,
    config::{DeploymentMode, InstallConfig, InstallLayout, InstallRole},
    system::System,
};

pub async fn run(layout: InstallLayout, args: UninstallArgs) -> Result<()> {
    let config = InstallConfig::load(&layout.config_path)?;
    let system = System::new();
    system.require_root()?;
    match config.deployment_mode {
        DeploymentMode::Native => {
            if config.role.includes_control_plane() {
                system.disable_conflicting_caddy_services()?;
                system.disable_and_stop([
                    "anneal-api.service",
                    "anneal-worker.service",
                    "anneal-caddy.service",
                ])?;
                for unit in [
                    "anneal-api.service",
                    "anneal-worker.service",
                    "anneal-caddy.service",
                ] {
                    system.remove_path(&layout.systemd_dir.join(unit))?;
                }
                if !args.keep_database
                    && let Some(control_plane) = config.control_plane.as_ref()
                {
                    system.drop_local_database(&control_plane.database_url)?;
                }
            }
            if config.role.includes_node() {
                system.disable_and_stop([
                    "anneal-node-agent.service",
                    "anneal-xray.service",
                    "anneal-singbox.service",
                ])?;
                for unit in [
                    "anneal-node-agent.service",
                    "anneal-xray.service",
                    "anneal-singbox.service",
                ] {
                    system.remove_path(&layout.systemd_dir.join(unit))?;
                }
            }
            system.daemon_reload()?;
        }
        DeploymentMode::Docker => {
            if config.role.includes_control_plane() {
                let stack_root = layout.docker_stack_root(InstallRole::ControlPlane);
                let env_file = stack_root.join(".env");
                let _ = system.docker_compose_down(&stack_root, &env_file);
                system.remove_path(&stack_root)?;
            }
            if config.role.includes_node() {
                let stack_root = layout.docker_stack_root(InstallRole::Node);
                let env_file = stack_root.join(".env");
                let _ = system.docker_compose_down(&stack_root, &env_file);
                system.remove_path(&stack_root)?;
            }
        }
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
