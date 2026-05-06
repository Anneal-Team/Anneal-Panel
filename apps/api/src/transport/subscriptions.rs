use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use anneal_config_engine::SubscriptionDocumentFormat;
use anneal_subscriptions::{CreateSubscriptionCommand, UpdateSubscriptionCommand};

use crate::{app_state::AppState, error::ApiError, extractors::authenticated_actor};

#[derive(Debug, Serialize, ToSchema)]
pub struct DeviceResponse {
    pub id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub name: String,
    pub suspended: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<anneal_subscriptions::Device> for DeviceResponse {
    fn from(device: anneal_subscriptions::Device) -> Self {
        Self {
            id: device.id,
            tenant_id: device.tenant_id,
            user_id: device.user_id,
            name: device.name,
            suspended: device.suspended,
            created_at: device.created_at,
            updated_at: device.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubscriptionResponse {
    pub id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub device_id: uuid::Uuid,
    pub name: String,
    pub note: Option<String>,
    pub traffic_limit_bytes: i64,
    pub used_bytes: i64,
    pub quota_state: anneal_core::QuotaState,
    pub suspended: bool,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub delivery_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateSubscriptionRequest {
    pub tenant_id: uuid::Uuid,
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
    pub subscription: SubscriptionResponse,
    pub delivery_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RotateSubscriptionLinkResponse {
    pub delivery_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicSubscriptionResponse {
    pub name: String,
    pub note: Option<String>,
    pub traffic_limit_bytes: i64,
    pub used_bytes: i64,
    pub quota_state: anneal_core::QuotaState,
    pub suspended: bool,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub delivery_url: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ResolveSubscriptionQuery {
    pub raw: Option<String>,
    pub mode: Option<String>,
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
    Ok(Json(CreateSubscriptionResponse {
        delivery_url: format_delivery_url(&state.settings.public_base_url, &link.id),
        subscription: subscription_response(&state.settings.public_base_url, subscription),
    }))
}

#[utoipa::path(get, path = "/api/v1/devices")]
pub async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceResponse>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let devices = state
        .subscription_service()
        .list_devices(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        devices.into_iter().map(DeviceResponse::from).collect(),
    ))
}

#[utoipa::path(get, path = "/api/v1/subscriptions")]
pub async fn list_subscriptions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SubscriptionResponse>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let base_url = state.settings.public_base_url.clone();
    let subscriptions = state
        .subscription_service()
        .list_subscriptions(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        subscriptions
            .into_iter()
            .map(|subscription| subscription_response(&base_url, subscription))
            .collect(),
    ))
}

#[utoipa::path(patch, path = "/api/v1/subscriptions/{id}", request_body = UpdateSubscriptionRequest)]
pub async fn update_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subscription_id): Path<uuid::Uuid>,
    Json(request): Json<UpdateSubscriptionRequest>,
) -> Result<Json<SubscriptionResponse>, ApiError> {
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
    Ok(Json(subscription_response(
        &state.settings.public_base_url,
        subscription,
    )))
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
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(post, path = "/api/v1/subscriptions/{id}/rotate-link")]
pub async fn rotate_subscription_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subscription_id): Path<uuid::Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<RotateSubscriptionLinkResponse>, ApiError> {
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
    Ok(Json(RotateSubscriptionLinkResponse {
        delivery_url: format_delivery_url(&state.settings.public_base_url, &link.id),
    }))
}

#[utoipa::path(get, path = "/s/{token}")]
pub async fn resolve_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
    Query(query): Query<ResolveSubscriptionQuery>,
) -> Result<Response, ApiError> {
    match detect_delivery_mode(&headers, &query) {
        DeliveryMode::Page => Ok(Redirect::temporary(&format_public_page_url(
            &state.settings.public_base_url,
            &token,
        ))
        .into_response()),
        DeliveryMode::Bundle => {
            let bundle = state
                .unified_subscription_service()
                .render_bundle(&token, detect_subscription_format(&headers))
                .await
                .map_err(ApiError)?;
            Ok((
                [
                    ("content-type", bundle.content_type),
                    ("x-anneal-links-count", bundle.links_count.to_string()),
                    ("cache-control", "no-store".to_string()),
                    ("referrer-policy", "no-referrer".to_string()),
                ],
                bundle.content,
            )
                .into_response())
        }
    }
}

#[utoipa::path(get, path = "/api/v1/subscriptions/public/{token}")]
pub async fn public_subscription(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<PublicSubscriptionResponse>, ApiError> {
    let context = state
        .subscription_service()
        .resolve_subscription(&token)
        .await
        .map_err(ApiError)?;
    Ok(Json(public_subscription_response(
        &state.settings.public_base_url,
        context.subscription,
    )))
}

fn detect_subscription_format(headers: &HeaderMap) -> SubscriptionDocumentFormat {
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if user_agent.contains("clash.meta")
        || user_agent.contains("clash")
        || user_agent.contains("mihomo")
    {
        SubscriptionDocumentFormat::Mihomo
    } else {
        SubscriptionDocumentFormat::Base64
    }
}

fn subscription_response(
    base_url: &str,
    subscription: anneal_subscriptions::Subscription,
) -> SubscriptionResponse {
    let delivery_url = subscription
        .current_token
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .map(|link_id| format_delivery_url(base_url, &link_id));
    SubscriptionResponse {
        id: subscription.id,
        tenant_id: subscription.tenant_id,
        user_id: subscription.user_id,
        device_id: subscription.device_id,
        name: subscription.name,
        note: subscription.note,
        traffic_limit_bytes: subscription.traffic_limit_bytes,
        used_bytes: subscription.used_bytes,
        quota_state: subscription.quota_state,
        suspended: subscription.suspended,
        expires_at: subscription.expires_at,
        created_at: subscription.created_at,
        updated_at: subscription.updated_at,
        delivery_url,
    }
}

fn format_delivery_url(base_url: &str, link_id: &uuid::Uuid) -> String {
    format!("{base_url}/s/{link_id}")
}

fn public_subscription_response(
    base_url: &str,
    subscription: anneal_subscriptions::Subscription,
) -> PublicSubscriptionResponse {
    let delivery_url = subscription
        .current_token
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .map(|link_id| format_delivery_url(base_url, &link_id))
        .unwrap_or_default();
    PublicSubscriptionResponse {
        name: subscription.name,
        note: subscription.note,
        traffic_limit_bytes: subscription.traffic_limit_bytes,
        used_bytes: subscription.used_bytes,
        quota_state: subscription.quota_state,
        suspended: subscription.suspended,
        expires_at: subscription.expires_at,
        delivery_url,
    }
}

fn format_public_page_url(base_url: &str, token: &str) -> String {
    format!("{base_url}/import/{token}")
}

#[derive(Debug, PartialEq, Eq)]
enum DeliveryMode {
    Page,
    Bundle,
}

fn detect_delivery_mode(headers: &HeaderMap, query: &ResolveSubscriptionQuery) -> DeliveryMode {
    if query
        .mode
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("page"))
    {
        return DeliveryMode::Page;
    }
    if query
        .mode
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("raw"))
        || query
            .raw
            .as_deref()
            .is_some_and(|value| value != "0" && !value.eq_ignore_ascii_case("false"))
    {
        return DeliveryMode::Bundle;
    }
    if is_browser_request(headers) {
        DeliveryMode::Page
    } else {
        DeliveryMode::Bundle
    }
}

fn is_browser_request(headers: &HeaderMap) -> bool {
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if is_subscription_client(&user_agent) {
        return false;
    }
    let accept = headers
        .get("accept")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let sec_fetch_dest = headers
        .get("sec-fetch-dest")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let sec_fetch_mode = headers
        .get("sec-fetch-mode")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let browser_agent = user_agent.contains("mozilla")
        || user_agent.contains("chrome")
        || user_agent.contains("safari")
        || user_agent.contains("firefox")
        || user_agent.contains("edg/");
    (accept.contains("text/html") && browser_agent)
        || sec_fetch_dest == "document"
        || sec_fetch_mode == "navigate"
}

fn is_subscription_client(user_agent: &str) -> bool {
    [
        "clash",
        "mihomo",
        "stash",
        "shadowrocket",
        "surge",
        "loon",
        "nekobox",
        "v2rayng",
        "v2rayn",
        "sfa",
        "surfboard",
    ]
    .iter()
    .any(|needle| user_agent.contains(needle))
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::{DeliveryMode, ResolveSubscriptionQuery, detect_delivery_mode};

    #[test]
    fn browser_request_prefers_page_mode() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "accept",
            HeaderValue::from_static("text/html,application/xhtml+xml"),
        );
        headers.insert("user-agent", HeaderValue::from_static("Mozilla/5.0"));

        let mode = detect_delivery_mode(&headers, &ResolveSubscriptionQuery::default());

        assert_eq!(mode, DeliveryMode::Page);
    }

    #[test]
    fn known_client_prefers_bundle_mode() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "accept",
            HeaderValue::from_static("text/html,application/xhtml+xml"),
        );
        headers.insert("user-agent", HeaderValue::from_static("Mihomo"));

        let mode = detect_delivery_mode(&headers, &ResolveSubscriptionQuery::default());

        assert_eq!(mode, DeliveryMode::Bundle);
    }

    #[test]
    fn raw_query_forces_bundle_mode() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "accept",
            HeaderValue::from_static("text/html,application/xhtml+xml"),
        );
        headers.insert("user-agent", HeaderValue::from_static("Mozilla/5.0"));

        let mode = detect_delivery_mode(
            &headers,
            &ResolveSubscriptionQuery {
                raw: Some("1".into()),
                mode: None,
            },
        );

        assert_eq!(mode, DeliveryMode::Bundle);
    }
}
