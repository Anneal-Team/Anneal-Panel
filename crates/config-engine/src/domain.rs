use serde::{Deserialize, Serialize};
use sqlx::Type;
use utoipa::ToSchema;

use anneal_core::{ProtocolKind, ProxyEngine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "transport_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    Tcp,
    Ws,
    Grpc,
    HttpUpgrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "security_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SecurityKind {
    None,
    Tls,
    Reality,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClientCredential {
    pub email: String,
    pub uuid: String,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InboundProfile {
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
    pub reality_private_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub fingerprint: Option<String>,
    pub alpn: Vec<String>,
    pub cipher: Option<String>,
    pub tls_certificate_path: Option<String>,
    pub tls_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CanonicalConfig {
    pub engine: ProxyEngine,
    pub tag: String,
    pub server_name: Option<String>,
    pub credentials: Vec<ClientCredential>,
    pub inbound_profiles: Vec<InboundProfile>,
}
