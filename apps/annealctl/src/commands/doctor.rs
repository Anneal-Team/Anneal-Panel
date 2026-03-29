use anyhow::{Result, bail};

use crate::{
    config::{DeploymentMode, InstallConfig, InstallLayout},
    system::System,
};

pub async fn run(layout: InstallLayout) -> Result<()> {
    let config = InstallConfig::load(&layout.config_path)?;
    let system = System::new();
    let mut issues = Vec::new();

    if !layout.env_path.exists() {
        issues.push(format!("missing {}", layout.env_path.display()));
    }
    if config.role.includes_control_plane() && !layout.summary_path.exists() {
        issues.push(format!("missing {}", layout.summary_path.display()));
    }
    match config.deployment_mode {
        DeploymentMode::Native => {
            if config.role.includes_control_plane() {
                for service in [
                    "anneal-api.service",
                    "anneal-worker.service",
                    "anneal-caddy.service",
                ] {
                    match system.service_status(service) {
                        Ok(status) if status == "active" => {}
                        Ok(status) => issues.push(format!("{service} is {status}")),
                        Err(error) => issues.push(format!("{service}: {error}")),
                    }
                }
            }
            if config.role.includes_node() {
                match system.service_status("anneal-node-agent.service") {
                    Ok(status) if status == "active" => {}
                    Ok(status) => issues.push(format!("anneal-node-agent.service is {status}")),
                    Err(error) => issues.push(format!("anneal-node-agent.service: {error}")),
                }
            }
        }
        DeploymentMode::Docker => {
            if config.role.includes_control_plane()
                && !layout
                    .docker_stack_root(crate::config::InstallRole::ControlPlane)
                    .join("compose.yml")
                    .exists()
            {
                issues.push("missing docker control-plane compose.yml".into());
            }
            if config.role.includes_node()
                && !layout
                    .docker_stack_root(crate::config::InstallRole::Node)
                    .join("compose.yml")
                    .exists()
            {
                issues.push("missing docker node compose.yml".into());
            }
        }
    }

    if issues.is_empty() {
        println!("doctor=ok");
        return Ok(());
    }

    for issue in &issues {
        println!("doctor.issue={issue}");
    }
    bail!("doctor found {} issue(s)", issues.len())
}
