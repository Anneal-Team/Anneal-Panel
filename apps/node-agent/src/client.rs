use anyhow::{Context, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use anneal_core::{ProtocolKind, ProxyEngine};
use anneal_nodes::DeploymentRollout;

#[derive(Debug, Serialize)]
pub struct RegisterNodeRequest {
    pub enrollment_token: String,
    pub name: String,
    pub version: String,
    pub engine: ProxyEngine,
    pub protocols: Vec<ProtocolKind>,
}

#[derive(Debug, Serialize)]
pub struct HeartbeatRequest {
    pub node_id: Uuid,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct PullRolloutsRequest {
    pub node_id: Uuid,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AckRolloutRequest {
    pub node_id: Uuid,
    pub success: bool,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisteredNode {
    id: Uuid,
}

pub async fn register(
    client: &Client,
    server_url: &str,
    request: RegisterNodeRequest,
) -> anyhow::Result<Uuid> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/register"))
        .json(&request)
        .send()
        .await?;
    let response = ensure_success(response).await?;
    let response = response
        .json::<RegisteredNode>()
        .await
        .context("failed to decode register response")?;
    Ok(response.id)
}

pub async fn heartbeat(
    client: &Client,
    server_url: &str,
    node_id: Uuid,
    version: &str,
) -> anyhow::Result<()> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/heartbeat"))
        .json(&HeartbeatRequest {
            node_id,
            version: version.into(),
        })
        .send()
        .await?;
    ensure_success(response).await?;
    Ok(())
}

pub async fn pull_rollouts(
    client: &Client,
    server_url: &str,
    node_id: Uuid,
) -> anyhow::Result<Vec<DeploymentRollout>> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/jobs/pull"))
        .json(&PullRolloutsRequest {
            node_id,
            limit: Some(10),
        })
        .send()
        .await?;
    let response = ensure_success(response).await?;
    response
        .json::<Vec<DeploymentRollout>>()
        .await
        .context("failed to decode rollouts")
}

pub async fn ack_rollout(
    client: &Client,
    server_url: &str,
    node_id: Uuid,
    rollout_id: Uuid,
    success: bool,
    failure_reason: Option<String>,
) -> anyhow::Result<()> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/jobs/{rollout_id}/ack"))
        .json(&AckRolloutRequest {
            node_id,
            success,
            failure_reason,
        })
        .send()
        .await?;
    ensure_success(response).await?;
    Ok(())
}

async fn ensure_success(response: reqwest::Response) -> anyhow::Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("<failed to read response body>"));
    Err(anyhow!("request failed with {status}: {body}"))
}
