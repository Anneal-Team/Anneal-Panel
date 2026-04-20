use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{DeploymentMode, InstallRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStep {
    Prepare,
    Packages,
    Files,
    Services,
    ControlPlaneBootstrap,
    NodeBootstrap,
    StarterSubscription,
    Summary,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    pub status: StepStatus,
    pub updated_at: DateTime<Utc>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootstrapState {
    pub superadmin_totp_secret: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub node_group_id: Option<Uuid>,
    pub starter_subscription_name: Option<String>,
    pub starter_subscription_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallState {
    pub role: InstallRole,
    pub deployment_mode: DeploymentMode,
    pub release_version: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: BTreeMap<InstallStep, StepState>,
    pub bootstrap: BootstrapState,
}

impl InstallState {
    pub fn load(path: &Path) -> Result<Option<Self>> {
        match fs::read_to_string(path) {
            Ok(raw) => Ok(Some(
                serde_json::from_str(&raw).context("failed to parse install state")?,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
        }
    }

    pub fn load_or_new(
        path: &Path,
        role: InstallRole,
        deployment_mode: DeploymentMode,
    ) -> Result<Self> {
        Ok(Self::load(path)?.unwrap_or_else(|| Self {
            role,
            deployment_mode,
            release_version: None,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            steps: BTreeMap::new(),
            bootstrap: BootstrapState::default(),
        }))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let raw = serde_json::to_vec_pretty(self).context("failed to serialize install state")?;
        fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))?;
        set_owner_only_permissions(path)?;
        Ok(())
    }

    pub fn mark_running(&mut self, step: InstallStep, detail: impl Into<String>) {
        self.touch_step(step, StepStatus::Running, Some(detail.into()));
    }

    pub fn mark_completed(&mut self, step: InstallStep, detail: impl Into<String>) {
        self.touch_step(step, StepStatus::Completed, Some(detail.into()));
    }

    pub fn mark_failed(&mut self, step: InstallStep, detail: impl Into<String>) {
        self.touch_step(step, StepStatus::Failed, Some(detail.into()));
    }

    pub fn is_completed(&self, step: InstallStep) -> bool {
        self.steps
            .get(&step)
            .is_some_and(|state| state.status == StepStatus::Completed)
    }

    pub fn finish(&mut self) {
        let now = Utc::now();
        self.updated_at = now;
        self.completed_at = Some(now);
    }

    fn touch_step(&mut self, step: InstallStep, status: StepStatus, detail: Option<String>) {
        let now = Utc::now();
        self.updated_at = now;
        self.steps.insert(
            step,
            StepState {
                status,
                updated_at: now,
                detail,
            },
        );
    }
}

fn set_owner_only_permissions(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(_path, permissions)
            .with_context(|| format!("failed to chmod {}", _path.display()))?;
    }
    Ok(())
}
