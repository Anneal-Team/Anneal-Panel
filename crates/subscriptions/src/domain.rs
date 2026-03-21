use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use anneal_core::QuotaState;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Device {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub device_token: String,
    pub suspended: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Subscription {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub name: String,
    pub note: Option<String>,
    pub access_key: String,
    pub traffic_limit_bytes: i64,
    pub used_bytes: i64,
    pub quota_state: QuotaState,
    pub suspended: bool,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[sqlx(default)]
    pub current_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct SubscriptionLink {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub token: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateDeviceCommand {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CreateSubscriptionCommand {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub note: Option<String>,
    pub traffic_limit_bytes: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpdateSubscriptionCommand {
    pub name: String,
    pub note: Option<String>,
    pub traffic_limit_bytes: i64,
    pub expires_at: DateTime<Utc>,
    pub suspended: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedSubscriptionContext {
    pub subscription: Subscription,
    pub link: SubscriptionLink,
}

#[derive(Debug, Clone)]
pub struct RenderedSubscriptionBundle {
    pub content: String,
    pub links_count: usize,
    pub content_type: String,
}
