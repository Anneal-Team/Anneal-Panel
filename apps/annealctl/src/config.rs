use std::{
    fs,
    path::{Path, PathBuf},
};

use anneal_core::{ProtocolKind, ProxyEngine};
use anyhow::{Context, Result, anyhow, bail};
use clap::ValueEnum;
use rand::{Rng, distr::Alphanumeric};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::cli::InstallArgs;

const XRAY_DEFAULT_PROTOCOLS: &str = "vless_reality,vmess,trojan,shadowsocks_2022";
const SINGBOX_DEFAULT_PROTOCOLS: &str =
    "vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2";
const LOCAL_CONTROL_PLANE_URL: &str = "http://127.0.0.1:8080";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum InstallRole {
    AllInOne,
    ControlPlane,
    Node,
}

impl InstallRole {
    pub fn includes_control_plane(self) -> bool {
        matches!(self, Self::AllInOne | Self::ControlPlane)
    }

    pub fn includes_node(self) -> bool {
        matches!(self, Self::AllInOne | Self::Node)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    Native,
    Docker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    pub role: InstallRole,
    pub deployment_mode: DeploymentMode,
    pub release_version: Option<String>,
    pub install_user: String,
    pub install_group: String,
    pub control_plane: Option<ControlPlaneConfig>,
    pub node: Option<NodeConfig>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub server_url: String,
    pub bootstrap_token: Option<String>,
    pub name: String,
    pub group_name: Option<String>,
    pub engines: Vec<ProxyEngine>,
    pub protocols_xray: Vec<ProtocolKind>,
    pub protocols_singbox: Vec<ProtocolKind>,
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
    pub docker_root: PathBuf,
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
            docker_root: install_root.join("docker"),
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

    pub fn docker_stack_root(&self, role: InstallRole) -> PathBuf {
        match role {
            InstallRole::ControlPlane => self.docker_root.join("control-plane"),
            InstallRole::Node => self.docker_root.join("node"),
            InstallRole::AllInOne => self.docker_root.join("control-plane"),
        }
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
        let role = args
            .role
            .ok_or_else(|| anyhow!("install role is required"))?;
        let deployment_mode = args
            .deployment_mode
            .ok_or_else(|| anyhow!("deployment mode is required"))?;

        let control_plane = if role.includes_control_plane() {
            Some(resolve_control_plane(&args, role)?)
        } else {
            None
        };
        let node = if role.includes_node() {
            Some(resolve_node(&args, role, control_plane.as_ref())?)
        } else {
            None
        };

        Ok(Self {
            role,
            deployment_mode,
            release_version: None,
            install_user: "anneal".into(),
            install_group: "anneal".into(),
            control_plane,
            node,
        })
    }

    pub fn clear_control_plane_bootstrap_token(&mut self) {
        if let Some(control_plane) = self.control_plane.as_mut() {
            control_plane.bootstrap_token = None;
        }
    }

    pub fn clear_node_bootstrap_token(&mut self) {
        if let Some(node) = self.node.as_mut() {
            node.bootstrap_token = None;
        }
    }
}

fn resolve_control_plane(args: &InstallArgs, role: InstallRole) -> Result<ControlPlaneConfig> {
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
    let reseller = if role == InstallRole::AllInOne {
        Some(ResellerConfig {
            tenant_name: non_empty(args.reseller_tenant_name.clone())
                .unwrap_or_else(|| "Default Tenant".into()),
            email: non_empty(args.reseller_email.clone())
                .unwrap_or_else(|| format!("tenant-{}@{domain}", generate_hex(3))),
            display_name: non_empty(args.reseller_display_name.clone())
                .unwrap_or_else(|| "Tenant Admin".into()),
            password: non_empty(args.reseller_password.clone())
                .unwrap_or_else(|| generate_secret(18)),
        })
    } else {
        None
    };
    let starter_subscription = if role == InstallRole::AllInOne {
        Some(StarterSubscriptionConfig {
            name: args
                .starter_subscription_name
                .clone()
                .unwrap_or_else(|| "Starter access".into()),
            traffic_limit_bytes: args
                .starter_subscription_traffic_limit_bytes
                .unwrap_or(1_099_511_627_776),
            days: args.starter_subscription_days.unwrap_or(3650),
        })
    } else {
        None
    };

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
        reseller,
        starter_subscription,
    })
}

fn resolve_node(
    args: &InstallArgs,
    role: InstallRole,
    control_plane: Option<&ControlPlaneConfig>,
) -> Result<NodeConfig> {
    let group_name = if role == InstallRole::AllInOne {
        Some(
            non_empty(args.node_group_name.clone())
                .unwrap_or_else(|| format!("edge-{}", generate_hex(3))),
        )
    } else {
        non_empty(args.node_group_name.clone())
    };
    let name = non_empty(args.agent_name.clone()).unwrap_or_else(|| {
        group_name
            .clone()
            .unwrap_or_else(|| format!("node-{}", generate_hex(3)))
    });
    let server_url = match role {
        InstallRole::AllInOne => control_plane
            .map(|_| LOCAL_CONTROL_PLANE_URL.to_owned())
            .ok_or_else(|| anyhow!("all-in-one install requires control-plane config"))?,
        InstallRole::Node => args
            .agent_server_url
            .clone()
            .and_then(|value| non_empty(Some(value)))
            .ok_or_else(|| anyhow!("node install requires ANNEAL_AGENT_SERVER_URL"))?,
        InstallRole::ControlPlane => bail!("control-plane role does not include node config"),
    };
    validate_node_server_url(role, &server_url)?;

    let bootstrap_token = match role {
        InstallRole::AllInOne => None,
        InstallRole::Node => Some(
            args.agent_bootstrap_token
                .clone()
                .and_then(|value| non_empty(Some(value)))
                .ok_or_else(|| anyhow!("node install requires ANNEAL_AGENT_BOOTSTRAP_TOKEN"))?,
        ),
        InstallRole::ControlPlane => None,
    };
    let engines = parse_engines(
        non_empty(args.agent_engines.clone())
            .as_deref()
            .unwrap_or("xray,singbox"),
    )?;
    let protocols_xray = parse_protocols(
        non_empty(args.agent_protocols_xray.clone())
            .as_deref()
            .unwrap_or(XRAY_DEFAULT_PROTOCOLS),
    )?;
    let protocols_singbox = parse_protocols(
        non_empty(args.agent_protocols_singbox.clone())
            .as_deref()
            .unwrap_or(SINGBOX_DEFAULT_PROTOCOLS),
    )?;

    Ok(NodeConfig {
        server_url,
        bootstrap_token,
        name,
        group_name,
        engines,
        protocols_xray,
        protocols_singbox,
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

pub fn parse_engines(value: &str) -> Result<Vec<ProxyEngine>> {
    let mut engines = Vec::new();
    for item in value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let engine = match item {
            "xray" => ProxyEngine::Xray,
            "singbox" | "sing-box" => ProxyEngine::Singbox,
            other => bail!("unsupported engine: {other}"),
        };
        if !engines.contains(&engine) {
            engines.push(engine);
        }
    }
    if engines.is_empty() {
        bail!("at least one engine is required");
    }
    Ok(engines)
}

pub fn parse_protocols(value: &str) -> Result<Vec<ProtocolKind>> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| match item {
            "vless_reality" => Ok(ProtocolKind::VlessReality),
            "vmess" => Ok(ProtocolKind::Vmess),
            "trojan" => Ok(ProtocolKind::Trojan),
            "shadowsocks_2022" => Ok(ProtocolKind::Shadowsocks2022),
            "tuic" => Ok(ProtocolKind::Tuic),
            "hysteria2" => Ok(ProtocolKind::Hysteria2),
            other => bail!("unsupported protocol: {other}"),
        })
        .collect()
}

pub fn engines_csv(value: &[ProxyEngine]) -> String {
    value
        .iter()
        .map(|engine| match engine {
            ProxyEngine::Xray => "xray",
            ProxyEngine::Singbox => "singbox",
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub fn protocols_csv(value: &[ProtocolKind]) -> String {
    value
        .iter()
        .map(|protocol| match protocol {
            ProtocolKind::VlessReality => "vless_reality",
            ProtocolKind::Vmess => "vmess",
            ProtocolKind::Trojan => "trojan",
            ProtocolKind::Shadowsocks2022 => "shadowsocks_2022",
            ProtocolKind::Tuic => "tuic",
            ProtocolKind::Hysteria2 => "hysteria2",
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn validate_https_url(value: &str) -> Result<()> {
    let url = Url::parse(value).context("failed to parse URL")?;
    if url.scheme() != "https" {
        bail!("URL must use https");
    }
    Ok(())
}

fn validate_node_server_url(role: InstallRole, value: &str) -> Result<()> {
    match role {
        InstallRole::AllInOne => {
            let url = Url::parse(value).context("failed to parse URL")?;
            if url.scheme() != "http" {
                bail!("all-in-one agent URL must use http");
            }
            Ok(())
        }
        InstallRole::Node => validate_https_url(value),
        InstallRole::ControlPlane => bail!("control-plane role does not include node config"),
    }
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
            agent_server_url: None,
            agent_name: None,
            node_group_name: None,
            agent_engines: None,
            agent_protocols_xray: None,
            agent_protocols_singbox: None,
            agent_bootstrap_token: None,
            starter_subscription_name: None,
            starter_subscription_traffic_limit_bytes: None,
            starter_subscription_days: None,
            non_interactive: true,
        }
    }

    #[test]
    fn control_plane_defaults_generate_valid_critical_values() {
        let config = InstallConfig::from_args(base_args()).expect("config");
        let control_plane = config.control_plane.expect("control-plane");

        assert_eq!(config.role, InstallRole::ControlPlane);
        assert_eq!(config.deployment_mode, DeploymentMode::Native);
        assert_eq!(control_plane.domain, "panel.example.com");
        assert_eq!(control_plane.superadmin.display_name, "Superadmin");
        assert_eq!(control_plane.panel_path.len(), 48);
        assert!(
            control_plane
                .panel_path
                .chars()
                .all(|char| char.is_ascii_hexdigit())
        );
        assert_eq!(
            control_plane.public_base_url,
            format!("https://panel.example.com/{}", control_plane.panel_path)
        );
        assert!(
            control_plane
                .superadmin
                .email
                .ends_with("@panel.example.com")
        );
        assert_eq!(control_plane.superadmin.password.len(), 18);
        assert_eq!(
            control_plane.bootstrap_token.as_deref().map(str::len),
            Some(24)
        );
        assert_eq!(control_plane.data_encryption_key.len(), 64);
        assert!(
            control_plane
                .data_encryption_key
                .chars()
                .all(|char| char.is_ascii_hexdigit())
        );
        assert_eq!(control_plane.token_hash_key.len(), 64);
        assert_eq!(control_plane.access_jwt_secret.len(), 64);
        assert_eq!(control_plane.pre_auth_jwt_secret.len(), 64);
        assert!(control_plane.reseller.is_none());
        assert!(control_plane.starter_subscription.is_none());
        assert!(config.node.is_none());

        let database_url = Url::parse(&control_plane.database_url).expect("database url");
        assert_eq!(database_url.scheme(), "postgres");
        assert_eq!(database_url.host_str(), Some("127.0.0.1"));
        assert_eq!(database_url.port(), Some(5432));
        assert!(!database_url.username().is_empty());
        assert!(database_url.password().is_some());
        assert!(!database_url.path().trim_matches('/').is_empty());
    }

    #[test]
    fn public_url_hydrates_domain_and_panel_path() {
        let mut args = base_args();
        args.domain = None;
        args.public_base_url = Some("https://panel.example.com/private-path".into());

        let config = InstallConfig::from_args(args).expect("config");
        let control_plane = config.control_plane.expect("control-plane");

        assert_eq!(control_plane.domain, "panel.example.com");
        assert_eq!(control_plane.panel_path, "private-path");
        assert_eq!(
            control_plane.public_base_url,
            "https://panel.example.com/private-path"
        );
    }

    #[test]
    fn all_in_one_defaults_generate_reseller_node_and_subscription() {
        let mut args = base_args();
        args.role = Some(InstallRole::AllInOne);

        let config = InstallConfig::from_args(args).expect("config");
        let control_plane = config.control_plane.expect("control-plane");
        let reseller = control_plane.reseller.expect("reseller");
        let starter = control_plane
            .starter_subscription
            .expect("starter subscription");
        let node = config.node.expect("node");

        assert_eq!(reseller.tenant_name, "Default Tenant");
        assert_eq!(reseller.display_name, "Tenant Admin");
        assert!(reseller.email.ends_with("@panel.example.com"));
        assert_eq!(reseller.password.len(), 18);
        assert_eq!(starter.name, "Starter access");
        assert_eq!(starter.traffic_limit_bytes, 1_099_511_627_776);
        assert_eq!(starter.days, 3650);
        assert_eq!(node.server_url, "http://127.0.0.1:8080");
        assert!(node.bootstrap_token.is_none());
        assert_eq!(node.engines, vec![ProxyEngine::Xray, ProxyEngine::Singbox]);
        assert_eq!(
            node.protocols_xray,
            parse_protocols(XRAY_DEFAULT_PROTOCOLS).expect("xray")
        );
        assert_eq!(
            node.protocols_singbox,
            parse_protocols(SINGBOX_DEFAULT_PROTOCOLS).expect("singbox")
        );
        let group_name = node.group_name.expect("group name");
        assert!(group_name.starts_with("edge-"));
        assert_eq!(node.name, group_name);
    }

    #[test]
    fn node_install_requires_bootstrap_token_non_interactive() {
        let mut args = base_args();
        args.role = Some(InstallRole::Node);
        args.domain = None;
        args.agent_server_url = Some("https://panel.example.com/private".into());

        let error = InstallConfig::from_args(args).expect_err("must fail");
        assert!(
            error.to_string().contains("ANNEAL_AGENT_BOOTSTRAP_TOKEN"),
            "{error}"
        );
    }

    #[test]
    fn node_install_rejects_non_https_agent_server_url() {
        let mut args = base_args();
        args.role = Some(InstallRole::Node);
        args.domain = None;
        args.agent_server_url = Some("http://panel.example.com/private".into());
        args.agent_bootstrap_token = Some("bootstrap-token".into());

        let error = InstallConfig::from_args(args).expect_err("must fail");
        assert!(error.to_string().contains("https"), "{error}");
    }
}
