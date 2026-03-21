use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use anneal_core::{UserRole, UserStatus};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub owner_user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    #[sqlx(default)]
    pub tenant_name: Option<String>,
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub totp_confirmed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateUserCommand {
    pub target_tenant_id: Option<Uuid>,
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct CreateResellerCommand {
    pub tenant_name: String,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct UpdateUserCommand {
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub password_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateResellerCommand {
    pub tenant_name: String,
    pub email: String,
    pub display_name: String,
    pub status: UserStatus,
    pub password_hash: Option<String>,
}
