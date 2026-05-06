use std::{
    env,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anneal_core::{ApplicationError, ApplicationResult};

#[derive(Debug, Clone)]
pub struct Settings {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub migrations_dir: String,
    pub bootstrap_token: Option<String>,
    pub data_encryption_key: String,
    pub token_hash_key: String,
    pub access_jwt_secret: String,
    pub pre_auth_jwt_secret: String,
    pub otlp_endpoint: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub public_base_url: String,
    pub caddy_domain: String,
    pub mihomo_public_host: String,
    pub mihomo_public_port: u16,
    pub mihomo_protocols: String,
    pub mihomo_server_name: Option<String>,
    pub mihomo_reality_public_key: Option<String>,
    pub mihomo_reality_short_id: Option<String>,
    pub mihomo_cipher: String,
}

impl Settings {
    pub fn from_env() -> ApplicationResult<Self> {
        let bind_address = env::var("ANNEAL_BIND_ADDRESS")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .map_err(|error: std::net::AddrParseError| {
                ApplicationError::Validation(error.to_string())
            })?;
        let public_base_url =
            env::var("ANNEAL_PUBLIC_BASE_URL").unwrap_or_else(|_| "https://localhost".into());
        let caddy_domain = env::var("ANNEAL_CADDY_DOMAIN").unwrap_or_else(|_| "localhost".into());
        let mihomo_public_host = env::var("ANNEAL_MIHOMO_PUBLIC_HOST")
            .unwrap_or_else(|_| public_host_from_base_url(&public_base_url, &caddy_domain));
        let mihomo_public_port = env::var("ANNEAL_MIHOMO_PUBLIC_PORT")
            .ok()
            .map(|value| {
                value
                    .parse::<u16>()
                    .map_err(|error| ApplicationError::Validation(error.to_string()))
            })
            .transpose()?
            .unwrap_or(443);

        Ok(Self {
            bind_address,
            database_url: required("ANNEAL_DATABASE_URL")?,
            migrations_dir: env::var("ANNEAL_MIGRATIONS_DIR")
                .unwrap_or_else(|_| default_migrations_dir().to_string_lossy().into_owned()),
            bootstrap_token: env::var("ANNEAL_BOOTSTRAP_TOKEN").ok(),
            data_encryption_key: required("ANNEAL_DATA_ENCRYPTION_KEY")?,
            token_hash_key: required("ANNEAL_TOKEN_HASH_KEY")?,
            access_jwt_secret: required("ANNEAL_ACCESS_JWT_SECRET")?,
            pre_auth_jwt_secret: required("ANNEAL_PRE_AUTH_JWT_SECRET")?,
            otlp_endpoint: env::var("ANNEAL_OTLP_ENDPOINT").ok(),
            telegram_bot_token: env::var("ANNEAL_TELEGRAM_BOT_TOKEN").ok(),
            telegram_chat_id: env::var("ANNEAL_TELEGRAM_CHAT_ID").ok(),
            public_base_url,
            caddy_domain,
            mihomo_public_host,
            mihomo_public_port,
            mihomo_protocols: env::var("ANNEAL_MIHOMO_PROTOCOLS").unwrap_or_else(|_| {
                "vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2".into()
            }),
            mihomo_server_name: env::var("ANNEAL_MIHOMO_SERVER_NAME").ok(),
            mihomo_reality_public_key: env::var("ANNEAL_MIHOMO_REALITY_PUBLIC_KEY").ok(),
            mihomo_reality_short_id: env::var("ANNEAL_MIHOMO_REALITY_SHORT_ID").ok(),
            mihomo_cipher: env::var("ANNEAL_MIHOMO_CIPHER")
                .unwrap_or_else(|_| "2022-blake3-aes-128-gcm".into()),
        })
    }
}

fn required(name: &str) -> ApplicationResult<String> {
    env::var(name).map_err(|_| ApplicationError::Validation(format!("{name} is required")))
}

fn default_migrations_dir() -> PathBuf {
    let candidates = [
        Path::new("migrations").to_path_buf(),
        Path::new("/app/migrations").to_path_buf(),
        Path::new("/opt/anneal/migrations").to_path_buf(),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    Path::new("migrations").to_path_buf()
}

fn public_host_from_base_url(base_url: &str, fallback: &str) -> String {
    base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or(fallback)
        .split(':')
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_owned()
}
