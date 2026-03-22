use anneal_rbac::{AccessScope, Permission};
use axum::{Json, extract::State, http::HeaderMap};

use crate::{app_state::AppState, error::ApiError, extractors::authenticated_actor};

#[utoipa::path(get, path = "/api/v1/notifications")]
pub async fn list_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_notifications::NotificationEvent>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let tenant_id = match actor.role {
        anneal_core::UserRole::Reseller => actor.tenant_id,
        anneal_core::UserRole::Admin | anneal_core::UserRole::Superadmin => None,
        anneal_core::UserRole::User => {
            return Err(ApiError(anneal_core::ApplicationError::Forbidden));
        }
    };
    state
        .rbac
        .authorize(
            &actor,
            Permission::ManageNotifications,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )
        .map_err(ApiError)?;
    let events = state
        .notification_service()
        .list_events(tenant_id)
        .await
        .map_err(ApiError)?;
    Ok(Json(events))
}
