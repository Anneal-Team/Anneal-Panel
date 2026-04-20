use anyhow::Result;

use crate::{
    config::{DeploymentMode, InstallConfig, InstallLayout},
    system::System,
};

pub async fn run(layout: InstallLayout) -> Result<()> {
    let config = InstallConfig::load(&layout.config_path)?;
    let system = System::new();
    system.require_root()?;
    match config.deployment_mode {
        DeploymentMode::Native => {
            let mut services = Vec::new();
            if config.role.includes_control_plane() {
                services.extend([
                    "anneal-api.service",
                    "anneal-worker.service",
                    "anneal-caddy.service",
                ]);
            }
            if config.role.includes_node() {
                services.push("anneal-node-agent.service");
            }
            if !services.is_empty() {
                system.restart(services)?;
            }
        }
        DeploymentMode::Docker => {
            if config.role.includes_control_plane() {
                let stack_root = layout.docker_stack_root(crate::config::InstallRole::ControlPlane);
                system.docker_compose_restart(&stack_root, &stack_root.join(".env"))?;
            }
            if config.role.includes_node() {
                let stack_root = layout.docker_stack_root(crate::config::InstallRole::Node);
                system.docker_compose_restart(&stack_root, &stack_root.join(".env"))?;
            }
        }
    }
    Ok(())
}
