use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::ValueEnum;
use rand::{RngExt, distr::Alphanumeric};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::cli::InstallArgs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum InstallRole {
    ControlPlane,
}

impl InstallRole {
    pub fn includes_control_plane(self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    Native,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    pub role: InstallRole,
    pub deployment_mode: DeploymentMode,
    pub release_version: Option<String>,
    pub install_user: String,
    pub install_group: String,
    pub control_plane: ControlPlaneConfig,
    pub mihomo: MihomoConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPlaneConfig {
    pub domain: String,
    pub panel_path: String,
    pub public_base_url: String,
    pub database_url: String,
    pub bootstrap_token: Option<String>,
    pub data_encryption_key: String,
    pub token_hash_key: String,
    pub access_jwt_secret: String,
    pub pre_auth_jwt_secret: String,
    pub otlp_endpoint: Option<String>,
    pub superadmin: AdminConfig,
    pub reseller: Option<ResellerConfig>,
    pub starter_subscription: Option<StarterSubscriptionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MihomoConfig {
    pub public_host: String,
    pub public_port: u16,
    pub protocols: String,
    pub server_name: Option<String>,
    pub reality_public_key: Option<String>,
    pub reality_short_id: Option<String>,
    pub cipher: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminConfig {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResellerConfig {
    pub tenant_name: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarterSubscriptionConfig {
    pub name: String,
    pub traffic_limit_bytes: i64,
    pub days: i64,
}

#[derive(Debug, Clone)]
pub struct InstallLayout {
    pub install_root: PathBuf,
    pub config_dir: PathBuf,
    pub data_root: PathBuf,
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub env_path: PathBuf,
    pub summary_path: PathBuf,
    pub caddyfile_path: PathBuf,
    pub systemd_dir: PathBuf,
    pub utility_path: PathBuf,
}

impl Default for InstallLayout {
    fn default() -> Self {
        let install_root = PathBuf::from("/opt/anneal");
        let config_dir = PathBuf::from("/etc/anneal");
        let data_root = PathBuf::from("/var/lib/anneal");
        Self {
            install_root: install_root.clone(),
            config_dir: config_dir.clone(),
            data_root: data_root.clone(),
            config_path: config_dir.join("install.toml"),
            state_path: data_root.join("install-state.json"),
            env_path: config_dir.join("anneal.env"),
            summary_path: config_dir.join("admin-summary.env"),
            caddyfile_path: config_dir.join("Caddyfile"),
            systemd_dir: PathBuf::from("/etc/systemd/system"),
            utility_path: PathBuf::from("/usr/local/bin/annealctl"),
        }
    }
}

impl InstallLayout {
    pub fn bin_dir(&self) -> PathBuf {
        self.install_root.join("bin")
    }

    pub fn migrations_dir(&self) -> PathBuf {
        self.install_root.join("migrations")
    }

    pub fn web_dir(&self) -> PathBuf {
        self.install_root.join("web")
    }

    pub fn mihomo_config_path(&self) -> PathBuf {
        self.data_root.join("mihomo").join("config.yaml")
    }
}

impl InstallConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&raw).context("failed to parse install config")
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self).context("failed to serialize install config")?;
        fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))?;
        set_owner_only_permissions(path)?;
        Ok(())
    }

    pub fn from_args(args: InstallArgs) -> Result<Self> {
        let role = args.role.unwrap_or(InstallRole::ControlPlane);
        let deployment_mode = args.deployment_mode.unwrap_or(DeploymentMode::Native);
        let control_plane = resolve_control_plane(&args)?;
        let mihomo = MihomoConfig {
            public_host: public_host_from_url(
                &control_plane.public_base_url,
                &control_plane.domain,
            ),
            public_port: 443,
            protocols: "vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2".into(),
            server_name: Some(control_plane.domain.clone()),
            reality_public_key: None,
            reality_short_id: None,
            cipher: "2022-blake3-aes-128-gcm".into(),
        };
        Ok(Self {
            role,
            deployment_mode,
            release_version: None,
            install_user: "anneal".into(),
            install_group: "anneal".into(),
            control_plane,
            mihomo,
        })
    }

    pub fn clear_control_plane_bootstrap_token(&mut self) {
        self.control_plane.bootstrap_token = None;
    }
}

fn resolve_control_plane(args: &InstallArgs) -> Result<ControlPlaneConfig> {
    let mut domain = non_empty(args.domain.clone());
    let mut panel_path = non_empty(args.panel_path.clone());
    let mut public_base_url = non_empty(args.public_base_url.clone());

    if let Some(value) = public_base_url.as_deref() {
        hydrate_from_public_url(value, &mut domain, &mut panel_path)?;
    }
    if domain.is_none() {
        domain = args.public_base_url.clone();
    }
    if let Some(value) = domain.clone()
        && (value.starts_with("http://") || value.starts_with("https://"))
    {
        public_base_url = Some(value.clone());
        hydrate_from_public_url(&value, &mut domain, &mut panel_path)?;
    }

    let domain = normalize_domain_input(
        domain
            .ok_or_else(|| anyhow!("domain or panel URL is required"))?
            .as_str(),
    )?;
    let panel_path = panel_path
        .as_deref()
        .map(normalize_panel_path)
        .transpose()?
        .unwrap_or_else(|| generate_hex(24));
    let public_base_url = public_base_url.unwrap_or_else(|| {
        format!(
            "https://{domain}/{}",
            normalize_panel_path(&panel_path).expect("panel path")
        )
    });
    let database_url = non_empty(args.database_url.clone()).unwrap_or_else(default_database_url);
    validate_database_url(&database_url)?;
    validate_https_url(&public_base_url)?;

    let email_default = format!("admin-{}@{domain}", generate_hex(3));
    let superadmin_email =
        non_empty(args.superadmin_email.clone()).unwrap_or_else(|| email_default.clone());
    let superadmin_password = args
        .superadmin_password
        .clone()
        .and_then(|value| non_empty(Some(value)))
        .unwrap_or_else(|| generate_secret(18));

    let reseller_email_default = format!("tenant-{}@{domain}", generate_hex(3));

    Ok(ControlPlaneConfig {
        domain,
        panel_path,
        public_base_url,
        database_url,
        bootstrap_token: Some(
            non_empty(args.bootstrap_token.clone()).unwrap_or_else(|| generate_secret(24)),
        ),
        data_encryption_key: args
            .data_encryption_key
            .clone()
            .and_then(|value| non_empty(Some(value)))
            .unwrap_or_else(|| generate_hex(32)),
        token_hash_key: args
            .token_hash_key
            .clone()
            .and_then(|value| non_empty(Some(value)))
            .unwrap_or_else(|| generate_hex(32)),
        access_jwt_secret: args
            .access_jwt_secret
            .clone()
            .and_then(|value| non_empty(Some(value)))
            .unwrap_or_else(|| generate_hex(32)),
        pre_auth_jwt_secret: args
            .pre_auth_jwt_secret
            .clone()
            .and_then(|value| non_empty(Some(value)))
            .unwrap_or_else(|| generate_hex(32)),
        otlp_endpoint: non_empty(args.otlp_endpoint.clone()),
        superadmin: AdminConfig {
            email: superadmin_email,
            display_name: non_empty(Some(args.superadmin_display_name.clone()))
                .unwrap_or_else(|| "Superadmin".into()),
            password: superadmin_password,
        },
        reseller: Some(ResellerConfig {
            tenant_name: non_empty(args.reseller_tenant_name.clone())
                .unwrap_or_else(|| "Default Tenant".into()),
            email: non_empty(args.reseller_email.clone()).unwrap_or(reseller_email_default),
            display_name: non_empty(args.reseller_display_name.clone())
                .unwrap_or_else(|| "Tenant Admin".into()),
            password: non_empty(args.reseller_password.clone())
                .unwrap_or_else(|| generate_secret(18)),
        }),
        starter_subscription: Some(StarterSubscriptionConfig {
            name: args
                .starter_subscription_name
                .clone()
                .unwrap_or_else(|| "Starter access".into()),
            traffic_limit_bytes: args
                .starter_subscription_traffic_limit_bytes
                .unwrap_or(1_099_511_627_776),
            days: args.starter_subscription_days.unwrap_or(3650),
        }),
    })
}

fn hydrate_from_public_url(
    value: &str,
    domain: &mut Option<String>,
    panel_path: &mut Option<String>,
) -> Result<()> {
    let url = Url::parse(value).context("failed to parse public base URL")?;
    if domain.is_none() {
        domain.replace(
            url.host_str()
                .ok_or_else(|| anyhow!("public base URL host is required"))?
                .to_owned(),
        );
    }
    if panel_path.is_none() {
        panel_path.replace(normalize_panel_path(url.path())?);
    }
    Ok(())
}

fn normalize_domain_input(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("domain is required");
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let url = Url::parse(trimmed)?;
        return Ok(url
            .host_str()
            .ok_or_else(|| anyhow!("domain host is required"))?
            .to_owned());
    }
    let host = trimmed.trim_matches('/');
    if host.contains('/') {
        bail!("domain must not contain path segments");
    }
    Ok(host.to_owned())
}

pub fn normalize_panel_path(value: &str) -> Result<String> {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        bail!("panel path must not be empty");
    }
    Ok(trimmed.to_owned())
}

pub fn panel_base_href(panel_path: &str) -> String {
    format!("/{panel_path}/")
}

pub fn panel_path_prefix(panel_path: &str) -> String {
    format!("/{panel_path}")
}

fn validate_https_url(value: &str) -> Result<()> {
    let url = Url::parse(value).context("failed to parse URL")?;
    if url.scheme() != "https" {
        bail!("URL must use https");
    }
    Ok(())
}

fn validate_database_url(value: &str) -> Result<()> {
    let url = Url::parse(value).context("failed to parse database URL")?;
    if url.scheme() != "postgres" && url.scheme() != "postgresql" {
        bail!("database URL must use postgres:// or postgresql://");
    }
    if url.username().is_empty() {
        bail!("database URL user is required");
    }
    if url.path().trim_matches('/').is_empty() {
        bail!("database URL database name is required");
    }
    Ok(())
}

fn default_database_url() -> String {
    let db_name = format!("anneal_{}", generate_hex(4));
    let user = format!("anneal_{}", generate_hex(4));
    let password = generate_secret(18);
    format!("postgres://{user}:{password}@127.0.0.1:5432/{db_name}")
}

fn public_host_from_url(value: &str, fallback: &str) -> String {
    Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| fallback.to_owned())
}

fn generate_hex(length: usize) -> String {
    let mut rng = rand::rng();
    let mut result = String::with_capacity(length * 2);
    for _ in 0..length {
        use std::fmt::Write as _;
        let _ = write!(&mut result, "{:02x}", rng.random::<u8>());
    }
    result
}

fn generate_secret(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed.to_owned())
    })
}

fn set_owner_only_permissions(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(_path, permissions)
            .with_context(|| format!("failed to chmod {}", _path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use reqwest::Url;

    use super::*;

    fn base_args() -> InstallArgs {
        InstallArgs {
            bundle_root: None,
            lang: None,
            role: Some(InstallRole::ControlPlane),
            deployment_mode: Some(DeploymentMode::Native),
            domain: Some("panel.example.com".into()),
            panel_path: None,
            public_base_url: None,
            database_url: None,
            otlp_endpoint: None,
            bootstrap_token: None,
            data_encryption_key: None,
            token_hash_key: None,
            access_jwt_secret: None,
            pre_auth_jwt_secret: None,
            superadmin_email: None,
            superadmin_display_name: "Superadmin".into(),
            superadmin_password: None,
            reseller_tenant_name: None,
            reseller_email: None,
            reseller_display_name: None,
            reseller_password: None,
            starter_subscription_name: None,
            starter_subscription_traffic_limit_bytes: None,
            starter_subscription_days: None,
            non_interactive: true,
        }
    }

    #[test]
    fn control_plane_defaults_generate_valid_critical_values() {
        let config = InstallConfig::from_args(base_args()).expect("config");
        let control_plane = config.control_plane;

        assert_eq!(config.role, InstallRole::ControlPlane);
        assert_eq!(config.deployment_mode, DeploymentMode::Native);
        assert_eq!(control_plane.domain, "panel.example.com");
        assert_eq!(control_plane.superadmin.display_name, "Superadmin");
        assert_eq!(control_plane.panel_path.len(), 48);
        assert_eq!(config.mihomo.public_host, "panel.example.com");
        assert_eq!(config.mihomo.public_port, 443);
        assert!(control_plane.reseller.is_some());
        assert!(control_plane.starter_subscription.is_some());

        let database_url = Url::parse(&control_plane.database_url).expect("database url");
        assert_eq!(database_url.scheme(), "postgres");
        assert_eq!(database_url.host_str(), Some("127.0.0.1"));
        assert_eq!(database_url.port(), Some(5432));
        assert!(!database_url.username().is_empty());
        assert!(database_url.password().is_some());
    }

    #[test]
    fn public_url_hydrates_domain_and_panel_path() {
        let mut args = base_args();
        args.domain = None;
        args.public_base_url = Some("https://panel.example.com/private-path".into());

        let config = InstallConfig::from_args(args).expect("config");
        let control_plane = config.control_plane;

        assert_eq!(control_plane.domain, "panel.example.com");
        assert_eq!(control_plane.panel_path, "private-path");
        assert_eq!(
            control_plane.public_base_url,
            "https://panel.example.com/private-path"
        );
    }
}
