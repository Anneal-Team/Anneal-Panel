use anyhow::{Result, anyhow};

use crate::{
    config::{DeploymentMode, InstallConfig, InstallLayout},
    state::InstallState,
    system::System,
};

pub async fn run(layout: InstallLayout) -> Result<()> {
    let config = InstallConfig::load(&layout.config_path)?;
    let state = InstallState::load(&layout.state_path)?
        .ok_or_else(|| anyhow!("install state not found"))?;
    let system = System::new();

    println!("role={:?}", config.role);
    println!("mode={:?}", config.deployment_mode);
    if let Some(version) = config.release_version.as_ref() {
        println!("version={version}");
    }
    println!("config_path={}", layout.config_path.display());
    println!("state_path={}", layout.state_path.display());
    for (step, step_state) in state.steps {
        println!("step.{step:?}={:?}", step_state.status);
    }
    match config.deployment_mode {
        DeploymentMode::Native => {
            for service in [
                "postgresql",
                "anneal-api.service",
                "anneal-worker.service",
                "anneal-caddy.service",
                "anneal-node-agent.service",
                "anneal-xray.service",
                "anneal-singbox.service",
            ] {
                if let Ok(status) = system.service_status(service) {
                    println!("service.{service}={status}");
                }
            }
        }
        DeploymentMode::Docker => {
            println!(
                "docker.control_plane_root={}",
                layout
                    .docker_stack_root(crate::config::InstallRole::ControlPlane)
                    .display()
            );
            println!(
                "docker.node_root={}",
                layout
                    .docker_stack_root(crate::config::InstallRole::Node)
                    .display()
            );
        }
    }
    Ok(())
}
