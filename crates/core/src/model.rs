use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Actor {
    pub user_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub role: UserRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Superadmin,
    Admin,
    Reseller,
    User,
}

impl UserRole {
    pub fn is_staff(self) -> bool {
        matches!(self, Self::Superadmin | Self::Admin | Self::Reseller)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "user_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "node_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "proxy_engine", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ProxyEngine {
    Xray,
    Singbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "protocol_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ProtocolKind {
    VlessReality,
    Vmess,
    Trojan,
    #[serde(rename = "shadowsocks_2022")]
    #[sqlx(rename = "shadowsocks_2022")]
    Shadowsocks2022,
    Tuic,
    Hysteria2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "deployment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Queued,
    Rendering,
    Validating,
    Ready,
    Applied,
    RolledBack,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "quota_state", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum QuotaState {
    Normal,
    Warning80,
    Warning95,
    Exhausted,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditStamp {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
