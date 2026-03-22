use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::Type;
use std::ops::Deref;
use utoipa::ToSchema;
use uuid::Uuid;

use anneal_config_engine::{SecurityKind, TransportKind};
use anneal_core::{DeploymentStatus, NodeStatus, ProtocolKind, ProxyEngine};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NodeGroup {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "node_group_domain_mode", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum NodeGroupDomainMode {
    Direct,
    LegacyDirect,
    Cdn,
    AutoCdn,
    Relay,
    Worker,
    Reality,
    Fake,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct NodeGroupDomain {
    pub id: Uuid,
    pub node_group_id: Uuid,
    pub mode: NodeGroupDomainMode,
    pub domain: String,
    pub alias: Option<String>,
    pub server_names: Vec<String>,
    pub host_headers: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeGroupDomainDraft {
    pub mode: NodeGroupDomainMode,
    pub domain: String,
    pub alias: Option<String>,
    pub server_names: Vec<String>,
    pub host_headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NodeEnrollmentToken {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_group_id: Uuid,
    #[serde(skip_serializing, default)]
    pub token_hash: String,
    pub engine: ProxyEngine,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Node {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_group_id: Uuid,
    pub name: String,
    pub engine: ProxyEngine,
    pub version: String,
    pub status: NodeStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing, default)]
    pub node_token_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NodeEndpoint {
    pub id: Uuid,
    pub node_id: Uuid,
    pub protocol: ProtocolKind,
    pub listen_host: String,
    pub listen_port: i32,
    pub public_host: String,
    pub public_port: i32,
    pub transport: TransportKind,
    pub security: SecurityKind,
    pub server_name: Option<String>,
    pub host_header: Option<String>,
    pub path: Option<String>,
    pub service_name: Option<String>,
    pub flow: Option<String>,
    pub reality_public_key: Option<String>,
    #[serde(skip_serializing, default)]
    pub reality_private_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub fingerprint: Option<String>,
    pub alpn: Vec<String>,
    pub cipher: Option<String>,
    pub tls_certificate_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeEndpointDraft {
    pub protocol: ProtocolKind,
    pub listen_host: String,
    pub listen_port: u16,
    pub public_host: String,
    pub public_port: u16,
    pub transport: TransportKind,
    pub security: SecurityKind,
    pub server_name: Option<String>,
    pub host_header: Option<String>,
    pub path: Option<String>,
    pub service_name: Option<String>,
    pub flow: Option<String>,
    pub reality_public_key: Option<String>,
    #[serde(skip_serializing, default)]
    pub reality_private_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub fingerprint: Option<String>,
    pub alpn: Vec<String>,
    pub cipher: Option<String>,
    pub tls_certificate_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeliveryNodeEndpoint {
    pub node_id: Uuid,
    pub node_name: String,
    pub engine: ProxyEngine,
    pub protocol: ProtocolKind,
    pub listen_host: String,
    pub listen_port: i32,
    pub public_host: String,
    pub public_port: i32,
    pub transport: TransportKind,
    pub security: SecurityKind,
    pub server_name: Option<String>,
    pub host_header: Option<String>,
    pub path: Option<String>,
    pub service_name: Option<String>,
    pub flow: Option<String>,
    pub reality_public_key: Option<String>,
    #[serde(skip_serializing, default)]
    pub reality_private_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub fingerprint: Option<String>,
    pub alpn: Vec<String>,
    pub cipher: Option<String>,
    pub tls_certificate_path: Option<String>,
    pub tls_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeploymentRollout {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_id: Uuid,
    pub config_revision_id: Uuid,
    pub engine: ProxyEngine,
    pub revision_name: String,
    pub rendered_config: String,
    pub target_path: String,
    pub status: DeploymentStatus,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConfigRevision {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_id: Option<Uuid>,
    pub name: String,
    pub engine: ProxyEngine,
    pub rendered_config: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRegistration {
    pub name: String,
    pub version: String,
    pub engine: ProxyEngine,
    pub protocols: Vec<ProtocolKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentGrant {
    pub token: String,
    pub record: NodeEnrollmentToken,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NodeRegistrationGrant {
    pub node: Node,
    pub node_token: String,
}

impl Deref for NodeRegistrationGrant {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NodeCapability {
    pub node_id: Uuid,
    pub protocol: ProtocolKind,
}
