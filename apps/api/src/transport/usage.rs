use std::collections::HashMap;

use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use anneal_core::QuotaState;
use anneal_notifications::NotificationKind;
use anneal_platform::NotificationJob;
use anneal_usage::{QuotaEnvelope, UsageBatchItem};
use apalis::prelude::TaskSink;

use crate::{
    app_state::AppState, error::ApiError, extractors::authenticated_actor,
    transport::rollout_sync::queue_tenant_rollouts_for_current_state,
};

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UsageBulkRequest {
    pub samples: Vec<UsageSampleRequest>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UsageSampleRequest {
    pub tenant_id: Uuid,
    pub node_id: Uuid,
    pub subscription_id: Uuid,
    pub device_id: Uuid,
    pub bytes_in: i64,
    pub bytes_out: i64,
    pub measured_at: chrono::DateTime<chrono::Utc>,
}

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
    let rows = state
        .usage_service()
        .list_usage_overview(tenant_id)
        .await
        .map_err(ApiError)?;
    Ok(Json(rows))
}

#[utoipa::path(post, path = "/api/v1/agent/usage/bulk", request_body = UsageBulkRequest)]
pub async fn ingest_usage(
    State(state): State<AppState>,
    Json(request): Json<UsageBulkRequest>,
) -> Result<Json<HashMap<Uuid, anneal_usage::QuotaDecision>>, ApiError> {
    let samples = request
        .samples
        .iter()
        .map(|sample| UsageBatchItem {
            tenant_id: sample.tenant_id,
            node_id: sample.node_id,
            subscription_id: sample.subscription_id,
            device_id: sample.device_id,
            bytes_in: sample.bytes_in,
            bytes_out: sample.bytes_out,
            measured_at: sample.measured_at,
        })
        .collect::<Vec<_>>();

    let mut quotas = Vec::new();
    for sample in &request.samples {
        let row = sqlx::query_as::<_, (Uuid, i64, i64)>(
            "select id, traffic_limit_bytes, used_bytes from subscriptions where id = $1",
        )
        .bind(sample.subscription_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|error| {
            ApiError(anneal_core::ApplicationError::Infrastructure(
                error.to_string(),
            ))
        })?;
        if let Some((subscription_id, traffic_limit_bytes, current_used_bytes)) = row {
            quotas.push(QuotaEnvelope {
                subscription_id,
                traffic_limit_bytes,
                current_used_bytes,
            });
        }
    }
    let decisions = state
        .usage_service()
        .ingest(samples, quotas)
        .await
        .map_err(ApiError)?;
    let mut suspended_tenants = Vec::new();

    for sample in request.samples {
        if let Some(decision) = decisions.get(&sample.subscription_id) {
            if decision.suspend {
                suspended_tenants.push(sample.tenant_id);
            }
            let kind = match decision.quota_state {
                QuotaState::Warning80 => Some(NotificationKind::Quota80),
                QuotaState::Warning95 => Some(NotificationKind::Quota95),
                QuotaState::Exhausted => Some(NotificationKind::Quota100),
                QuotaState::Normal => None,
            };
            if let Some(kind) = kind {
                let event = state
                    .notification_service()
                    .create_event(
                        sample.tenant_id,
                        kind,
                        "Quota alert".into(),
                        format!(
                            "Subscription {} reached {:?} with {} bytes used",
                            sample.subscription_id, decision.quota_state, decision.used_bytes
                        ),
                    )
                    .await
                    .map_err(ApiError)?;
                state
                    .notification_queue
                    .clone()
                    .push(NotificationJob { event_id: event.id })
                    .await
                    .map_err(|error| {
                        ApiError(anneal_core::ApplicationError::Infrastructure(
                            error.to_string(),
                        ))
                    })?;
                state
                    .audit_service()
                    .write(
                        None,
                        Some(sample.tenant_id),
                        if decision.suspend {
                            "usage.quota.suspend"
                        } else {
                            "usage.quota.warning"
                        },
                        "subscription",
                        Some(sample.subscription_id),
                        json!({
                            "quota_state": decision.quota_state,
                            "used_bytes": decision.used_bytes,
                            "notification_id": event.id
                        }),
                    )
                    .await
                    .map_err(ApiError)?;
            }
        }
    }

    suspended_tenants.sort_unstable();
    suspended_tenants.dedup();
    for tenant_id in suspended_tenants {
        queue_tenant_rollouts_for_current_state(&state, tenant_id, "quota-sync")
            .await
            .map_err(ApiError)?;
    }

    Ok(Json(decisions))
}
