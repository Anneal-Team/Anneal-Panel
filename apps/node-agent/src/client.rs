use anyhow::{Context, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
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

pub fn build_client() -> anyhow::Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("failed to build agent client")
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
    Err(anyhow!("request failed with {status}"))
}

#[cfg(test)]
mod tests {
    use super::{build_client, ensure_success};
    use anneal_core::{ProtocolKind, ProxyEngine};
    use reqwest::Client;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn spawn_http_server(
        response: &'static str,
    ) -> anyhow::Result<(String, tokio::task::JoinHandle<anyhow::Result<()>>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer).await?;
            stream.write_all(response.as_bytes()).await?;
            stream.shutdown().await?;
            Ok(())
        });
        Ok((format!("http://{addr}"), handle))
    }

    #[tokio::test]
    async fn ensure_success_does_not_leak_response_body() {
        let (server_url, handle) = spawn_http_server(
            "HTTP/1.1 400 Bad Request\r\nContent-Length: 14\r\nConnection: close\r\n\r\nsupersecret123",
        )
        .await
        .expect("server");
        let client = Client::new();
        let response = client
            .post(format!("{server_url}/api/v1/agent/register"))
            .send()
            .await
            .expect("response");

        let error = ensure_success(response)
            .await
            .expect_err("expected failure");
        assert!(!error.to_string().contains("supersecret123"));

        handle.await.expect("server task").expect("server ok");
    }

    #[tokio::test]
    async fn client_does_not_follow_redirects() {
        let (server_url, handle) = spawn_http_server(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .await
        .expect("server");
        let client = build_client().expect("client");
        let error = super::register(
            &client,
            &server_url,
            super::RegisterNodeRequest {
                enrollment_token: "token".into(),
                name: "node".into(),
                version: "1.0.0".into(),
                engine: ProxyEngine::Xray,
                protocols: vec![ProtocolKind::VlessReality],
            },
        )
        .await
        .expect_err("expected redirect failure");

        assert!(error.to_string().contains("302"));
        handle.await.expect("server task").expect("server ok");
    }
}
