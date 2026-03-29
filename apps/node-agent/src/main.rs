mod client;
mod runtime;

use std::{
    collections::HashMap,
    net::IpAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::anyhow;
use clap::Parser;
use reqwest::Client;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::{
    client::{
        BootstrapAgentRequest, RegisterRuntimeRequest, RuntimeIdentity, ack_rollout, bootstrap,
        build_client, heartbeat, pull_rollouts,
    },
    runtime::{
        RuntimeSettings, apply_rollout, ensure_runtime_running, parse_engine, parse_protocols,
        parse_runtime_controller,
    },
};

const XRAY_DEFAULT_PROTOCOLS: &str = "vless_reality,vmess,trojan,shadowsocks_2022";
const SINGBOX_DEFAULT_PROTOCOLS: &str =
    "vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2";

#[derive(Debug, Parser)]
struct AgentArgs {
    #[arg(long, env = "ANNEAL_AGENT_SERVER_URL")]
    server_url: String,
    #[arg(long, env = "ANNEAL_AGENT_NAME")]
    name: String,
    #[arg(long, env = "ANNEAL_AGENT_VERSION", default_value = "0.1.0")]
    version: String,
    #[arg(long, env = "ANNEAL_AGENT_ENGINES", default_value = "xray,singbox")]
    engines: String,
    #[arg(long, env = "ANNEAL_AGENT_PROTOCOLS_XRAY")]
    xray_protocols: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_PROTOCOLS_SINGBOX")]
    singbox_protocols: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_BOOTSTRAP_TOKEN")]
    bootstrap_token: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_NODE_STATE_PATH")]
    node_state_path: Option<PathBuf>,
    #[arg(
        long,
        env = "ANNEAL_AGENT_CONFIG_ROOT",
        default_value = "/var/lib/anneal"
    )]
    config_root: PathBuf,
    #[arg(long, env = "ANNEAL_AGENT_INTERVAL_SECONDS", default_value_t = 30)]
    interval_seconds: u64,
    #[arg(long, env = "ANNEAL_AGENT_ONCE", default_value_t = false)]
    once: bool,
    #[arg(
        long,
        env = "ANNEAL_AGENT_XRAY_BINARY",
        default_value = "/opt/anneal/bin/xray"
    )]
    xray_binary: PathBuf,
    #[arg(
        long,
        env = "ANNEAL_AGENT_SINGBOX_BINARY",
        default_value = "/opt/anneal/bin/hiddify-core"
    )]
    singbox_binary: PathBuf,
    #[arg(
        long,
        env = "ANNEAL_AGENT_RUNTIME_CONTROLLER",
        default_value = "systemctl"
    )]
    runtime_controller: String,
    #[arg(
        long,
        env = "ANNEAL_AGENT_SYSTEMCTL_BINARY",
        default_value = "/usr/bin/systemctl"
    )]
    systemctl_binary: PathBuf,
    #[arg(
        long,
        env = "ANNEAL_AGENT_XRAY_SERVICE",
        default_value = "anneal-xray.service"
    )]
    xray_service: String,
    #[arg(
        long,
        env = "ANNEAL_AGENT_SINGBOX_SERVICE",
        default_value = "anneal-singbox.service"
    )]
    singbox_service: String,
    #[arg(long, env = "ANNEAL_AGENT_SKIP_RESTART", default_value_t = false)]
    skip_restart: bool,
}

#[derive(Debug, Clone)]
struct ManagedRuntime {
    identity: RuntimeIdentity,
    engine: anneal_core::ProxyEngine,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AgentState {
    runtimes: HashMap<String, StoredRuntimeState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRuntimeState {
    node_id: Uuid,
    node_token: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = AgentArgs::parse();
    validate_server_url(&args.server_url)?;
    let client = build_client()?;
    let runtime = RuntimeSettings {
        config_root: args.config_root.clone(),
        xray_binary: args.xray_binary.clone(),
        singbox_binary: args.singbox_binary.clone(),
        runtime_controller: parse_runtime_controller(&args.runtime_controller)?,
        systemctl_binary: args.systemctl_binary.clone(),
        xray_service: args.xray_service.clone(),
        singbox_service: args.singbox_service.clone(),
        skip_restart: args.skip_restart,
    };
    fs::create_dir_all(&runtime.config_root).await?;

    let state_path = args
        .node_state_path
        .clone()
        .unwrap_or_else(|| runtime.config_root.join("agent-state.json"));
    let configured_engines = parse_engines(&args)?;
    let mut state = load_state(&state_path).await?;
    let runtimes = register_runtimes(&client, &args, &configured_engines, &mut state).await?;
    store_state(&state_path, &state).await?;

    loop {
        for managed in &runtimes {
            if let Err(error) = ensure_runtime_running(&runtime, managed.engine).await {
                eprintln!("failed ensuring runtime {:?}: {error:#}", managed.engine);
            }
            heartbeat(&client, &args.server_url, &managed.identity, &args.version).await?;
            let rollouts = pull_rollouts(&client, &args.server_url, &managed.identity).await?;
            for rollout in rollouts {
                let result = apply_rollout(&runtime, &rollout).await;
                ack_rollout(
                    &client,
                    &args.server_url,
                    &managed.identity,
                    rollout.id,
                    result.is_ok(),
                    result
                        .err()
                        .map(|error| classify_rollout_error(&error).to_owned()),
                )
                .await?;
            }
        }
        if args.once {
            break;
        }
        tokio::time::sleep(Duration::from_secs(args.interval_seconds)).await;
    }

    Ok(())
}

async fn register_runtimes(
    client: &Client,
    args: &AgentArgs,
    configured_engines: &[anneal_core::ProxyEngine],
    state: &mut AgentState,
) -> anyhow::Result<Vec<ManagedRuntime>> {
    let mut runtimes = Vec::with_capacity(configured_engines.len());
    let mut pending_bootstrap = Vec::new();

    for engine in configured_engines {
        let key = engine_key(*engine);
        let protocols = protocols_for_engine(args, *engine)?;
        let persisted_identity = state.runtimes.get(key).cloned();
        if let Some(identity) = persisted_identity {
            if identity.node_token.trim().is_empty() {
                return Err(anyhow!("missing node token for persisted runtime {key}"));
            }
            runtimes.push(ManagedRuntime {
                identity: RuntimeIdentity {
                    node_id: identity.node_id,
                    node_token: identity.node_token,
                },
                engine: *engine,
            });
        } else {
            let name = if configured_engines.len() == 1 {
                args.name.clone()
            } else {
                format!("{}-{key}", args.name)
            };
            pending_bootstrap.push(RegisterRuntimeRequest {
                name,
                version: args.version.clone(),
                engine: *engine,
                protocols,
            });
        }
    }

    if !pending_bootstrap.is_empty() {
        let bootstrap_token = args
            .bootstrap_token
            .clone()
            .ok_or_else(|| anyhow!("missing ANNEAL_AGENT_BOOTSTRAP_TOKEN"))?;
        let grants = bootstrap(
            client,
            &args.server_url,
            BootstrapAgentRequest {
                bootstrap_token,
                runtimes: pending_bootstrap,
            },
        )
        .await?;
        for grant in grants {
            let key = engine_key(grant.engine).to_owned();
            let stored = StoredRuntimeState {
                node_id: grant.node_id,
                node_token: grant.node_token.clone(),
            };
            state.runtimes.insert(key, stored.clone());
            runtimes.push(ManagedRuntime {
                identity: RuntimeIdentity {
                    node_id: stored.node_id,
                    node_token: stored.node_token,
                },
                engine: grant.engine,
            });
        }
    }

    Ok(runtimes)
}

fn parse_engines(args: &AgentArgs) -> anyhow::Result<Vec<anneal_core::ProxyEngine>> {
    let mut parsed = Vec::new();
    for item in args
        .engines
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let engine = parse_engine(item)?;
        if !parsed.contains(&engine) {
            parsed.push(engine);
        }
    }
    if parsed.is_empty() {
        return Err(anyhow!(
            "ANNEAL_AGENT_ENGINES must contain at least one runtime"
        ));
    }
    Ok(parsed)
}

fn protocols_for_engine(
    args: &AgentArgs,
    engine: anneal_core::ProxyEngine,
) -> anyhow::Result<Vec<anneal_core::ProtocolKind>> {
    if let Some(value) = match engine {
        anneal_core::ProxyEngine::Xray => args.xray_protocols.as_deref(),
        anneal_core::ProxyEngine::Singbox => args.singbox_protocols.as_deref(),
    } {
        return parse_protocols(value);
    }
    parse_protocols(match engine {
        anneal_core::ProxyEngine::Xray => XRAY_DEFAULT_PROTOCOLS,
        anneal_core::ProxyEngine::Singbox => SINGBOX_DEFAULT_PROTOCOLS,
    })
}

async fn load_state(path: &Path) -> anyhow::Result<AgentState> {
    match fs::read_to_string(path).await {
        Ok(raw) => Ok(serde_json::from_str::<AgentState>(&raw)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(AgentState::default()),
        Err(error) => Err(error.into()),
    }
}

async fn store_state(path: &Path, state: &AgentState) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, serde_json::to_vec_pretty(state)?).await?;
    Ok(())
}

fn engine_key(engine: anneal_core::ProxyEngine) -> &'static str {
    match engine {
        anneal_core::ProxyEngine::Xray => "xray",
        anneal_core::ProxyEngine::Singbox => "singbox",
    }
}

fn validate_server_url(server_url: &str) -> anyhow::Result<()> {
    let url = Url::parse(server_url)?;
    if url.scheme() == "https" {
        return Ok(());
    }
    if url.scheme() == "http" && is_loopback_host(&url) {
        return Ok(());
    }
    Err(anyhow!("ANNEAL_AGENT_SERVER_URL must use https"))
}

fn is_loopback_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let normalized = host.trim_matches(['[', ']']);
    normalized.eq_ignore_ascii_case("localhost")
        || normalized
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn classify_rollout_error(error: &anyhow::Error) -> &'static str {
    let message = error.to_string();
    if message.contains("validation") || message.contains("valid json") {
        return "config_invalid";
    }
    if message.contains("restart") {
        return "restart_failed";
    }
    if message.contains("health-check") {
        return "healthcheck_failed";
    }
    if message.contains("rollback") || message.contains("restoring backup") {
        return "rollback_failed";
    }
    "restart_failed"
}

#[cfg(test)]
mod tests {
    use super::{classify_rollout_error, validate_server_url};

    #[test]
    fn rejects_non_https_control_plane_url() {
        let error = validate_server_url("http://panel.example.com").expect_err("must fail");
        assert!(error.to_string().contains("https"));
    }

    #[test]
    fn accepts_loopback_http_control_plane_url() {
        validate_server_url("http://127.0.0.1:8080").expect("loopback http must pass");
        validate_server_url("http://localhost:8080").expect("localhost http must pass");
        validate_server_url("http://[::1]:8080").expect("ipv6 loopback http must pass");
    }

    #[test]
    fn rollout_errors_are_redacted_to_codes() {
        assert_eq!(
            classify_rollout_error(&anyhow::anyhow!("runtime validation failed: stderr")),
            "config_invalid"
        );
        assert_eq!(
            classify_rollout_error(&anyhow::anyhow!("health-check failed for xray")),
            "healthcheck_failed"
        );
    }
}
