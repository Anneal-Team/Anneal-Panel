use std::collections::{HashMap, HashSet};

use axum::{Json, extract::State, http::HeaderMap};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use anneal_core::{ApplicationError, QuotaState};
use anneal_nodes::{NodeRuntime, NodeRepository};
use anneal_notifications::NotificationKind;
use anneal_platform::NotificationJob;
use anneal_rbac::{AccessScope, Permission};
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

#[utoipa::path(post, path = "/api/v1/agent/usage/bulk", request_body = UsageBulkRequest)]
pub async fn ingest_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UsageBulkRequest>,
) -> Result<Json<HashMap<Uuid, anneal_usage::QuotaDecision>>, ApiError> {
    let node = authenticated_node(&headers, &state).await.map_err(ApiError)?;
    let samples = validate_usage_samples(&state, &node, &request.samples)
        .await
        .map_err(ApiError)?;
    let quotas = load_quotas(&state, &samples).await.map_err(ApiError)?;
    let decisions = state
        .usage_service()
        .ingest(samples, quotas)
        .await
        .map_err(ApiError)?;
    let mut suspended_tenants = Vec::new();
    let mut notified_subscriptions = HashSet::new();

    for sample in request.samples {
        if !notified_subscriptions.insert(sample.subscription_id) {
            continue;
        }
        if let Some(decision) = decisions.get(&sample.subscription_id) {
            if decision.suspend {
                suspended_tenants.push(node.tenant_id);
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
                        node.tenant_id,
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
                        Some(node.tenant_id),
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

async fn authenticated_node(headers: &HeaderMap, state: &AppState) -> anneal_core::ApplicationResult<NodeRuntime> {
    let token = bearer_token(headers)?;
    state
        .nodes
        .find_node_by_token_hash(&state.token_hasher.hash(token))
        .await?
        .ok_or(ApplicationError::Unauthorized)
}

async fn validate_usage_samples(
    state: &AppState,
    node: &NodeRuntime,
    samples: &[UsageSampleRequest],
) -> anneal_core::ApplicationResult<Vec<UsageBatchItem>> {
    let mut validated = Vec::with_capacity(samples.len());
    for sample in samples {
        let row = sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"
            select s.id, s.tenant_id
            from subscriptions s
            join devices d on d.id = s.device_id
            where s.id = $1 and d.id = $2
            "#,
        )
        .bind(sample.subscription_id)
        .bind(sample.device_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let Some((subscription_id, tenant_id)) = row else {
            return Err(ApplicationError::Forbidden);
        };
        if tenant_id != node.tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        validated.push(UsageBatchItem {
            tenant_id,
            node_id: node.id,
            subscription_id,
            device_id: sample.device_id,
            bytes_in: sample.bytes_in,
            bytes_out: sample.bytes_out,
            measured_at: sample.measured_at,
        });
    }
    Ok(validated)
}

async fn load_quotas(
    state: &AppState,
    samples: &[UsageBatchItem],
) -> anneal_core::ApplicationResult<Vec<QuotaEnvelope>> {
    let mut quotas = Vec::new();
    let mut seen = HashSet::new();
    for sample in samples {
        if !seen.insert(sample.subscription_id) {
            continue;
        }
        let row = sqlx::query_as::<_, (Uuid, i64, i64)>(
            "select id, traffic_limit_bytes, used_bytes from subscriptions where id = $1",
        )
        .bind(sample.subscription_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let Some((subscription_id, traffic_limit_bytes, current_used_bytes)) = row else {
            return Err(ApplicationError::Forbidden);
        };
        quotas.push(QuotaEnvelope {
            subscription_id,
            traffic_limit_bytes,
            current_used_bytes,
        });
    }
    Ok(quotas)
}

fn bearer_token(headers: &HeaderMap) -> anneal_core::ApplicationResult<&str> {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApplicationError::Unauthorized)?;
    authorization
        .strip_prefix("Bearer ")
        .ok_or(ApplicationError::Unauthorized)
}


