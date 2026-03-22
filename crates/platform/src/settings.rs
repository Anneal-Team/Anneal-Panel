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
    pub access_jwt_secret: String,
    pub pre_auth_jwt_secret: String,
    pub otlp_endpoint: Option<String>,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub public_base_url: String,
    pub caddy_domain: String,
}

impl Settings {
    pub fn from_env() -> ApplicationResult<Self> {
        let bind_address = env::var("ANNEAL_BIND_ADDRESS")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .map_err(|error: std::net::AddrParseError| {
                ApplicationError::Validation(error.to_string())
            })?;
        Ok(Self {
            bind_address,
            database_url: required("ANNEAL_DATABASE_URL")?,
            migrations_dir: env::var("ANNEAL_MIGRATIONS_DIR")
                .unwrap_or_else(|_| default_migrations_dir().to_string_lossy().into_owned()),
            bootstrap_token: env::var("ANNEAL_BOOTSTRAP_TOKEN").ok(),
            data_encryption_key: required("ANNEAL_DATA_ENCRYPTION_KEY")?,
            access_jwt_secret: required("ANNEAL_ACCESS_JWT_SECRET")?,
            pre_auth_jwt_secret: required("ANNEAL_PRE_AUTH_JWT_SECRET")?,
            otlp_endpoint: env::var("ANNEAL_OTLP_ENDPOINT").ok(),
            telegram_bot_token: env::var("ANNEAL_TELEGRAM_BOT_TOKEN").ok(),
            telegram_chat_id: env::var("ANNEAL_TELEGRAM_CHAT_ID").ok(),
            public_base_url: env::var("ANNEAL_PUBLIC_BASE_URL")
                .unwrap_or_else(|_| "https://localhost".into()),
            caddy_domain: env::var("ANNEAL_CADDY_DOMAIN").unwrap_or_else(|_| "localhost".into()),
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
