use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentJob {
    pub rollout_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationJob {
    pub event_id: Uuid,
}
