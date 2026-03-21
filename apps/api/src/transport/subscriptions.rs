use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, header::CONTENT_TYPE},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use anneal_config_engine::SubscriptionDocumentFormat;
use anneal_subscriptions::{CreateSubscriptionCommand, UpdateSubscriptionCommand};

use crate::{
    app_state::AppState, error::ApiError, extractors::authenticated_actor,
    transport::rollout_sync::queue_tenant_rollouts_for_current_state,
};

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateSubscriptionRequest {
    pub tenant_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub name: String,
    pub note: Option<String>,
    pub traffic_limit_bytes: i64,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateSubscriptionRequest {
    pub name: String,
    pub note: Option<String>,
    pub traffic_limit_bytes: i64,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub suspended: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateSubscriptionResponse {
    pub subscription: anneal_subscriptions::Subscription,
    pub link: anneal_subscriptions::SubscriptionLink,
    pub delivery_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveSubscriptionQuery {
    pub format: Option<String>,
}

#[utoipa::path(post, path = "/api/v1/subscriptions", request_body = CreateSubscriptionRequest)]
pub async fn create_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateSubscriptionRequest>,
) -> Result<Json<CreateSubscriptionResponse>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let (subscription, link) = state
        .subscription_service()
        .create_subscription(
            &actor,
            CreateSubscriptionCommand {
                tenant_id: request.tenant_id,
                user_id: request.user_id,
                name: request.name,
                note: request.note,
                traffic_limit_bytes: request.traffic_limit_bytes,
                expires_at: request.expires_at,
            },
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(subscription.tenant_id),
            "subscriptions.create",
            "subscription",
            Some(subscription.id),
            json!({
                "traffic_limit_bytes": subscription.traffic_limit_bytes,
                "expires_at": subscription.expires_at
            }),
        )
        .await
        .map_err(ApiError)?;
    queue_tenant_rollouts_for_current_state(&state, subscription.tenant_id, "subscription-sync")
        .await
        .map_err(ApiError)?;
    Ok(Json(CreateSubscriptionResponse {
        delivery_url: format!("{}/s/{}", state.settings.public_base_url, link.token),
        subscription,
        link,
    }))
}

#[utoipa::path(get, path = "/api/v1/devices")]
pub async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_subscriptions::Device>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let devices = state
        .subscription_service()
        .list_devices(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(devices))
}

#[utoipa::path(get, path = "/api/v1/subscriptions")]
pub async fn list_subscriptions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_subscriptions::Subscription>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let subscriptions = state
        .subscription_service()
        .list_subscriptions(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(subscriptions))
}

#[utoipa::path(patch, path = "/api/v1/subscriptions/{id}", request_body = UpdateSubscriptionRequest)]
pub async fn update_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subscription_id): Path<uuid::Uuid>,
    Json(request): Json<UpdateSubscriptionRequest>,
) -> Result<Json<anneal_subscriptions::Subscription>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let subscription = state
        .subscription_service()
        .update_subscription(
            &actor,
            subscription_id,
            UpdateSubscriptionCommand {
                name: request.name,
                note: request.note,
                traffic_limit_bytes: request.traffic_limit_bytes,
                expires_at: request.expires_at,
                suspended: request.suspended,
            },
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(subscription.tenant_id),
            "subscriptions.update",
            "subscription",
            Some(subscription.id),
            json!({
                "traffic_limit_bytes": subscription.traffic_limit_bytes,
                "expires_at": subscription.expires_at,
                "suspended": subscription.suspended
            }),
        )
        .await
        .map_err(ApiError)?;
    queue_tenant_rollouts_for_current_state(&state, subscription.tenant_id, "subscription-sync")
        .await
        .map_err(ApiError)?;
    Ok(Json(subscription))
}

#[utoipa::path(delete, path = "/api/v1/subscriptions/{id}")]
pub async fn delete_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subscription_id): Path<uuid::Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let tenant_id = params
        .get("tenant_id")
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .or(actor.tenant_id)
        .ok_or_else(|| {
            ApiError(anneal_core::ApplicationError::Validation(
                "tenant_id query param is required".into(),
            ))
        })?;
    state
        .subscription_service()
        .delete_subscription(&actor, tenant_id, subscription_id)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(tenant_id),
            "subscriptions.delete",
            "subscription",
            Some(subscription_id),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    queue_tenant_rollouts_for_current_state(&state, tenant_id, "subscription-sync")
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(post, path = "/api/v1/subscriptions/{id}/rotate-link")]
pub async fn rotate_subscription_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subscription_id): Path<uuid::Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<anneal_subscriptions::SubscriptionLink>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let tenant_id = params
        .get("tenant_id")
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .or(actor.tenant_id)
        .ok_or_else(|| {
            ApiError(anneal_core::ApplicationError::Validation(
                "tenant_id query param is required".into(),
            ))
        })?;
    let link = state
        .subscription_service()
        .rotate_subscription_token(&actor, tenant_id, subscription_id)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(tenant_id),
            "subscriptions.rotate_link",
            "subscription",
            Some(subscription_id),
            json!({ "link_id": link.id }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(link))
}

#[utoipa::path(get, path = "/s/{token}")]
pub async fn resolve_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
    Query(query): Query<ResolveSubscriptionQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let format = match query.format.as_deref() {
        Some("raw") => SubscriptionDocumentFormat::Raw,
        Some("base64") => SubscriptionDocumentFormat::Base64,
        Some("clash-meta") | Some("clash") => SubscriptionDocumentFormat::ClashMeta,
        Some("sing-box") | Some("singbox") => SubscriptionDocumentFormat::SingBox,
        Some("hiddify-json") | Some("hiddify") | Some("json") => {
            SubscriptionDocumentFormat::HiddifyJson
        }
        None => detect_subscription_format(&headers),
        Some(_) => {
            return Err(ApiError(anneal_core::ApplicationError::Validation(
                "unsupported format".into(),
            )));
        }
    };
    let bundle = state
        .unified_subscription_service()
        .render_bundle(&token, None, format)
        .await
        .map_err(ApiError)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&bundle.content_type).map_err(|error| {
            ApiError(anneal_core::ApplicationError::Infrastructure(
                error.to_string(),
            ))
        })?,
    );
    headers.insert(
        "x-anneal-links-count",
        HeaderValue::from_str(&bundle.links_count.to_string()).map_err(|error| {
            ApiError(anneal_core::ApplicationError::Infrastructure(
                error.to_string(),
            ))
        })?,
    );
    Ok((headers, bundle.content))
}

fn detect_subscription_format(headers: &HeaderMap) -> SubscriptionDocumentFormat {
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if user_agent.contains("hiddify") {
        SubscriptionDocumentFormat::HiddifyJson
    } else if user_agent.contains("clash.meta")
        || user_agent.contains("clash")
        || user_agent.contains("mihomo")
    {
        SubscriptionDocumentFormat::ClashMeta
    } else if user_agent.contains("sing-box") || user_agent.contains("singbox") {
        SubscriptionDocumentFormat::SingBox
    } else {
        SubscriptionDocumentFormat::Base64
    }
}
