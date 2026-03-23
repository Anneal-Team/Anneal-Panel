use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use anneal_core::UserRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResult {
    Authenticated { tokens: SessionTokens },
    TotpRequired { pre_auth_token: String },
    TotpSetupRequired { pre_auth_token: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotpSetup {
    pub secret: String,
    pub otpauth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RefreshSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub rotated_from_session_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: Uuid,
    pub role: UserRole,
    pub tenant_id: Option<Uuid>,
    pub kind: String,
    pub challenge_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreAuthPurpose {
    TotpSetup,
    TotpVerify,
}

impl PreAuthPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TotpSetup => "totp_setup",
            Self::TotpVerify => "totp_verify",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PreAuthChallenge {
    pub id: Uuid,
    pub user_id: Uuid,
    pub purpose: String,
    pub pending_totp_secret: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}
