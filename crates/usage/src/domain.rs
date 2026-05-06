use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use anneal_core::QuotaState;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageSample {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub subscription_id: Uuid,
    pub device_id: Uuid,
    pub bytes_in: i64,
    pub bytes_out: i64,
    pub measured_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageBatchItem {
    pub tenant_id: Uuid,
    pub subscription_id: Uuid,
    pub device_id: Uuid,
    pub bytes_in: i64,
    pub bytes_out: i64,
    pub measured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaEnvelope {
    pub subscription_id: Uuid,
    pub traffic_limit_bytes: i64,
    pub current_used_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaDecision {
    pub used_bytes: i64,
    pub quota_state: QuotaState,
    pub suspend: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageOverview {
    pub subscription_id: Uuid,
    pub tenant_id: Uuid,
    pub subscription_name: String,
    pub device_id: Uuid,
    pub device_name: String,
    pub traffic_limit_bytes: i64,
    pub used_bytes: i64,
    pub quota_state: QuotaState,
    pub suspended: bool,
    pub updated_at: DateTime<Utc>,
}
