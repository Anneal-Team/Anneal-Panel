use anneal_core::ProxyEngine;
use anyhow::{Context, Result, anyhow, bail};
use chrono::{Duration, Utc};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

use crate::{
    config::{NodeConfig, StarterSubscriptionConfig},
    state::InstallState,
};

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum LoginResponse {
    Authenticated { tokens: SessionTokens },
    TotpRequired { pre_auth_token: String },
    TotpSetupRequired { pre_auth_token: String },
}

#[derive(Debug, Clone, Deserialize)]
struct SessionTokens {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct TotpSetup {
    secret: String,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapSession {
    pub bootstrap_token: String,
}

#[derive(Debug, Deserialize)]
pub struct ResellerResponse {
    pub tenant_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct NodeResponse {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubscriptionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub note: Option<String>,
    pub traffic_limit_bytes: i64,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub suspended: bool,
    pub delivery_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubscriptionResponse {
    pub delivery_url: String,
}

#[derive(Debug, Serialize)]
struct LoginRequest<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize)]
struct BootstrapRequest<'a> {
    email: &'a str,
    display_name: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize)]
struct TotpVerifyRequest<'a> {
    code: &'a str,
}

#[derive(Debug, Serialize)]
struct CreateResellerRequest<'a> {
    tenant_name: &'a str,
    email: &'a str,
    display_name: &'a str,
    password: &'a str,
}

#[derive(Debug, Serialize)]
struct CreateNodeRequest<'a> {
    tenant_id: Uuid,
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct CreateBootstrapSessionRequest {
    tenant_id: Uuid,
    engines: Vec<ProxyEngine>,
}

#[derive(Debug, Serialize)]
struct CreateSubscriptionRequest<'a> {
    tenant_id: Uuid,
    name: &'a str,
    note: Option<&'a str>,
    traffic_limit_bytes: i64,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct UpdateSubscriptionRequest<'a> {
    name: &'a str,
    note: Option<&'a str>,
    traffic_limit_bytes: i64,
    expires_at: chrono::DateTime<chrono::Utc>,
    suspended: bool,
}

impl ApiClient {
    pub fn local() -> Result<Self> {
        Self::new("http://127.0.0.1:8080/api/v1")
    }

    pub fn new(base_url: &str) -> Result<Self> {
        let client = Client::builder()
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn bootstrap_superadmin(
        &self,
        bootstrap_token: &str,
        email: &str,
        display_name: &str,
        password: &str,
    ) -> Result<()> {
        let response = self
            .client
            .post(self.url("/bootstrap"))
            .header("x-bootstrap-token", bootstrap_token)
            .json(&BootstrapRequest {
                email,
                display_name,
                password,
            })
            .send()
            .await
            .context("failed to bootstrap superadmin")?;
        match response.status() {
            StatusCode::OK | StatusCode::CONFLICT => Ok(()),
            status => {
                let body = response.text().await.unwrap_or_else(|_| String::new());
                bail!(
                    "{}",
                    http_error_message("bootstrap superadmin", status, body.trim())
                );
            }
        }
    }

    pub async fn login_superadmin(
        &self,
        email: &str,
        password: &str,
        state: &mut InstallState,
    ) -> Result<String> {
        let response = self
            .client
            .post(self.url("/auth/login"))
            .json(&LoginRequest { email, password })
            .send()
            .await
            .context("failed to log in as superadmin")?;
        let login: LoginResponse = self.json_response(response, "superadmin login").await?;
        match login {
            LoginResponse::Authenticated { tokens } => Ok(tokens.access_token),
            LoginResponse::TotpRequired { pre_auth_token } => {
                let secret = state
                    .bootstrap
                    .superadmin_totp_secret
                    .clone()
                    .ok_or_else(|| anyhow!("missing persisted TOTP secret for superadmin"))?;
                self.verify_totp(&pre_auth_token, &secret).await
            }
            LoginResponse::TotpSetupRequired { pre_auth_token } => {
                let setup = self.begin_totp_setup(&pre_auth_token).await?;
                state.bootstrap.superadmin_totp_secret = Some(setup.secret.clone());
                self.verify_totp(&pre_auth_token, &setup.secret).await
            }
        }
    }

    pub async fn create_reseller(
        &self,
        access_token: &str,
        tenant_name: &str,
        email: &str,
        display_name: &str,
        password: &str,
    ) -> Result<Uuid> {
        let response = self
            .client
            .post(self.url("/resellers"))
            .bearer_auth(access_token)
            .json(&CreateResellerRequest {
                tenant_name,
                email,
                display_name,
                password,
            })
            .send()
            .await
            .context("failed to create reseller")?;
        let reseller: ResellerResponse = self.json_response(response, "create reseller").await?;
        reseller
            .tenant_id
            .ok_or_else(|| anyhow!("reseller response did not include tenant_id"))
    }

    pub async fn create_node(
        &self,
        access_token: &str,
        tenant_id: Uuid,
        name: &str,
    ) -> Result<Uuid> {
        let response = self
            .client
            .post(self.url("/nodes"))
            .bearer_auth(access_token)
            .json(&CreateNodeRequest { tenant_id, name })
            .send()
            .await
            .context("failed to create node")?;
        let node: NodeResponse = self.json_response(response, "create node").await?;
        Ok(node.id)
    }

    pub async fn create_bootstrap_session(
        &self,
        access_token: &str,
        tenant_id: Uuid,
        node_id: Uuid,
        node: &NodeConfig,
    ) -> Result<BootstrapSession> {
        let response = self
            .client
            .post(self.url(&format!("/nodes/{node_id}/bootstrap-sessions")))
            .bearer_auth(access_token)
            .json(&CreateBootstrapSessionRequest {
                tenant_id,
                engines: node.engines.clone(),
            })
            .send()
            .await
            .context("failed to create node bootstrap session")?;
        self.json_response(response, "create bootstrap session")
            .await
    }

    pub async fn list_subscriptions(
        &self,
        access_token: &str,
    ) -> Result<Vec<SubscriptionResponse>> {
        let response = self
            .client
            .get(self.url("/subscriptions"))
            .bearer_auth(access_token)
            .send()
            .await
            .context("failed to list subscriptions")?;
        self.json_response(response, "list subscriptions").await
    }

    pub async fn touch_subscription(
        &self,
        access_token: &str,
        subscription: &SubscriptionResponse,
    ) -> Result<()> {
        let response = self
            .client
            .patch(self.url(&format!("/subscriptions/{}", subscription.id)))
            .bearer_auth(access_token)
            .json(&UpdateSubscriptionRequest {
                name: &subscription.name,
                note: subscription.note.as_deref(),
                traffic_limit_bytes: subscription.traffic_limit_bytes,
                expires_at: subscription.expires_at,
                suspended: subscription.suspended,
            })
            .send()
            .await
            .context("failed to touch subscription")?;
        self.require_success(response, "touch subscription").await?;
        Ok(())
    }

    pub async fn create_subscription(
        &self,
        access_token: &str,
        tenant_id: Uuid,
        starter: &StarterSubscriptionConfig,
    ) -> Result<String> {
        let response = self
            .client
            .post(self.url("/subscriptions"))
            .bearer_auth(access_token)
            .json(&CreateSubscriptionRequest {
                tenant_id,
                name: &starter.name,
                note: None,
                traffic_limit_bytes: starter.traffic_limit_bytes,
                expires_at: Utc::now() + Duration::days(starter.days),
            })
            .send()
            .await
            .context("failed to create starter subscription")?;
        let created: CreateSubscriptionResponse = self
            .json_response(response, "create starter subscription")
            .await?;
        Ok(created.delivery_url)
    }

    async fn begin_totp_setup(&self, pre_auth_token: &str) -> Result<TotpSetup> {
        let response = self
            .client
            .post(self.url("/auth/totp/setup"))
            .bearer_auth(pre_auth_token)
            .send()
            .await
            .context("failed to request TOTP setup")?;
        self.json_response(response, "request TOTP setup").await
    }

    async fn verify_totp(&self, pre_auth_token: &str, secret: &str) -> Result<String> {
        let code = generate_totp_code(secret)?;
        let response = self
            .client
            .post(self.url("/auth/totp/verify"))
            .bearer_auth(pre_auth_token)
            .json(&TotpVerifyRequest { code: &code })
            .send()
            .await
            .context("failed to verify TOTP")?;
        let tokens: SessionTokens = self.json_response(response, "verify TOTP").await?;
        Ok(tokens.access_token)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn json_response<T>(&self, response: reqwest::Response, context: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self.require_success(response, context).await?;
        response
            .json()
            .await
            .with_context(|| format!("failed to parse {context} response"))
    }

    async fn require_success(
        &self,
        response: reqwest::Response,
        context: &str,
    ) -> Result<reqwest::Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let body = response.text().await.unwrap_or_else(|_| String::new());
        bail!("{}", http_error_message(context, status, body.trim()));
    }
}

fn generate_totp_code(secret: &str) -> Result<String> {
    let bytes = Secret::Encoded(secret.into())
        .to_bytes()
        .context("failed to decode TOTP secret")?;
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        bytes,
        Some("Anneal".into()),
        "installer".into(),
    )
    .context("failed to build TOTP generator")?;
    totp.generate_current()
        .context("failed to generate current TOTP code")
}

fn http_error_message(context: &str, status: StatusCode, body: &str) -> String {
    let detail = extract_error_detail(body);
    if detail.is_empty() {
        return format!("{context} failed with HTTP {status}");
    }
    format!("{context} failed with HTTP {status}: {detail}")
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
    use reqwest::StatusCode;

    use super::{extract_error_detail, http_error_message};

    #[test]
    fn http_error_message_uses_api_message_field() {
        let message = http_error_message(
            "create reseller",
            StatusCode::INTERNAL_SERVER_ERROR,
            r#"{"message":"internal server error"}"#,
        );

        assert_eq!(
            message,
            "create reseller failed with HTTP 500 Internal Server Error: internal server error"
        );
    }

    #[test]
    fn http_error_message_falls_back_to_raw_body() {
        let message = http_error_message(
            "create bootstrap session",
            StatusCode::BAD_REQUEST,
            "plain text body",
        );

        assert_eq!(
            message,
            "create bootstrap session failed with HTTP 400 Bad Request: plain text body"
        );
    }

    #[test]
    fn extract_error_detail_ignores_empty_payload() {
        assert_eq!(extract_error_detail(""), "");
        assert_eq!(extract_error_detail("   "), "");
    }
}
