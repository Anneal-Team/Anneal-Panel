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
use anneal_nodes::{NodeEndpointDraft, NodeDomainDraft, RuntimeRegistration};

use crate::{
    app_state::AppState, error::ApiError, extractors::authenticated_actor,
    transport::rollout_sync::queue_tenant_rollouts_for_current_state,
};

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeEndpointResponse {
    pub id: Uuid,
    pub node_runtime_id: Uuid,
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
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<anneal_nodes::NodeEndpoint> for NodeEndpointResponse {
    fn from(endpoint: anneal_nodes::NodeEndpoint) -> Self {
        Self {
            id: endpoint.id,
            node_runtime_id: endpoint.node_id,
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
    pub node_runtime_id: Uuid,
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
            node_runtime_id: rollout.node_id,
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

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeRuntimeResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_id: Uuid,
    pub engine: ProxyEngine,
    pub version: String,
    pub status: anneal_core::NodeStatus,
    pub last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<anneal_nodes::NodeRuntime> for NodeRuntimeResponse {
    fn from(runtime: anneal_nodes::NodeRuntime) -> Self {
        Self {
            id: runtime.id,
            tenant_id: runtime.tenant_id,
            node_id: runtime.server_node_id,
            engine: runtime.engine,
            version: runtime.version,
            status: runtime.status,
            last_seen_at: runtime.last_seen_at,
            created_at: runtime.created_at,
            updated_at: runtime.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub runtimes: Vec<NodeRuntimeResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeDomainResponse {
    pub id: Uuid,
    pub node_id: Uuid,
    pub mode: anneal_nodes::NodeDomainMode,
    pub domain: String,
    pub alias: Option<String>,
    pub server_names: Vec<String>,
    pub host_headers: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<anneal_nodes::NodeDomain> for NodeDomainResponse {
    fn from(domain: anneal_nodes::NodeDomain) -> Self {
        Self {
            id: domain.id,
            node_id: domain.server_node_id,
            mode: domain.mode,
            domain: domain.domain,
            alias: domain.alias,
            server_names: domain.server_names,
            host_headers: domain.host_headers,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateNodeRequest {
    pub tenant_id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateNodeRequest {
    pub tenant_id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ReplaceNodeDomainsRequest {
    pub tenant_id: Uuid,
    pub domains: Vec<NodeDomainRequest>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct NodeDomainRequest {
    pub mode: anneal_nodes::NodeDomainMode,
    pub domain: String,
    pub alias: Option<String>,
    pub server_names: Vec<String>,
    pub host_headers: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateBootstrapSessionRequest {
    pub tenant_id: Uuid,
    pub engines: Vec<ProxyEngine>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct BootstrapAgentRequest {
    pub bootstrap_token: String,
    pub runtimes: Vec<BootstrapRuntimeRequest>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct BootstrapRuntimeRequest {
    pub name: String,
    pub version: String,
    pub engine: ProxyEngine,
    pub protocols: Vec<ProtocolKind>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct HeartbeatRequest {
    pub node_id: Uuid,
    pub version: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PullRolloutsRequest {
    pub node_id: Uuid,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AckRolloutRequest {
    pub node_id: Uuid,
    pub success: bool,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RotateNodeTokenRequest {
    pub node_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ReissueBootstrapRequest {
    pub tenant_id: Uuid,
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
    pub enabled: bool,
}

#[utoipa::path(post, path = "/api/v1/nodes", request_body = CreateNodeRequest)]
pub async fn create_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateNodeRequest>,
) -> Result<Json<NodeResponse>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let group = state
        .node_service()
        .create_server_node(&actor, request.tenant_id, request.name)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(group.tenant_id),
            "nodes.create",
            "node",
            Some(group.id),
            json!({ "name": group.name }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(NodeResponse {
        id: group.id,
        tenant_id: group.tenant_id,
        name: group.name,
        created_at: group.created_at,
        updated_at: group.updated_at,
        runtimes: Vec::new(),
    }))
}

#[utoipa::path(get, path = "/api/v1/nodes")]
pub async fn list_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<NodeResponse>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let groups = state
        .node_service()
        .list_server_nodes(&actor)
        .await
        .map_err(ApiError)?;
    let runtimes = state
        .node_service()
        .list_nodes(&actor)
        .await
        .map_err(ApiError)?;
    Ok(Json(build_node_responses(groups, runtimes)))
}

#[utoipa::path(patch, path = "/api/v1/nodes/{id}", request_body = UpdateNodeRequest)]
pub async fn update_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
    Json(request): Json<UpdateNodeRequest>,
) -> Result<Json<NodeResponse>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let group = state
        .node_service()
        .update_server_node(&actor, request.tenant_id, node_id, request.name)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(group.tenant_id),
            "nodes.update",
            "node",
            Some(group.id),
            json!({ "name": group.name }),
        )
        .await
        .map_err(ApiError)?;
    let runtimes = state
        .node_service()
        .list_nodes(&actor)
        .await
        .map_err(ApiError)?
        .into_iter()
        .filter(|runtime| runtime.server_node_id == group.id)
        .map(NodeRuntimeResponse::from)
        .collect();
    Ok(Json(NodeResponse {
        id: group.id,
        tenant_id: group.tenant_id,
        name: group.name,
        created_at: group.created_at,
        updated_at: group.updated_at,
        runtimes,
    }))
}

#[utoipa::path(delete, path = "/api/v1/nodes/{id}")]
pub async fn delete_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
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
        .delete_server_node(&actor, tenant_id, node_id)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(tenant_id),
            "nodes.delete",
            "node",
            Some(node_id),
            json!({}),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(json!({ "ok": true })))
}

#[utoipa::path(get, path = "/api/v1/nodes/{id}/domains")]
pub async fn list_node_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<NodeDomainResponse>>, ApiError> {
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
        .list_node_domains(&actor, tenant_id, node_id)
        .await
        .map_err(ApiError)?;
    Ok(Json(
        domains
            .into_iter()
            .map(NodeDomainResponse::from)
            .collect(),
    ))
}

#[utoipa::path(post, path = "/api/v1/nodes/{id}/domains", request_body = ReplaceNodeDomainsRequest)]
pub async fn replace_node_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
    Json(request): Json<ReplaceNodeDomainsRequest>,
) -> Result<Json<Vec<NodeDomainResponse>>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let tenant_id = request.tenant_id;
    let domains = state
        .node_service()
        .replace_node_domains(
            &actor,
            tenant_id,
            node_id,
            request
                .domains
                .into_iter()
                .map(|domain| NodeDomainDraft {
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
            "nodes.domains.replace",
            "node",
            Some(node_id),
            json!({ "count": domains.len() }),
        )
        .await
        .map_err(ApiError)?;
    queue_tenant_rollouts_for_current_state(&state, tenant_id, "group-domains-sync")
        .await
        .map_err(ApiError)?;
    Ok(Json(
        domains
            .into_iter()
            .map(NodeDomainResponse::from)
            .collect(),
    ))
}

#[utoipa::path(post, path = "/api/v1/nodes/{id}/bootstrap-sessions", request_body = CreateBootstrapSessionRequest)]
pub async fn create_bootstrap_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
    Json(request): Json<CreateBootstrapSessionRequest>,
) -> Result<Json<anneal_nodes::NodeBootstrapGrant>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let node = state
        .node_service()
        .list_server_nodes(&actor)
        .await
        .map_err(ApiError)?
        .into_iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| {
            ApiError(anneal_core::ApplicationError::NotFound("node not found".into()))
        })?;
    let grant = state
        .node_service()
        .create_bootstrap_token(
            &actor,
            request.tenant_id,
            node_id,
            node.name.clone(),
            request.engines,
        )
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(grant.tenant_id),
            "nodes.bootstrap_session.create",
            "node_bootstrap",
            Some(node_id),
            json!({ "engines": grant.engines, "node_name": grant.node_name }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(grant))
}

#[utoipa::path(post, path = "/api/v1/node-runtimes/{id}/endpoints", request_body = ReplaceNodeEndpointsRequest)]
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
            "node_runtime",
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

#[utoipa::path(get, path = "/api/v1/node-runtimes/{id}/endpoints")]
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

#[utoipa::path(post, path = "/api/v1/agent/bootstrap", request_body = BootstrapAgentRequest)]
pub async fn bootstrap_agent(
    State(state): State<AppState>,
    Json(request): Json<BootstrapAgentRequest>,
) -> Result<Json<Vec<anneal_nodes::NodeBootstrapRuntimeGrant>>, ApiError> {
    let grants = state
        .node_service()
        .bootstrap_nodes(
            &request.bootstrap_token,
            request
                .runtimes
                .into_iter()
                .map(|runtime| RuntimeRegistration {
                    name: runtime.name,
                    version: runtime.version,
                    engine: runtime.engine,
                    protocols: runtime.protocols,
                })
                .collect(),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(grants))
}

#[utoipa::path(post, path = "/api/v1/agent/heartbeat", request_body = HeartbeatRequest)]
pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<HeartbeatRequest>,
) -> Result<Json<anneal_nodes::NodeRuntime>, ApiError> {
    let node_token = bearer_node_token(&headers).map_err(ApiError)?;
    let node = state
        .node_service()
        .heartbeat(request.node_id, &node_token, &request.version)
        .await
        .map_err(ApiError)?;
    Ok(Json(node))
}

#[utoipa::path(post, path = "/api/v1/agent/jobs/pull", request_body = PullRolloutsRequest)]
pub async fn pull_rollouts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PullRolloutsRequest>,
) -> Result<Json<Vec<anneal_nodes::DeploymentRollout>>, ApiError> {
    let node_token = bearer_node_token(&headers).map_err(ApiError)?;
    let rollouts = state
        .node_service()
        .pull_rollouts(request.node_id, &node_token, request.limit.unwrap_or(10))
        .await
        .map_err(ApiError)?;
    Ok(Json(rollouts))
}

#[utoipa::path(post, path = "/api/v1/agent/jobs/{id}/ack", request_body = AckRolloutRequest)]
pub async fn ack_rollout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(rollout_id): Path<Uuid>,
    Json(request): Json<AckRolloutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let failure_reason = request.failure_reason.clone();
    let node_token = bearer_node_token(&headers).map_err(ApiError)?;
    let rollout = state
        .node_service()
        .acknowledge_rollout(
            request.node_id,
            &node_token,
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

#[utoipa::path(post, path = "/api/v1/agent/node-token/rotate", request_body = RotateNodeTokenRequest)]
pub async fn rotate_node_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RotateNodeTokenRequest>,
) -> Result<Json<anneal_nodes::NodeTokenRotationGrant>, ApiError> {
    let node_token = bearer_node_token(&headers).map_err(ApiError)?;
    let grant = state
        .node_service()
        .rotate_node_token(request.node_id, &node_token)
        .await
        .map_err(ApiError)?;
    Ok(Json(grant))
}

#[utoipa::path(post, path = "/api/v1/node-runtimes/{id}/reissue-bootstrap", request_body = ReissueBootstrapRequest)]
pub async fn reissue_bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<Uuid>,
    Json(request): Json<ReissueBootstrapRequest>,
) -> Result<Json<anneal_nodes::NodeBootstrapGrant>, ApiError> {
    let actor = authenticated_actor(&headers, &state).map_err(ApiError)?;
    let grant = state
        .node_service()
        .reissue_bootstrap_for_node(&actor, request.tenant_id, node_id)
        .await
        .map_err(ApiError)?;
    state
        .audit_service()
        .write(
            Some(actor.user_id),
            Some(grant.tenant_id),
            "nodes.bootstrap.reissue",
            "node",
            Some(node_id),
            json!({ "engines": grant.engines }),
        )
        .await
        .map_err(ApiError)?;
    Ok(Json(grant))
}

fn build_node_responses(
    groups: Vec<anneal_nodes::ServerNode>,
    runtimes: Vec<anneal_nodes::NodeRuntime>,
) -> Vec<NodeResponse> {
    let mut runtimes_by_node = std::collections::HashMap::<Uuid, Vec<NodeRuntimeResponse>>::new();
    for runtime in runtimes {
        runtimes_by_node
            .entry(runtime.server_node_id)
            .or_default()
            .push(NodeRuntimeResponse::from(runtime));
    }
    let mut nodes = groups
        .into_iter()
        .map(|group| {
            let mut node_runtimes = runtimes_by_node.remove(&group.id).unwrap_or_default();
            node_runtimes.sort_by_key(|runtime| match runtime.engine {
                ProxyEngine::Xray => 0_u8,
                ProxyEngine::Singbox => 1_u8,
            });
            NodeResponse {
                id: group.id,
                tenant_id: group.tenant_id,
                name: group.name,
                created_at: group.created_at,
                updated_at: group.updated_at,
                runtimes: node_runtimes,
            }
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.name.cmp(&right.name));
    nodes
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
            tls_certificate_path: None,
            tls_key_path: None,
            enabled: endpoint.enabled,
        })
        .collect()
}

fn bearer_node_token(headers: &HeaderMap) -> anneal_core::ApplicationResult<String> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(anneal_core::ApplicationError::Unauthorized)
}



