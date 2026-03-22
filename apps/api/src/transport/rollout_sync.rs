use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use anneal_config_engine::{CanonicalConfig, ClientCredential, ConfigRenderer, InboundProfile};
use anneal_core::DeploymentStatus;
use anneal_nodes::{ConfigRevision, DeploymentRollout, Node, NodeEndpoint, NodeRepository};
use anneal_platform::DeploymentJob;
use anneal_subscriptions::SubscriptionRepository;
use apalis::prelude::TaskSink;

use crate::app_state::AppState;

pub async fn queue_tenant_rollouts_for_current_state(
    state: &AppState,
    tenant_id: Uuid,
    reason: &str,
) -> anneal_core::ApplicationResult<()> {
    let nodes = sqlx::query_as::<_, Node>(
        "select * from nodes where tenant_id = $1 and status = 'online' order by name asc",
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;

    if nodes.is_empty() {
        return Ok(());
    }

    let credentials = state
        .subscriptions
        .list_subscriptions(Some(tenant_id))
        .await?
    .into_iter()
    .filter(|subscription| !subscription.suspended && subscription.expires_at > Utc::now())
    .map(|subscription| ClientCredential {
        email: subscription.name,
        uuid: subscription.id.to_string(),
        password: Some(subscription.access_key),
    })
    .collect::<Vec<_>>();

    if credentials.is_empty() {
        return Ok(());
    }

    for node in nodes {
        let endpoints = state
            .nodes
            .list_node_endpoints(node.id)
            .await?
            .into_iter()
            .filter(|endpoint| endpoint.enabled)
            .collect::<Vec<NodeEndpoint>>();

        if endpoints.is_empty() {
            continue;
        }

        let inbound_profiles = endpoints
            .iter()
            .map(map_endpoint_to_profile)
            .collect::<anneal_core::ApplicationResult<Vec<_>>>()?;
        let rendered_config = ConfigRenderer
            .render(&CanonicalConfig {
                engine: node.engine,
                tag: format!("tenant-{tenant_id}-{}", node.name),
                server_name: endpoints
                    .first()
                    .and_then(|endpoint| endpoint.server_name.clone()),
                credentials: credentials.clone(),
                inbound_profiles,
            })
            .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
        let revision_id = Uuid::new_v4();
        let rollout_id = Uuid::new_v4();
        let now = Utc::now();
        let revision_name = format!("{reason}-{}", now.timestamp());
        state
            .nodes
            .create_config_revision(ConfigRevision {
                id: revision_id,
                tenant_id,
                node_id: Some(node.id),
                name: revision_name.clone(),
                engine: node.engine,
                rendered_config: rendered_config.clone(),
                created_by: None,
                created_at: now,
            })
            .await?;
        state
            .nodes
            .create_rollout(DeploymentRollout {
                id: rollout_id,
                tenant_id,
                node_id: node.id,
                config_revision_id: revision_id,
                engine: node.engine,
                revision_name: revision_name.clone(),
                rendered_config,
                target_path: default_target_path(node.engine).to_owned(),
                status: DeploymentStatus::Queued,
                failure_reason: None,
                created_at: now,
                updated_at: now,
                applied_at: None,
            })
            .await?;
        state
            .deployment_queue
            .clone()
            .push(DeploymentJob { rollout_id })
            .await
            .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
        state
            .audit_service()
            .write(
                None,
                Some(tenant_id),
                "rollouts.auto_sync_queued",
                "rollout",
                Some(rollout_id),
                json!({ "node_id": node.id, "revision_id": revision_id, "reason": reason }),
            )
            .await?;
    }

    Ok(())
}

fn default_target_path(engine: anneal_core::ProxyEngine) -> &'static str {
    match engine {
        anneal_core::ProxyEngine::Xray => "xray/config.json",
        anneal_core::ProxyEngine::Singbox => "singbox/config.json",
    }
}

fn map_endpoint_to_profile(
    endpoint: &NodeEndpoint,
) -> anneal_core::ApplicationResult<InboundProfile> {
    Ok(InboundProfile {
        protocol: endpoint.protocol,
        listen_host: endpoint.listen_host.clone(),
        listen_port: u16::try_from(endpoint.listen_port)
            .map_err(|_| anneal_core::ApplicationError::Validation("invalid listen_port".into()))?,
        public_host: endpoint.public_host.clone(),
        public_port: u16::try_from(endpoint.public_port)
            .map_err(|_| anneal_core::ApplicationError::Validation("invalid public_port".into()))?,
        transport: endpoint.transport,
        security: endpoint.security,
        server_name: endpoint.server_name.clone(),
        host_header: endpoint.host_header.clone(),
        path: endpoint.path.clone(),
        service_name: endpoint.service_name.clone(),
        flow: endpoint.flow.clone(),
        reality_public_key: endpoint.reality_public_key.clone(),
        reality_private_key: endpoint.reality_private_key.clone(),
        reality_short_id: endpoint.reality_short_id.clone(),
        fingerprint: endpoint.fingerprint.clone(),
        alpn: endpoint.alpn.clone(),
        cipher: endpoint.cipher.clone(),
        tls_certificate_path: endpoint.tls_certificate_path.clone(),
        tls_key_path: endpoint.tls_key_path.clone(),
    })
}
