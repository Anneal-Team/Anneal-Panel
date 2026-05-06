use anyhow::{Result, bail};

use crate::{
    config::{InstallConfig, InstallLayout},
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
    if config.role.includes_control_plane() {
        for service in [
            "postgresql",
            "anneal-api.service",
            "anneal-worker.service",
            "anneal-caddy.service",
            "anneal-mihomo.service",
        ] {
            match system.service_status(service) {
                Ok(status) if status == "active" => {}
                Ok(status) => issues.push(format!("{service} is {status}")),
                Err(error) => issues.push(format!("{service}: {error}")),
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
