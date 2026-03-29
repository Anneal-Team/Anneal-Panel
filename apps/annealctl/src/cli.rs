use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::config::{DeploymentMode, InstallRole};
use crate::i18n::Language;

#[derive(Debug, Parser)]
#[command(name = "annealctl")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Install(InstallArgs),
    Resume(ResumeArgs),
    Status,
    Doctor,
    Restart,
    Update(UpdateArgs),
    Uninstall(UninstallArgs),
    Manage,
}

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    #[arg(long, env = "ANNEAL_BUNDLE_ROOT")]
    pub bundle_root: Option<PathBuf>,
    #[arg(long, env = "ANNEAL_LANG", value_enum)]
    pub lang: Option<Language>,
    #[arg(long, env = "ANNEAL_ROLE", value_enum)]
    pub role: Option<InstallRole>,
    #[arg(
        long = "mode",
        alias = "deployment-mode",
        env = "ANNEAL_DEPLOYMENT_MODE",
        value_enum
    )]
    pub deployment_mode: Option<DeploymentMode>,
    #[arg(long, env = "ANNEAL_DOMAIN")]
    pub domain: Option<String>,
    #[arg(long, env = "ANNEAL_PANEL_PATH")]
    pub panel_path: Option<String>,
    #[arg(long, env = "ANNEAL_PUBLIC_BASE_URL")]
    pub public_base_url: Option<String>,
    #[arg(long, env = "ANNEAL_DATABASE_URL")]
    pub database_url: Option<String>,
    #[arg(long, env = "ANNEAL_OTLP_ENDPOINT")]
    pub otlp_endpoint: Option<String>,
    #[arg(long, env = "ANNEAL_BOOTSTRAP_TOKEN")]
    pub bootstrap_token: Option<String>,
    #[arg(long, env = "ANNEAL_DATA_ENCRYPTION_KEY")]
    pub data_encryption_key: Option<String>,
    #[arg(long, env = "ANNEAL_TOKEN_HASH_KEY")]
    pub token_hash_key: Option<String>,
    #[arg(long, env = "ANNEAL_ACCESS_JWT_SECRET")]
    pub access_jwt_secret: Option<String>,
    #[arg(long, env = "ANNEAL_PRE_AUTH_JWT_SECRET")]
    pub pre_auth_jwt_secret: Option<String>,
    #[arg(long, env = "ANNEAL_SUPERADMIN_EMAIL")]
    pub superadmin_email: Option<String>,
    #[arg(
        long,
        env = "ANNEAL_SUPERADMIN_DISPLAY_NAME",
        default_value = "Superadmin"
    )]
    pub superadmin_display_name: String,
    #[arg(long, env = "ANNEAL_SUPERADMIN_PASSWORD")]
    pub superadmin_password: Option<String>,
    #[arg(long, env = "ANNEAL_RESELLER_TENANT_NAME")]
    pub reseller_tenant_name: Option<String>,
    #[arg(long, env = "ANNEAL_RESELLER_EMAIL")]
    pub reseller_email: Option<String>,
    #[arg(long, env = "ANNEAL_RESELLER_DISPLAY_NAME")]
    pub reseller_display_name: Option<String>,
    #[arg(long, env = "ANNEAL_RESELLER_PASSWORD")]
    pub reseller_password: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_SERVER_URL")]
    pub agent_server_url: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_NAME")]
    pub agent_name: Option<String>,
    #[arg(long, env = "ANNEAL_NODE_GROUP_NAME")]
    pub node_group_name: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_ENGINES")]
    pub agent_engines: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_PROTOCOLS_XRAY")]
    pub agent_protocols_xray: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_PROTOCOLS_SINGBOX")]
    pub agent_protocols_singbox: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_BOOTSTRAP_TOKEN")]
    pub agent_bootstrap_token: Option<String>,
    #[arg(long, env = "ANNEAL_STARTER_SUBSCRIPTION_NAME")]
    pub starter_subscription_name: Option<String>,
    #[arg(long, env = "ANNEAL_STARTER_SUBSCRIPTION_TRAFFIC_LIMIT_BYTES")]
    pub starter_subscription_traffic_limit_bytes: Option<i64>,
    #[arg(long, env = "ANNEAL_STARTER_SUBSCRIPTION_DAYS")]
    pub starter_subscription_days: Option<i64>,
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ResumeArgs {
    #[arg(long, env = "ANNEAL_BUNDLE_ROOT")]
    pub bundle_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct UpdateArgs {
    #[arg(long, env = "ANNEAL_BUNDLE_ROOT")]
    pub bundle_root: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct UninstallArgs {
    #[arg(long, default_value_t = false)]
    pub keep_data: bool,
    #[arg(long, default_value_t = false)]
    pub keep_database: bool,
}
