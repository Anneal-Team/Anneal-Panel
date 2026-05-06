use anyhow::{Result, anyhow};

use crate::{
    config::{InstallConfig, InstallLayout},
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
    for service in [
        "postgresql",
        "anneal-api.service",
        "anneal-worker.service",
        "anneal-caddy.service",
        "anneal-mihomo.service",
    ] {
        if let Ok(status) = system.service_status(service) {
            println!("service.{service}={status}");
        }
    }
    Ok(())
}
