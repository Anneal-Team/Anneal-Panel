use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use anneal_auth::{LoginResult, SessionContext};

use crate::{
    app_state::AppState,
    error::ApiError,
    extractors::{authenticated_actor, pre_auth_claims},
};

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
    Json(request): Json<BootstrapRequest>,
) -> Result<Json<anneal_users::User>, ApiError> {
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
    Ok(Json(user))
}

#[utoipa::path(post, path = "/api/v1/auth/login", request_body = LoginRequest)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResult>, ApiError> {
    let session_context = SessionContext {
        user_agent: headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        ip_address: headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
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
        .map_err(ApiError)?;
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
                ip_address: headers
                    .get("x-forwarded-for")
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned),
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
                ip_address: headers
                    .get("x-forwarded-for")
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned),
            },
        )
        .await
        .map_err(ApiError)?;
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
