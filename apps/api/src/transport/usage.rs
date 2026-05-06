use axum::{Json, extract::State, http::HeaderMap};

use anneal_rbac::{AccessScope, Permission};

use crate::{app_state::AppState, error::ApiError, extractors::authenticated_actor};

#[utoipa::path(get, path = "/api/v1/usage")]
pub async fn list_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_usage::UsageOverview>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let tenant_id = if actor.role == anneal_core::UserRole::Reseller {
        actor.tenant_id
    } else {
        None
    };
    state
        .rbac
        .authorize(
            &actor,
            Permission::ManageUsage,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )
        .map_err(ApiError)?;
    let rows = state
        .usage_service()
        .list_usage_overview(tenant_id)
        .await
        .map_err(ApiError)?;
    Ok(Json(rows))
}
