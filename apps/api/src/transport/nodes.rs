use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use anneal_core::{ProtocolKind, ProxyEngine};
use anneal_nodes::{NodeEndpointDraft, NodeGroupDomainDraft, NodeRegistration};

use crate::{
    app_state::AppState, error::ApiError, extractors::authenticated_actor,
    transport::rollout_sync::queue_tenant_rollouts_for_current_state,
};

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeEndpointResponse {
    pub id: Uuid,
    pub node_id: Uuid,
    pub protocol: ProtocolKind,
    pub listen_host: String,
    pub listen_port: i32,
    pub public_host: String,
    pub public_port: i32,
    pub transport: anneal_config_engine::TransportKind,
    pub security: anneal_config_engine::SecurityKind,
    pub server_name: Option<String>,
    pub host_header: Option<String>,
    pub path: Option<String>,
    pub service_name: Option<String>,
    pub flow: Option<String>,
    pub reality_public_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub fingerprint: Option<String>,
    pub alpn: Vec<String>,
    pub cipher: Option<String>,
    pub tls_certificate_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<anneal_nodes::NodeEndpoint> for NodeEndpointResponse {
    fn from(endpoint: anneal_nodes::NodeEndpoint) -> Self {
        Self {
            id: endpoint.id,
            node_id: endpoint.node_id,
            protocol: endpoint.protocol,
            listen_host: endpoint.listen_host,
            listen_port: endpoint.listen_port,
            public_host: endpoint.public_host,
            public_port: endpoint.public_port,
            transport: endpoint.transport,
            security: endpoint.security,
            server_name: endpoint.server_name,
            host_header: endpoint.host_header,
            path: endpoint.path,
            service_name: endpoint.service_name,
            flow: endpoint.flow,
            reality_public_key: endpoint.reality_public_key,
            reality_short_id: endpoint.reality_short_id,
            fingerprint: endpoint.fingerprint,
            alpn: endpoint.alpn,
            cipher: endpoint.cipher,
            tls_certificate_path: endpoint.tls_certificate_path,
            tls_key_path: endpoint.tls_key_path,
            enabled: endpoint.enabled,
            created_at: endpoint.created_at,
            updated_at: endpoint.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeploymentRolloutResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_id: Uuid,
    pub config_revision_id: Uuid,
    pub engine: ProxyEngine,
    pub revision_name: String,
    pub target_path: String,
    pub status: anneal_core::DeploymentStatus,
    pub failure_reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub applied_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<anneal_nodes::DeploymentRollout> for DeploymentRolloutResponse {
    fn from(rollout: anneal_nodes::DeploymentRollout) -> Self {
        Self {
            id: rollout.id,
            tenant_id: rollout.tenant_id,
            node_id: rollout.node_id,
            config_revision_id: rollout.config_revision_id,
            engine: rollout.engine,
            revision_name: rollout.revision_name,
            target_path: rollout.target_path,
            status: rollout.status,
            failure_reason: rollout.failure_reason,
            created_at: rollout.created_at,
            updated_at: rollout.updated_at,
            applied_at: rollout.applied_at,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateNodeGroupRequest {
    pub tenant_id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateNodeGroupRequest {
    pub tenant_id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ReplaceNodeGroupDomainsRequest {
    pub tenant_id: Uuid,
    pub domains: Vec<NodeGroupDomainRequest>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct NodeGroupDomainRequest {
    pub mode: anneal_nodes::NodeGroupDomainMode,
    pub domain: String,
    pub alias: Option<String>,
    pub server_names: Vec<String>,
    pub host_headers: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateEnrollmentTokenRequest {
    pub tenant_id: Uuid,
    pub node_group_id: Uuid,
    pub engine: ProxyEngine,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RegisterNodeRequest {
    pub enrollment_token: String,
    pub name: String,
    pub version: String,
    pub engine: ProxyEngine,
    pub protocols: Vec<ProtocolKind>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct HeartbeatRequest {
    pub node_id: Uuid,
    pub node_token: String,
    pub version: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PullRolloutsRequest {
    pub node_id: Uuid,
    pub node_token: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AckRolloutRequest {
    pub node_id: Uuid,
    pub node_token: String,
    pub success: bool,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ReplaceNodeEndpointsRequest {
    pub tenant_id: Uuid,
    pub endpoints: Vec<NodeEndpointRequest>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct NodeEndpointRequest {
    pub protocol: ProtocolKind,
    pub listen_host: String,
    pub listen_port: u16,
    pub public_host: String,
    pub public_port: u16,
    pub transport: anneal_config_engine::TransportKind,
    pub security: anneal_config_engine::SecurityKind,
    pub server_name: Option<String>,
    pub host_header: Option<String>,
    pub path: Option<String>,
    pub service_name: Option<String>,
    pub flow: Option<String>,
    pub fingerprint: Option<String>,
    pub alpn: Vec<String>,
    pub cipher: Option<String>,
    pub tls_certificate_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub enabled: bool,
}

#[utoipa::path(post, path = "/api/v1/node-groups", request_body = CreateNodeGroupRequest)]
pub async fn create_node_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateNodeGroupRequest>,
) -> Result<Json<anneal_nodes::NodeGroup>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let group = state
        .node_service()
        .create_node_group(&actor, request.tenant_id, request.name)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(group.tenant_id),
            "nodes.group.create",
            "node_group",
            Some(group.id),
            json!({ "name": group.name }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(group))
}

#[utoipa::path(get, path = "/api/v1/node-groups")]
pub async fn list_node_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_nodes::NodeGroup>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let rows = state
        .node_service()
        .list_node_groups(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(rows))
}

#[utoipa::path(patch, path = "/api/v1/node-groups/{id}", request_body = UpdateNodeGroupRequest)]
pub async fn update_node_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_group_id): Path<Uuid>,
    Json(request): Json<UpdateNodeGroupRequest>,
) -> Result<Json<anneal_nodes::NodeGroup>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let group = state
        .node_service()
        .update_node_group(&actor, request.tenant_id, node_group_id, request.name)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(group.tenant_id),
            "nodes.group.update",
            "node_group",
            Some(group.id),
            json!({ "name": group.name }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(group))
}

#[utoipa::path(delete, path = "/api/v1/node-groups/{id}")]
pub async fn delete_node_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_group_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = params
        .get("tenant_id")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| {
            ApiError(anneal_core::ApplicationError::Validation(
                "tenant_id query param is required".into(),
            ))
        })?;
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    state
        .node_service()
        .delete_node_group(&actor, tenant_id, node_group_id)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(tenant_id),
            "nodes.group.delete",
            "node_group",
            Some(node_group_id),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(get, path = "/api/v1/node-groups/{id}/domains")]
pub async fn list_node_group_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_group_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<anneal_nodes::NodeGroupDomain>>, ApiError> {
    let tenant_id = params
        .get("tenant_id")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| {
            ApiError(anneal_core::ApplicationError::Validation(
                "tenant_id query param is required".into(),
            ))
        })?;
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let domains = state
        .node_service()
        .list_node_group_domains(&actor, tenant_id, node_group_id)
        .await
        .map_err(ApiError)?;
    Ok(Json(domains))
}

#[utoipa::path(post, path = "/api/v1/node-groups/{id}/domains", request_body = ReplaceNodeGroupDomainsRequest)]
pub async fn replace_node_group_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_group_id): Path<Uuid>,
    Json(request): Json<ReplaceNodeGroupDomainsRequest>,
) -> Result<Json<Vec<anneal_nodes::NodeGroupDomain>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let tenant_id = request.tenant_id;
    let domains = state
        .node_service()
        .replace_node_group_domains(
            &actor,
            tenant_id,
            node_group_id,
            request
                .domains
                .into_iter()
                .map(|domain| NodeGroupDomainDraft {
                    mode: domain.mode,
                    domain: domain.domain,
                    alias: domain.alias,
                    server_names: domain.server_names,
                    host_headers: domain.host_headers,
                })
                .collect(),
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(tenant_id),
            "nodes.group_domains.replace",
            "node_group",
            Some(node_group_id),
            json!({ "count": domains.len() }),
        )
        .await
        .map_err(ApiError)?;
    queue_tenant_rollouts_for_current_state(&state, tenant_id, "group-domains-sync")
        .await
        .map_err(ApiError)?;
    Ok(Json(domains))
}

#[utoipa::path(post, path = "/api/v1/nodes/enrollment-tokens", request_body = CreateEnrollmentTokenRequest)]
pub async fn create_enrollment_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateEnrollmentTokenRequest>,
) -> Result<Json<anneal_nodes::EnrollmentGrant>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let grant = state
        .node_service()
        .create_enrollment_token(
            &actor,
            request.tenant_id,
            request.node_group_id,
            request.engine,
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(grant.record.tenant_id),
            "nodes.enrollment_token.create",
            "node_enrollment_token",
            Some(grant.record.id),
            json!({ "engine": grant.record.engine }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(grant))
}

#[utoipa::path(get, path = "/api/v1/nodes")]
pub async fn list_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<anneal_nodes::Node>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let nodes = state
        .node_service()
        .list_nodes(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(nodes))
}

#[utoipa::path(post, path = "/api/v1/nodes/{id}/endpoints", request_body = ReplaceNodeEndpointsRequest)]
pub async fn replace_node_endpoints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
    Json(request): Json<ReplaceNodeEndpointsRequest>,
) -> Result<Json<Vec<NodeEndpointResponse>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let tenant_id = request.tenant_id;
    let endpoints = state
        .node_service()
        .replace_node_endpoints(
            &actor,
            tenant_id,
            node_id,
            build_node_endpoint_drafts(request.endpoints),
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(tenant_id),
            "nodes.endpoints.replace",
            "node",
            Some(node_id),
            json!({ "count": endpoints.len() }),
        )
        .await
        .map_err(ApiError)?;
    queue_tenant_rollouts_for_current_state(&state, tenant_id, "endpoints-sync")
        .await
        .map_err(ApiError)?;
    Ok(Json(
        endpoints
            .into_iter()
            .map(NodeEndpointResponse::from)
            .collect(),
    ))
}

#[utoipa::path(get, path = "/api/v1/nodes/{id}/endpoints")]
pub async fn list_node_endpoints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<NodeEndpointResponse>>, ApiError> {
    let tenant_id = params
        .get("tenant_id")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| {
            ApiError(anneal_core::ApplicationError::Validation(
                "tenant_id query param is required".into(),
            ))
        })?;
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let endpoints = state
        .node_service()
        .list_node_endpoints(&actor, tenant_id, node_id)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        endpoints
            .into_iter()
            .map(NodeEndpointResponse::from)
            .collect(),
    ))
}

#[utoipa::path(get, path = "/api/v1/rollouts")]
pub async fn list_rollouts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeploymentRolloutResponse>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let rows = state
        .node_service()
        .list_rollouts(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        rows.into_iter()
            .map(DeploymentRolloutResponse::from)
            .collect(),
    ))
}

#[utoipa::path(post, path = "/api/v1/agent/register", request_body = RegisterNodeRequest)]
pub async fn register_agent(
    State(state): State<AppState>,
    Json(request): Json<RegisterNodeRequest>,
) -> Result<Json<anneal_nodes::domain::NodeRegistrationGrant>, ApiError> {
    let grant = state
        .node_service()
        .register_node(
            &request.enrollment_token,
            NodeRegistration {
                name: request.name,
                version: request.version,
                engine: request.engine,
                protocols: request.protocols,
            },
        )
        .await
        .map_err(ApiError)?;
    let node = &grant.node;
    state
        .audit_service()
        .write(
            None,
            Some(node.tenant_id),
            "nodes.register",
            "node",
            Some(node.id),
            json!({ "engine": node.engine, "name": node.name.clone() }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(grant))
}

#[utoipa::path(post, path = "/api/v1/agent/heartbeat", request_body = HeartbeatRequest)]
pub async fn heartbeat(
    State(state): State<AppState>,
    Json(request): Json<HeartbeatRequest>,
) -> Result<Json<anneal_nodes::Node>, ApiError> {
    let node = state
        .node_service()
        .heartbeat(request.node_id, &request.node_token, &request.version)
        .await
        .map_err(ApiError)?;
    Ok(Json(node))
}

#[utoipa::path(post, path = "/api/v1/agent/jobs/pull", request_body = PullRolloutsRequest)]
pub async fn pull_rollouts(
    State(state): State<AppState>,
    Json(request): Json<PullRolloutsRequest>,
) -> Result<Json<Vec<anneal_nodes::DeploymentRollout>>, ApiError> {
    let rollouts = state
        .node_service()
        .pull_rollouts(request.node_id, &request.node_token, request.limit.unwrap_or(10))
        .await
        .map_err(ApiError)?;
    Ok(Json(rollouts))
}

#[utoipa::path(post, path = "/api/v1/agent/jobs/{id}/ack", request_body = AckRolloutRequest)]
pub async fn ack_rollout(
    State(state): State<AppState>,
    Path(rollout_id): Path<Uuid>,
    Json(request): Json<AckRolloutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let failure_reason = request.failure_reason.clone();
    let rollout = state
        .node_service()
        .acknowledge_rollout(
            request.node_id,
            &request.node_token,
            rollout_id,
            request.success,
            failure_reason.clone(),
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            None,
            Some(rollout.tenant_id),
            if request.success {
                "nodes.rollout.applied"
            } else {
                "nodes.rollout.failed"
            },
            "rollout",
            Some(rollout_id),
            json!({ "node_id": request.node_id, "failure_reason": failure_reason }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "status": rollout.status
    })))
}

fn build_node_endpoint_drafts(endpoints: Vec<NodeEndpointRequest>) -> Vec<NodeEndpointDraft> {
    endpoints
        .into_iter()
        .map(|endpoint| NodeEndpointDraft {
            protocol: endpoint.protocol,
            listen_host: endpoint.listen_host,
            listen_port: endpoint.listen_port,
            public_host: endpoint.public_host,
            public_port: endpoint.public_port,
            transport: endpoint.transport,
            security: endpoint.security,
            server_name: endpoint.server_name,
            host_header: endpoint.host_header,
            path: endpoint.path,
            service_name: endpoint.service_name,
            flow: endpoint.flow,
            reality_public_key: None,
            reality_private_key: None,
            reality_short_id: None,
            fingerprint: endpoint.fingerprint,
            alpn: endpoint.alpn,
            cipher: endpoint.cipher,
            tls_certificate_path: endpoint.tls_certificate_path,
            tls_key_path: endpoint.tls_key_path,
            enabled: endpoint.enabled,
        })
        .collect()
}
