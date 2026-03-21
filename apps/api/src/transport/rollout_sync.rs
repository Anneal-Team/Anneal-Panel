use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use anneal_config_engine::{CanonicalConfig, ClientCredential, ConfigRenderer, InboundProfile};
use anneal_nodes::{Node, NodeEndpoint};
use anneal_platform::DeploymentJob;
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

    let credentials = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"
        select s.id, s.name, s.access_key
        from subscriptions s
        where s.tenant_id = $1
          and s.suspended = false
          and s.expires_at > now() at time zone 'utc'
        order by s.name asc
        "#,
    )
    .bind(tenant_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?
    .into_iter()
    .map(
        |(subscription_id, subscription_name, access_key)| ClientCredential {
            email: subscription_name,
            uuid: subscription_id.to_string(),
            password: Some(access_key),
        },
    )
    .collect::<Vec<_>>();

    if credentials.is_empty() {
        return Ok(());
    }

    for node in nodes {
        let endpoints = sqlx::query_as::<_, NodeEndpoint>(
            "select * from node_endpoints where node_id = $1 and enabled = true order by public_port asc",
        )
        .bind(node.id)
        .fetch_all(&state.pool)
        .await
        .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;

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
        sqlx::query(
            r#"
            insert into config_revisions (id, tenant_id, node_id, name, engine, rendered_config, created_by, created_at)
            values ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
        )
        .bind(revision_id)
        .bind(tenant_id)
        .bind(node.id)
        .bind(&revision_name)
        .bind(node.engine)
        .bind(&rendered_config)
        .bind(Option::<Uuid>::None)
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            r#"
            insert into deployment_rollouts (
                id, tenant_id, node_id, config_revision_id, engine, revision_name, rendered_config, target_path, status, failure_reason, created_at, updated_at, applied_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,'queued',null,$9,$10,null)
            "#,
        )
        .bind(rollout_id)
        .bind(tenant_id)
        .bind(node.id)
        .bind(revision_id)
        .bind(node.engine)
        .bind(&revision_name)
        .bind(&rendered_config)
        .bind(default_target_path(node.engine))
        .bind(now)
        .bind(now)
        .execute(&state.pool)
        .await
        .map_err(|error| anneal_core::ApplicationError::Infrastructure(error.to_string()))?;
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
