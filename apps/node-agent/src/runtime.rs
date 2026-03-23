use std::net::IpAddr;
use std::path::{Component, Path, PathBuf};

use anneal_core::{ProtocolKind, ProxyEngine};
use anneal_nodes::DeploymentRollout;
use anyhow::{Context, anyhow};
use tokio::{
    fs,
    net::{TcpStream, UdpSocket},
    process::Command,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeController {
    Systemctl,
    Supervisorctl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HealthTargetKind {
    Tcp,
    Udp,
}

#[derive(Debug, Clone)]
struct HealthTarget {
    host: String,
    port: u16,
    kind: HealthTargetKind,
}

#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub config_root: PathBuf,
    pub xray_binary: PathBuf,
    pub singbox_binary: PathBuf,
    pub runtime_controller: RuntimeController,
    pub systemctl_binary: PathBuf,
    pub xray_service: String,
    pub singbox_service: String,
    pub skip_restart: bool,
}

pub async fn apply_rollout(
    settings: &RuntimeSettings,
    rollout: &DeploymentRollout,
) -> anyhow::Result<()> {
    let target_path = resolve_target_path(&settings.config_root, &rollout.target_path)?;
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let candidate_path = decorate_path(&target_path, "candidate");
    let backup_path = decorate_path(&target_path, "previous");
    fs::write(&candidate_path, &rollout.rendered_config)
        .await
        .with_context(|| format!("failed writing candidate {}", candidate_path.display()))?;
    validate_candidate(settings, rollout.engine, &candidate_path).await?;
    let backup_exists = try_backup_current(&target_path, &backup_path).await?;
    fs::rename(&candidate_path, &target_path)
        .await
        .with_context(|| format!("failed promoting candidate to {}", target_path.display()))?;
    if let Err(error) = restart_runtime(settings, rollout.engine).await {
        if backup_exists {
            fs::copy(&backup_path, &target_path)
                .await
                .with_context(|| format!("failed restoring backup {}", backup_path.display()))?;
            restart_runtime(settings, rollout.engine)
                .await
                .context("restart failed after rollback")?;
        }
        return Err(error);
    }
    if let Err(error) =
        health_check_runtime(settings, rollout.engine, &rollout.rendered_config).await
    {
        if backup_exists {
            fs::copy(&backup_path, &target_path)
                .await
                .with_context(|| format!("failed restoring backup {}", backup_path.display()))?;
            restart_runtime(settings, rollout.engine)
                .await
                .context("restart failed after rollback")?;
        }
        return Err(error);
    }
    Ok(())
}

pub fn parse_engine(value: &str) -> anyhow::Result<ProxyEngine> {
    match value {
        "xray" => Ok(ProxyEngine::Xray),
        "singbox" | "sing-box" => Ok(ProxyEngine::Singbox),
        _ => Err(anyhow!("unsupported engine: {value}")),
    }
}

pub fn parse_protocols(value: &str) -> anyhow::Result<Vec<ProtocolKind>> {
    value
        .split(',')
        .map(|item| match item.trim() {
            "vless_reality" => Ok(ProtocolKind::VlessReality),
            "vmess" => Ok(ProtocolKind::Vmess),
            "trojan" => Ok(ProtocolKind::Trojan),
            "shadowsocks_2022" => Ok(ProtocolKind::Shadowsocks2022),
            "tuic" => Ok(ProtocolKind::Tuic),
            "hysteria2" => Ok(ProtocolKind::Hysteria2),
            other => Err(anyhow!("unsupported protocol: {other}")),
        })
        .collect()
}

pub fn parse_runtime_controller(value: &str) -> anyhow::Result<RuntimeController> {
    match value {
        "systemctl" => Ok(RuntimeController::Systemctl),
        "supervisorctl" | "supervisor" => Ok(RuntimeController::Supervisorctl),
        _ => Err(anyhow!("unsupported runtime controller: {value}")),
    }
}

fn resolve_target_path(config_root: &Path, target_path: &str) -> anyhow::Result<PathBuf> {
    let target = target_path.trim();
    if target.is_empty() {
        return Err(anyhow!("rollout target path is required"));
    }

    let raw = Path::new(target);
    if raw.is_absolute() {
        return Err(anyhow!("rollout target path must be relative"));
    }

    let mut resolved = PathBuf::from(config_root);
    for component in raw.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("rollout target path must not escape config root"));
            }
        }
    }

    Ok(resolved)
}

fn decorate_path(target_path: &Path, suffix: &str) -> PathBuf {
    let extension = target_path.extension().and_then(|value| value.to_str());
    let stem = target_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("config");
    let decorated_name = match extension {
        Some(extension) if !extension.is_empty() => format!("{stem}.{suffix}.{extension}"),
        _ => format!("{stem}.{suffix}"),
    };
    match target_path.parent() {
        Some(parent) => parent.join(decorated_name),
        None => PathBuf::from(decorated_name),
    }
}

async fn try_backup_current(target_path: &Path, backup_path: &Path) -> anyhow::Result<bool> {
    match fs::metadata(target_path).await {
        Ok(_) => {
            fs::copy(target_path, backup_path)
                .await
                .with_context(|| format!("failed backing up {}", target_path.display()))?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

async fn validate_candidate(
    settings: &RuntimeSettings,
    engine: ProxyEngine,
    candidate_path: &Path,
) -> anyhow::Result<()> {
    serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(candidate_path)
            .await
            .with_context(|| format!("failed reading {}", candidate_path.display()))?,
    )
    .context("rendered config is not valid json")?;
    let mut command = match engine {
        ProxyEngine::Xray => {
            let mut command = Command::new(&settings.xray_binary);
            command
                .arg("run")
                .arg("-test")
                .arg("-c")
                .arg(candidate_path);
            command
        }
        ProxyEngine::Singbox => {
            let mut command = Command::new(&settings.singbox_binary);
            let built_path = decorate_path(candidate_path, "built");
            command
                .arg("build")
                .arg("-c")
                .arg(candidate_path)
                .arg("-o")
                .arg(&built_path);
            command
        }
    };
    let output = command.output().await.with_context(|| {
        format!(
            "failed to start {} validation binary",
            runtime_binary(settings, engine).display()
        )
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "runtime validation failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

async fn restart_runtime(settings: &RuntimeSettings, engine: ProxyEngine) -> anyhow::Result<()> {
    if settings.skip_restart {
        return Ok(());
    }
    let service = runtime_service(settings, engine);
    let output = match settings.runtime_controller {
        RuntimeController::Systemctl => Command::new(&settings.systemctl_binary)
            .arg("restart")
            .arg(service)
            .output()
            .await
            .with_context(|| format!("failed to start systemctl for {service}"))?,
        RuntimeController::Supervisorctl => Command::new(&settings.systemctl_binary)
            .arg("restart")
            .arg(service)
            .output()
            .await
            .with_context(|| format!("failed to start supervisorctl for {service}"))?,
    };
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "failed restarting {}: {}",
        service,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

async fn health_check_runtime(
    settings: &RuntimeSettings,
    engine: ProxyEngine,
    rendered_config: &str,
) -> anyhow::Result<()> {
    if settings.skip_restart {
        return Ok(());
    }
    let service = runtime_service(settings, engine);
    let checks = extract_health_targets(rendered_config)?;
    for _ in 0..3 {
        if is_service_active(settings, service).await? && check_targets(&checks).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    Err(anyhow!("health-check failed for {service}"))
}

async fn is_service_active(settings: &RuntimeSettings, service: &str) -> anyhow::Result<bool> {
    let output = match settings.runtime_controller {
        RuntimeController::Systemctl => Command::new(&settings.systemctl_binary)
            .arg("is-active")
            .arg(service)
            .output()
            .await
            .with_context(|| format!("failed to query systemctl is-active for {service}"))?,
        RuntimeController::Supervisorctl => Command::new(&settings.systemctl_binary)
            .arg("status")
            .arg(service)
            .output()
            .await
            .with_context(|| format!("failed to query supervisorctl status for {service}"))?,
    };
    Ok(match settings.runtime_controller {
        RuntimeController::Systemctl => {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "active"
        }
        RuntimeController::Supervisorctl => supervisor_service_is_running(&output.stdout),
    })
}

fn extract_health_targets(rendered_config: &str) -> anyhow::Result<Vec<HealthTarget>> {
    let value = serde_json::from_str::<serde_json::Value>(rendered_config)
        .context("invalid rendered config json for health-check")?;
    let mut targets = Vec::new();
    if let Some(inbounds) = value.get("inbounds").and_then(serde_json::Value::as_array) {
        for inbound in inbounds {
            let port = inbound
                .get("port")
                .or_else(|| inbound.get("listen_port"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u16::try_from(value).ok());
            let listen = inbound
                .get("listen")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("127.0.0.1");
            let kind = match inbound
                .get("protocol")
                .or_else(|| inbound.get("type"))
                .and_then(serde_json::Value::as_str)
            {
                Some("tuic" | "hysteria2") => HealthTargetKind::Udp,
                _ => HealthTargetKind::Tcp,
            };
            if let Some(port) = port {
                targets.push(HealthTarget {
                    host: normalize_health_host(listen)?,
                    port,
                    kind,
                });
            }
        }
    }
    Ok(targets)
}

fn normalize_health_host(value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    match value {
        "" | "0.0.0.0" | "::" | "127.0.0.1" | "::1" | "localhost" => Ok("127.0.0.1".into()),
        other => match other.parse::<IpAddr>() {
            Ok(address) if address.is_loopback() => Ok("127.0.0.1".into()),
            _ => Err(anyhow!("health-check target must stay on loopback")),
        },
    }
}

async fn check_targets(targets: &[HealthTarget]) -> bool {
    for target in targets {
        let ok = match target.kind {
            HealthTargetKind::Tcp => check_tcp_target(&target.host, target.port).await,
            HealthTargetKind::Udp => check_udp_target(&target.host, target.port).await,
        };
        if !ok {
            return false;
        }
    }
    true
}

async fn check_tcp_target(host: &str, port: u16) -> bool {
    let address = format!("{host}:{port}");
    let result = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(&address)).await;
    result.is_ok() && result.ok().and_then(Result::ok).is_some()
}

async fn check_udp_target(host: &str, port: u16) -> bool {
    UdpSocket::bind((host, port)).await.is_err()
}

fn runtime_binary(settings: &RuntimeSettings, engine: ProxyEngine) -> &Path {
    match engine {
        ProxyEngine::Xray => &settings.xray_binary,
        ProxyEngine::Singbox => &settings.singbox_binary,
    }
}

fn runtime_service(settings: &RuntimeSettings, engine: ProxyEngine) -> &str {
    match engine {
        ProxyEngine::Xray => &settings.xray_service,
        ProxyEngine::Singbox => &settings.singbox_service,
    }
}

fn supervisor_service_is_running(stdout: &[u8]) -> bool {
    String::from_utf8_lossy(stdout).contains(" RUNNING ")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        decorate_path, extract_health_targets, normalize_health_host, parse_runtime_controller,
        resolve_target_path, supervisor_service_is_running,
    };

    #[test]
    fn decorated_path_preserves_json_extension() {
        let target = Path::new("/var/lib/anneal/xray/config.json");
        assert_eq!(
            decorate_path(target, "candidate"),
            Path::new("/var/lib/anneal/xray/config.candidate.json")
        );
    }

    #[test]
    fn decorated_path_handles_extensionless_target() {
        let target = Path::new("/var/lib/anneal/runtime/config");
        assert_eq!(
            decorate_path(target, "previous"),
            Path::new("/var/lib/anneal/runtime/config.previous")
        );
    }

    #[test]
    fn resolve_target_path_rejects_parent_escape() {
        let result = resolve_target_path(Path::new("/var/lib/anneal"), "../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_target_path_rejects_absolute_paths() {
        let result = resolve_target_path(Path::new("/var/lib/anneal"), "/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn normalize_health_host_rejects_remote_targets() {
        let result = normalize_health_host("8.8.8.8");
        assert!(result.is_err());
    }

    #[test]
    fn extract_health_targets_keeps_local_targets() {
        let rendered_config = serde_json::json!({
            "inbounds": [
                {
                    "listen": "127.0.0.1",
                    "port": 8443,
                    "protocol": "tcp"
                }
            ]
        })
        .to_string();

        let targets = extract_health_targets(&rendered_config).expect("targets");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].host, "127.0.0.1");
        assert_eq!(targets[0].port, 8443);
    }

    #[test]
    fn runtime_controller_accepts_supervisorctl() {
        assert!(parse_runtime_controller("supervisorctl").is_ok());
    }

    #[test]
    fn supervisor_running_status_is_detected() {
        assert!(supervisor_service_is_running(
            b"xray                             RUNNING   pid 17, uptime 0:00:15\n"
        ));
        assert!(!supervisor_service_is_running(
            b"xray                             STOPPED   Not started\n"
        ));
    }
}
