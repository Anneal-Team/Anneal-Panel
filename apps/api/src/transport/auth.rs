use axum::{Json, extract::State, http::HeaderMap};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use utoipa::ToSchema;

use anneal_auth::{LoginResult, SessionContext};

use crate::{
    app_state::AppState,
    error::ApiError,
    extractors::{authenticated_actor, pre_auth_claims},
    transport::users::UserResponse,
};

const AUTH_FAILURE_LIMIT: i32 = 5;
const AUTH_FAILURE_WINDOW_MINUTES: i64 = 15;
const AUTH_LOCKOUT_MINUTES: i64 = 15;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct BootstrapRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DisableTotpRequest {
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[utoipa::path(post, path = "/api/v1/bootstrap", request_body = BootstrapRequest)]
pub async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BootstrapRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    let provided_token = headers
        .get("x-bootstrap-token")
        .and_then(|value| value.to_str().ok());
    match (state.settings.bootstrap_token.as_deref(), provided_token) {
        (Some(expected), Some(actual)) if actual == expected => {}
        _ => return Err(ApiError(anneal_core::ApplicationError::Forbidden)),
    }
    let password_hash = state
        .auth_service()
        .hash_password(&request.password)
        .await
        .map_err(ApiError)?;
    let user = state
        .user_service()
        .bootstrap_superadmin(request.email, request.display_name, password_hash)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(user.id),
            user.tenant_id,
            "auth.bootstrap",
            "user",
            Some(user.id),
            json!({ "email": user.email }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(user.into()))
}

#[utoipa::path(post, path = "/api/v1/auth/login", request_body = LoginRequest)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResult>, ApiError> {
    let login_scope = login_scope(&request.email);
    ensure_auth_scope_available(&state.pool, &login_scope)
        .await
        .map_err(ApiError)?;
    let session_context = SessionContext {
        user_agent: headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        ip_address: None,
    };
    let response = state
        .auth_service()
        .login(
            &request.email,
            &request.password,
            request.totp_code.as_deref(),
            session_context,
        )
        .await
        .map_err(ApiError);
    let response = match response {
        Ok(response) => {
            clear_auth_failures(&state.pool, &login_scope)
                .await
                .map_err(ApiError)?;
            response
        }
        Err(ApiError(anneal_core::ApplicationError::Unauthorized)) => {
            let locked = record_auth_failure(&state.pool, &login_scope)
                .await
                .map_err(ApiError)?;
            write_auth_failure_audit(&state, None, None, "login", &login_scope)
                .await
                .map_err(ApiError)?;
            if locked {
                write_auth_lockout_audit(&state, None, None, "login", &login_scope)
                    .await
                    .map_err(ApiError)?;
            }
            return Err(ApiError(anneal_core::ApplicationError::Unauthorized));
        }
        Err(error) => return Err(error),
    };
    write_login_audit(&state, &response)
        .await
        .map_err(ApiError)?;
    Ok(Json(response))
}

#[utoipa::path(post, path = "/api/v1/auth/refresh", request_body = RefreshRequest)]
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<anneal_auth::SessionTokens>, ApiError> {
    let response = state
        .auth_service()
        .refresh(
            &request.refresh_token,
            SessionContext {
                user_agent: headers
                    .get("user-agent")
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned),
                ip_address: None,
            },
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(response))
}

#[utoipa::path(post, path = "/api/v1/auth/logout", request_body = RefreshRequest)]
pub async fn logout(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .auth_service()
        .logout(&request.refresh_token)
        .await
        .map_err(ApiError)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[utoipa::path(post, path = "/api/v1/auth/totp/setup")]
pub async fn begin_totp_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<anneal_auth::TotpSetup>, ApiError> {
    let claims = pre_auth_claims(&headers, &state).map_err(ApiError)?;
    let setup = state
        .auth_service()
        .begin_totp_setup(&claims)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(claims.sub),
            claims.tenant_id,
            "auth.totp.setup",
            "user",
            Some(claims.sub),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(setup))
}

#[utoipa::path(post, path = "/api/v1/auth/totp/verify", request_body = TotpVerifyRequest)]
pub async fn verify_totp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TotpVerifyRequest>,
) -> Result<Json<anneal_auth::SessionTokens>, ApiError> {
    let claims = pre_auth_claims(&headers, &state).map_err(ApiError)?;
    let totp_scope = totp_scope(claims.sub);
    ensure_auth_scope_available(&state.pool, &totp_scope)
        .await
        .map_err(ApiError)?;
    let tokens = state
        .auth_service()
        .verify_totp(
            &claims,
            &request.code,
            SessionContext {
                user_agent: headers
                    .get("user-agent")
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned),
                ip_address: None,
            },
        )
        .await
        .map_err(ApiError);
    let tokens = match tokens {
        Ok(tokens) => {
            clear_auth_failures(&state.pool, &totp_scope)
                .await
                .map_err(ApiError)?;
            tokens
        }
        Err(ApiError(anneal_core::ApplicationError::Unauthorized)) => {
            let locked = record_auth_failure(&state.pool, &totp_scope)
                .await
                .map_err(ApiError)?;
            write_auth_failure_audit(
                &state,
                Some(claims.sub),
                claims.tenant_id,
                "totp_verify",
                &totp_scope,
            )
            .await
            .map_err(ApiError)?;
            if locked {
                write_auth_lockout_audit(
                    &state,
                    Some(claims.sub),
                    claims.tenant_id,
                    "totp_verify",
                    &totp_scope,
                )
                .await
                .map_err(ApiError)?;
            }
            return Err(ApiError(anneal_core::ApplicationError::Unauthorized));
        }
        Err(error) => return Err(error),
    };
    state
        .audit_service()
        .write(
            Some(claims.sub),
            claims.tenant_id,
            "auth.totp.verify",
            "user",
            Some(claims.sub),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(tokens))
}

#[utoipa::path(post, path = "/api/v1/auth/totp/disable", request_body = DisableTotpRequest)]
pub async fn disable_totp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DisableTotpRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    state
        .auth_service()
        .disable_totp(&actor, &request.password)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            actor.tenant_id,
            "auth.totp.disable",
            "user",
            Some(actor.user_id),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(post, path = "/api/v1/auth/logout-all")]
pub async fn logout_all(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    state
        .auth_service()
        .logout_all(&actor)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            actor.tenant_id,
            "auth.logout_all",
            "session",
            None,
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(post, path = "/api/v1/auth/password", request_body = ChangePasswordRequest)]
pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    state
        .auth_service()
        .change_password(&actor, &request.current_password, &request.new_password)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            actor.tenant_id,
            "auth.password.change",
            "user",
            Some(actor.user_id),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(get, path = "/api/v1/auth/sessions")]
pub async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_auth::RefreshSession>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let sessions = state
        .auth_service()
        .list_sessions(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(sessions))
}

async fn write_login_audit(
    state: &AppState,
    response: &LoginResult,
) -> anneal_core::ApplicationResult<()> {
    let claims = match response {
        LoginResult::Authenticated { tokens } => {
            state.auth_service().decode_claims(&tokens.access_token)?
        }
        LoginResult::TotpRequired { pre_auth_token }
        | LoginResult::TotpSetupRequired { pre_auth_token } => {
            state.auth_service().decode_claims(pre_auth_token)?
        }
    };
    state
        .audit_service()
        .write(
            Some(claims.sub),
            claims.tenant_id,
            login_action(response),
            "session",
            None,
            json!({ "kind": claims.kind }),
        )
        .await?;
    Ok(())
}

fn login_action(response: &LoginResult) -> &'static str {
    match response {
        LoginResult::Authenticated { .. } => "auth.login",
        LoginResult::TotpRequired { .. } => "auth.login.totp_required",
        LoginResult::TotpSetupRequired { .. } => "auth.login.totp_setup_required",
    }
}

fn login_scope(email: &str) -> String {
    format!("login:{}", email.trim().to_ascii_lowercase())
}

fn totp_scope(user_id: uuid::Uuid) -> String {
    format!("totp:{user_id}")
}

async fn ensure_auth_scope_available(
    pool: &PgPool,
    scope: &str,
) -> anneal_core::ApplicationResult<()> {
    let locked_until = sqlx::query_scalar::<_, Option<chrono::DateTime<Utc>>>(
        "select locked_until from auth_rate_limits where scope = $1",
    )
    .bind(scope)
    .fetch_optional(pool)
    .await
    .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?
    .flatten();
    if locked_until.is_some_and(|value| value > Utc::now()) {
        return Err(anneal_core::ApplicationError::Unauthorized);
    }
    Ok(())
}

async fn clear_auth_failures(pool: &PgPool, scope: &str) -> anneal_core::ApplicationResult<()> {
    sqlx::query("delete from auth_rate_limits where scope = $1")
        .bind(scope)
        .execute(pool)
        .await
        .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
    Ok(())
}

async fn record_auth_failure(pool: &PgPool, scope: &str) -> anneal_core::ApplicationResult<bool> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
    let row = sqlx::query_as::<_, (i32, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
        "select failures, last_failed_at, locked_until from auth_rate_limits where scope = $1 for update",
    )
    .bind(scope)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
    let now = Utc::now();
    let window_start = now - Duration::minutes(AUTH_FAILURE_WINDOW_MINUTES);
    let locked = match row {
        Some((failures, last_failed_at, locked_until)) => {
            let next_failures =
                if locked_until.is_some_and(|value| value > now) || last_failed_at < window_start {
                    1
                } else {
                    failures + 1
                };
            let next_locked_until = (next_failures >= AUTH_FAILURE_LIMIT)
                .then_some(now + Duration::minutes(AUTH_LOCKOUT_MINUTES));
            sqlx::query(
                r#"
                update auth_rate_limits
                set failures = $2, first_failed_at = case when $3 then $4 else first_failed_at end,
                    last_failed_at = $4, locked_until = $5
                where scope = $1
                "#,
            )
            .bind(scope)
            .bind(next_failures)
            .bind(next_failures == 1)
            .bind(now)
            .bind(next_locked_until)
            .execute(&mut *transaction)
            .await
            .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
            next_locked_until.is_some()
        }
        None => {
            sqlx::query(
                r#"
                insert into auth_rate_limits (scope, failures, first_failed_at, last_failed_at, locked_until)
                values ($1, 1, $2, $2, null)
                "#,
            )
            .bind(scope)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
            false
        }
    };
    transaction
        .commit()
        .await
        .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
    Ok(locked)
}

async fn write_auth_failure_audit(
    state: &AppState,
    actor_user_id: Option<uuid::Uuid>,
    tenant_id: Option<uuid::Uuid>,
    stage: &str,
    scope: &str,
) -> anneal_core::ApplicationResult<()> {
    state
        .audit_service()
        .write(
            actor_user_id,
            tenant_id,
            "auth.failure",
            "auth",
            actor_user_id,
            json!({ "stage": stage, "scope": scope }),
        )
        .await
        .map(|_| ())
}

async fn write_auth_lockout_audit(
    state: &AppState,
    actor_user_id: Option<uuid::Uuid>,
    tenant_id: Option<uuid::Uuid>,
    stage: &str,
    scope: &str,
) -> anneal_core::ApplicationResult<()> {
    state
        .audit_service()
        .write(
            actor_user_id,
            tenant_id,
            "auth.lockout",
            "auth",
            actor_user_id,
            json!({
                "stage": stage,
                "scope": scope,
                "limit": AUTH_FAILURE_LIMIT,
                "window_minutes": AUTH_FAILURE_WINDOW_MINUTES,
                "lockout_minutes": AUTH_LOCKOUT_MINUTES
            }),
        )
        .await
        .map(|_| ())
}
