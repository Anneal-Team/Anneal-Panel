mod client;
mod runtime;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::anyhow;
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::{
    client::{RegisterNodeRequest, ack_rollout, heartbeat, pull_rollouts, register},
    runtime::{
        RuntimeSettings, apply_rollout, parse_engine, parse_protocols, parse_runtime_controller,
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
    #[arg(long, env = "ANNEAL_AGENT_ENGINE", default_value = "xray")]
    engine: String,
    #[arg(long, env = "ANNEAL_AGENT_ENGINES")]
    engines: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_PROTOCOLS", default_value = "vless_reality")]
    protocols: String,
    #[arg(long, env = "ANNEAL_AGENT_PROTOCOLS_XRAY")]
    xray_protocols: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_PROTOCOLS_SINGBOX")]
    singbox_protocols: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_ENROLLMENT_TOKEN")]
    enrollment_token: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_ENROLLMENT_TOKENS")]
    enrollment_tokens: Option<String>,
    #[arg(long, env = "ANNEAL_AGENT_NODE_ID")]
    node_id: Option<Uuid>,
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
    node_id: Uuid,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AgentState {
    node_ids: HashMap<String, Uuid>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = AgentArgs::parse();
    let client = Client::new();
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
    let mut enrollment_tokens = parse_enrollment_tokens(&args)?;
    let mut state = load_state(&state_path).await?;
    let runtimes = register_runtimes(
        &client,
        &args,
        &configured_engines,
        &mut enrollment_tokens,
        &mut state,
    )
    .await?;
    store_state(&state_path, &state).await?;

    loop {
        for managed in &runtimes {
            heartbeat(&client, &args.server_url, managed.node_id, &args.version).await?;
            let rollouts = pull_rollouts(&client, &args.server_url, managed.node_id).await?;
            for rollout in rollouts {
                let result = apply_rollout(&runtime, &rollout).await;
                ack_rollout(
                    &client,
                    &args.server_url,
                    managed.node_id,
                    rollout.id,
                    result.is_ok(),
                    result.err().map(|error| error.to_string()),
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
    enrollment_tokens: &mut HashMap<String, String>,
    state: &mut AgentState,
) -> anyhow::Result<Vec<ManagedRuntime>> {
    let legacy_engine = parse_engine(&args.engine)?;
    let legacy_mode = args.engines.is_none();
    let mut runtimes = Vec::with_capacity(configured_engines.len());

    for engine in configured_engines {
        let key = engine_key(*engine);
        let protocols = protocols_for_engine(args, *engine)?;
        let persisted_node_id = state.node_ids.get(key).copied().or_else(|| {
            if legacy_mode && *engine == legacy_engine {
                args.node_id
            } else {
                None
            }
        });
        let node_id = if let Some(node_id) = persisted_node_id {
            node_id
        } else {
            let enrollment_token = enrollment_tokens.remove(key).or_else(|| {
                if legacy_mode && *engine == legacy_engine {
                    args.enrollment_token.clone()
                } else {
                    None
                }
            });
            let enrollment_token = enrollment_token
                .ok_or_else(|| anyhow!("missing enrollment token for runtime {key}"))?;
            let name = if configured_engines.len() == 1 {
                args.name.clone()
            } else {
                format!("{}-{key}", args.name)
            };
            let node_id = register(
                client,
                &args.server_url,
                RegisterNodeRequest {
                    enrollment_token,
                    name,
                    version: args.version.clone(),
                    engine: *engine,
                    protocols: protocols.clone(),
                },
            )
            .await?;
            state.node_ids.insert(key.into(), node_id);
            node_id
        };
        runtimes.push(ManagedRuntime { node_id });
    }

    Ok(runtimes)
}

fn parse_engines(args: &AgentArgs) -> anyhow::Result<Vec<anneal_core::ProxyEngine>> {
    if let Some(value) = &args.engines {
        let mut parsed = Vec::new();
        for item in value
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
        return Ok(parsed);
    }
    Ok(vec![parse_engine(&args.engine)?])
}

fn protocols_for_engine(
    args: &AgentArgs,
    engine: anneal_core::ProxyEngine,
) -> anyhow::Result<Vec<anneal_core::ProtocolKind>> {
    let explicit = match engine {
        anneal_core::ProxyEngine::Xray => args.xray_protocols.as_deref(),
        anneal_core::ProxyEngine::Singbox => args.singbox_protocols.as_deref(),
    };
    if let Some(value) = explicit {
        return parse_protocols(value);
    }
    if args.engines.is_some() {
        return parse_protocols(match engine {
            anneal_core::ProxyEngine::Xray => XRAY_DEFAULT_PROTOCOLS,
            anneal_core::ProxyEngine::Singbox => SINGBOX_DEFAULT_PROTOCOLS,
        });
    }
    parse_protocols(&args.protocols)
}

fn parse_enrollment_tokens(args: &AgentArgs) -> anyhow::Result<HashMap<String, String>> {
    let mut tokens = HashMap::new();
    if let Some(value) = &args.enrollment_tokens {
        for item in value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            let (engine, token) = item
                .split_once(':')
                .ok_or_else(|| anyhow!("invalid enrollment token pair: {item}"))?;
            let key = engine_key(parse_engine(engine)?);
            let token = token.trim();
            if token.is_empty() {
                return Err(anyhow!("empty enrollment token for runtime {key}"));
            }
            tokens.insert(key.into(), token.into());
        }
    }
    Ok(tokens)
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
