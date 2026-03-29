use anyhow::{Context, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use anneal_core::{ProtocolKind, ProxyEngine};
use anneal_nodes::DeploymentRollout;

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BootstrapAgentRequest {
    pub bootstrap_token: String,
    pub runtimes: Vec<RegisterRuntimeRequest>,
}

#[derive(Debug, Serialize, Clone)]
pub struct RegisterRuntimeRequest {
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
pub struct BootstrappedRuntime {
    pub engine: ProxyEngine,
    pub node_id: Uuid,
    pub node_token: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeIdentity {
    pub node_id: Uuid,
    pub node_token: String,
}

pub async fn bootstrap(
    client: &Client,
    server_url: &str,
    request: BootstrapAgentRequest,
) -> anyhow::Result<Vec<BootstrappedRuntime>> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/bootstrap"))
        .json(&request)
        .send()
        .await?;
    let response = ensure_success(response).await?;
    response
        .json::<Vec<BootstrappedRuntime>>()
        .await
        .context("failed to decode bootstrap response")
}

pub async fn heartbeat(
    client: &Client,
    server_url: &str,
    identity: &RuntimeIdentity,
    version: &str,
) -> anyhow::Result<()> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/heartbeat"))
        .bearer_auth(&identity.node_token)
        .json(&HeartbeatRequest {
            node_id: identity.node_id,
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
    identity: &RuntimeIdentity,
) -> anyhow::Result<Vec<DeploymentRollout>> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/jobs/pull"))
        .bearer_auth(&identity.node_token)
        .json(&PullRolloutsRequest {
            node_id: identity.node_id,
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
    identity: &RuntimeIdentity,
    rollout_id: Uuid,
    success: bool,
    failure_reason: Option<String>,
) -> anyhow::Result<()> {
    let response = client
        .post(format!("{server_url}/api/v1/agent/jobs/{rollout_id}/ack"))
        .bearer_auth(&identity.node_token)
        .json(&AckRolloutRequest {
            node_id: identity.node_id,
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
    let url = response.url().clone();
    let body = response.text().await.unwrap_or_else(|_| String::new());
    Err(anyhow!(
        "{}",
        http_error_message(status, url.as_str(), body.trim())
    ))
}

fn http_error_message(status: reqwest::StatusCode, url: &str, body: &str) -> String {
    let detail = extract_error_detail(body);
    if detail.is_empty() {
        return format!("request failed with {status} for {url}");
    }
    format!("request failed with {status} for {url}: {detail}")
}

fn extract_error_detail(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    serde_json::from_str::<ErrorResponse>(trimmed)
        .ok()
        .and_then(|payload| payload.message)
        .map(|message| message.trim().to_owned())
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{build_client, ensure_success, extract_error_detail, http_error_message};
    use anneal_core::{ProtocolKind, ProxyEngine};
    use reqwest::Client;
    use reqwest::StatusCode;
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
    async fn ensure_success_includes_safe_error_detail() {
        let (server_url, handle) = spawn_http_server(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: 35\r\nConnection: close\r\n\r\n{\"message\":\"internal server error\"}",
        )
        .await
        .expect("server");
        let client = Client::new();
        let response = client
            .post(format!("{server_url}/api/v1/agent/bootstrap"))
            .send()
            .await
            .expect("response");

        let error = ensure_success(response)
            .await
            .expect_err("expected failure");
        assert!(error.to_string().contains("500 Internal Server Error"));
        assert!(error.to_string().contains("internal server error"));
        assert!(error.to_string().contains("/api/v1/agent/bootstrap"));

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
        let error = super::bootstrap(
            &client,
            &server_url,
            super::BootstrapAgentRequest {
                bootstrap_token: "token".into(),
                runtimes: vec![super::RegisterRuntimeRequest {
                    name: "node".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                }],
            },
        )
        .await
        .expect_err("expected redirect failure");

        assert!(error.to_string().contains("302"));
        handle.await.expect("server task").expect("server ok");
    }

    #[test]
    fn http_error_message_prefers_json_message() {
        let message = http_error_message(
            StatusCode::BAD_REQUEST,
            "https://panel.example.com/api/v1/agent/bootstrap",
            r#"{"message":"bootstrap session expired"}"#,
        );

        assert_eq!(
            message,
            "request failed with 400 Bad Request for https://panel.example.com/api/v1/agent/bootstrap: bootstrap session expired"
        );
    }

    #[test]
    fn extract_error_detail_falls_back_to_raw_body() {
        assert_eq!(extract_error_detail("plain text body"), "plain text body");
    }
}
