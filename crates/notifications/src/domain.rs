use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "notification_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    Quota80,
    Quota95,
    Quota100,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationEvent {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
