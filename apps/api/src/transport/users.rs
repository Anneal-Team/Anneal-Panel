use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use anneal_core::UserRole;
use anneal_users::{CreateResellerCommand, CreateUserCommand, UpdateResellerCommand, UpdateUserCommand};

use crate::{app_state::AppState, error::ApiError, extractors::authenticated_actor};

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateUserRequest {
    pub target_tenant_id: Option<Uuid>,
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateResellerRequest {
    pub tenant_name: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateUserRequest {
    pub email: String,
    pub display_name: String,
    pub role: UserRole,
    pub status: anneal_core::UserStatus,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateResellerRequest {
    pub tenant_name: String,
    pub email: String,
    pub display_name: String,
    pub status: anneal_core::UserStatus,
    pub password: Option<String>,
}

#[utoipa::path(post, path = "/api/v1/users", request_body = CreateUserRequest)]
pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<anneal_users::User>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let password_hash = state
        .auth_service()
        .hash_password(&request.password)
        .await
        .map_err(ApiError)?;
    let user = state
        .user_service()
        .create_user(
            &actor,
            CreateUserCommand {
                target_tenant_id: request.target_tenant_id,
                email: request.email,
                display_name: request.display_name,
                role: request.role,
                password_hash,
            },
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            user.tenant_id.or(actor.tenant_id),
            "users.create",
            "user",
            Some(user.id),
            json!({ "role": user.role, "email": user.email }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(user))
}

#[utoipa::path(get, path = "/api/v1/users")]
pub async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_users::User>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let users = state
        .user_service()
        .list_users(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(users))
}

#[utoipa::path(patch, path = "/api/v1/users/{id}", request_body = UpdateUserRequest)]
pub async fn update_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<Uuid>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<anneal_users::User>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let password_hash = match request.password {
        Some(password) if !password.trim().is_empty() => Some(
            state
                .auth_service()
                .hash_password(&password)
                .await
                .map_err(ApiError)?,
        ),
        _ => None,
    };
    let user = state
        .user_service()
        .update_user(
            &actor,
            user_id,
            UpdateUserCommand {
                email: request.email,
                display_name: request.display_name,
                role: request.role,
                status: request.status,
                password_hash,
            },
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            user.tenant_id.or(actor.tenant_id),
            "users.update",
            "user",
            Some(user.id),
            json!({ "role": user.role, "status": user.status, "email": user.email }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(user))
}

#[utoipa::path(delete, path = "/api/v1/users/{id}")]
pub async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    state
        .user_service()
        .delete_user(&actor, user_id)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            actor.tenant_id,
            "users.delete",
            "user",
            Some(user_id),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(post, path = "/api/v1/resellers", request_body = CreateResellerRequest)]
pub async fn create_reseller(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateResellerRequest>,
) -> Result<Json<anneal_users::User>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let password_hash = state
        .auth_service()
        .hash_password(&request.password)
        .await
        .map_err(ApiError)?;
    let reseller = state
        .user_service()
        .create_reseller(
            &actor,
            CreateResellerCommand {
                tenant_name: request.tenant_name,
                email: request.email,
                display_name: request.display_name,
                password_hash,
            },
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            reseller.tenant_id,
            "users.reseller.create",
            "user",
            Some(reseller.id),
            json!({ "email": reseller.email }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(reseller))
}

#[utoipa::path(get, path = "/api/v1/resellers")]
pub async fn list_resellers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_users::User>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let users = state
        .user_service()
        .list_resellers(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(users))
}

#[utoipa::path(patch, path = "/api/v1/resellers/{id}", request_body = UpdateResellerRequest)]
pub async fn update_reseller(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<Uuid>,
    Json(request): Json<UpdateResellerRequest>,
) -> Result<Json<anneal_users::User>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let password_hash = match request.password {
        Some(password) if !password.trim().is_empty() => Some(
            state
                .auth_service()
                .hash_password(&password)
                .await
                .map_err(ApiError)?,
        ),
        _ => None,
    };
    let reseller = state
        .user_service()
        .update_reseller(
            &actor,
            user_id,
            UpdateResellerCommand {
                tenant_name: request.tenant_name,
                email: request.email,
                display_name: request.display_name,
                status: request.status,
                password_hash,
            },
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            reseller.tenant_id,
            "users.reseller.update",
            "user",
            Some(reseller.id),
            json!({ "email": reseller.email, "status": reseller.status }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(reseller))
}

#[utoipa::path(delete, path = "/api/v1/resellers/{id}")]
pub async fn delete_reseller(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    state
        .user_service()
        .delete_reseller(&actor, user_id)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            None,
            "users.reseller.delete",
            "user",
            Some(user_id),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}
