use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Url;
use serde_json::Value;

use crate::config::{DeploymentMode, InstallConfig, InstallLayout, InstallRole};

const XRAY_PLACEHOLDER: &str = "{\"log\":{\"loglevel\":\"warning\"},\"inbounds\":[],\"outbounds\":[{\"protocol\":\"freedom\",\"tag\":\"direct\"}]}";
const SINGBOX_PLACEHOLDER: &str = "{\"log\":{\"level\":\"warn\"},\"inbounds\":[],\"outbounds\":[{\"type\":\"direct\",\"tag\":\"direct\"}]}";

#[derive(Debug, Clone)]
pub struct DatabaseParts {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct System;

#[derive(Debug, Clone)]
struct OsRelease {
    id: String,
    version_id: String,
    codename: String,
}

impl System {
    pub fn new() -> Self {
        Self
    }

    pub fn require_root(&self) -> Result<()> {
        let output = self.capture("id", ["-u"])?;
        if output.trim() == "0" {
            Ok(())
        } else {
            bail!("annealctl install/update/restart/uninstall must run as root")
        }
    }

    pub fn install_packages(&self, role: InstallRole, mode: DeploymentMode) -> Result<()> {
        let os = self.load_os_release()?;
        self.require_supported_platform(&os)?;
        match mode {
            DeploymentMode::Native => {
                if role.includes_control_plane() {
                    self.setup_postgres_repository(&os)?;
                    self.setup_caddy_repository()?;
                    self.run("apt-get", ["update"])?;
                    self.run(
                        "apt-get",
                        [
                            "install",
                            "-y",
                            "ca-certificates",
                            "curl",
                            "tar",
                            "openssl",
                            "iproute2",
                            "debian-keyring",
                            "debian-archive-keyring",
                            "apt-transport-https",
                            "postgresql-17",
                            "postgresql-client-17",
                            "postgresql-contrib-17",
                            "caddy",
                        ],
                    )?;
                } else {
                    self.run("apt-get", ["update"])?;
                    self.run(
                        "apt-get",
                        [
                            "install",
                            "-y",
                            "curl",
                            "tar",
                            "ca-certificates",
                            "openssl",
                            "iproute2",
                        ],
                    )?;
                }
            }
            DeploymentMode::Docker => self.install_docker_packages(&os)?,
        }
        Ok(())
    }

    pub fn ensure_user(&self, config: &InstallConfig, layout: &InstallLayout) -> Result<()> {
        if !self.status_ok("getent", ["group", &config.install_group]) {
            self.run("groupadd", ["--system", &config.install_group])?;
        }
        if !self.status_ok("id", ["-u", &config.install_user]) {
            self.run(
                "useradd",
                [
                    "--system",
                    "--gid",
                    &config.install_group,
                    "--home",
                    layout.data_root.to_str().unwrap_or("/var/lib/anneal"),
                    "--create-home",
                    "--shell",
                    "/usr/sbin/nologin",
                    &config.install_user,
                ],
            )?;
        }
        for path in [
            layout.bin_dir(),
            layout.web_dir(),
            layout.migrations_dir(),
            layout.config_dir.clone(),
            layout.data_root.clone(),
        ] {
            fs::create_dir_all(&path)
                .with_context(|| format!("failed to create {}", path.display()))?;
        }
        Ok(())
    }

    pub fn ensure_postgres(&self, database_url: &str) -> Result<()> {
        let parts = Self::parse_database_url(database_url)?;
        if parts.host != "127.0.0.1" && parts.host != "localhost" {
            return Ok(());
        }
        self.run("systemctl", ["enable", "--now", "postgresql"])?;
        let role_exists = self.capture(
            "runuser",
            [
                "-u",
                "postgres",
                "--",
                "psql",
                "-p",
                &parts.port.to_string(),
                "-tAc",
                &format!("select 1 from pg_roles where rolname='{}'", parts.user),
            ],
        )?;
        if !role_exists.trim().contains('1') {
            self.run(
                "runuser",
                [
                    "-u",
                    "postgres",
                    "--",
                    "psql",
                    "-p",
                    &parts.port.to_string(),
                    "-c",
                    &format!(
                        "create role {} login password '{}';",
                        parts.user, parts.password
                    ),
                ],
            )?;
        }
        let db_exists = self.capture(
            "runuser",
            [
                "-u",
                "postgres",
                "--",
                "psql",
                "-p",
                &parts.port.to_string(),
                "-tAc",
                &format!("select 1 from pg_database where datname='{}'", parts.name),
            ],
        )?;
        if !db_exists.trim().contains('1') {
            self.run(
                "runuser",
                [
                    "-u",
                    "postgres",
                    "--",
                    "createdb",
                    "-p",
                    &parts.port.to_string(),
                    "-O",
                    &parts.user,
                    &parts.name,
                ],
            )?;
        }
        Ok(())
    }

    pub fn drop_local_database(&self, database_url: &str) -> Result<()> {
        let parts = Self::parse_database_url(database_url)?;
        if parts.host != "127.0.0.1" && parts.host != "localhost" {
            return Ok(());
        }
        let _ = self.run(
            "runuser",
            [
                "-u",
                "postgres",
                "--",
                "psql",
                "-p",
                &parts.port.to_string(),
                "-c",
                &format!(
                    "select pg_terminate_backend(pid) from pg_stat_activity where datname='{}' and pid <> pg_backend_pid();",
                    parts.name
                ),
            ],
        );
        let _ = self.run(
            "runuser",
            [
                "-u",
                "postgres",
                "--",
                "dropdb",
                "-p",
                &parts.port.to_string(),
                "--if-exists",
                &parts.name,
            ],
        );
        let _ = self.run(
            "runuser",
            [
                "-u",
                "postgres",
                "--",
                "psql",
                "-p",
                &parts.port.to_string(),
                "-c",
                &format!("drop role if exists {};", parts.user),
            ],
        );
        Ok(())
    }

    pub fn install_executable(&self, source: &Path, destination: &Path) -> Result<()> {
        self.copy_file(source, destination)?;
        set_executable(destination)?;
        Ok(())
    }

    pub fn copy_file(&self, source: &Path, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(source, destination).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        Ok(())
    }

    pub fn sync_dir(&self, source: &Path, destination: &Path) -> Result<()> {
        if !source.exists() {
            bail!("missing source directory: {}", source.display());
        }
        if destination.exists() {
            fs::remove_dir_all(destination)
                .with_context(|| format!("failed to remove {}", destination.display()))?;
        }
        fs::create_dir_all(destination)
            .with_context(|| format!("failed to create {}", destination.display()))?;
        for entry in
            fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
        {
            let entry = entry?;
            let entry_type = entry.file_type()?;
            let target = destination.join(entry.file_name());
            if entry_type.is_dir() {
                self.sync_dir(&entry.path(), &target)?;
            } else {
                self.copy_file(&entry.path(), &target)?;
            }
        }
        Ok(())
    }

    pub fn chown_recursive(&self, user: &str, group: &str, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        self.run(
            "chown",
            [
                "-R",
                &format!("{user}:{group}"),
                path.to_str().ok_or_else(|| anyhow!("invalid path"))?,
            ],
        )
    }

    pub fn copy_systemd_unit(&self, source: &Path, destination_dir: &Path) -> Result<()> {
        let destination = destination_dir.join(
            source
                .file_name()
                .ok_or_else(|| anyhow!("missing systemd unit name"))?,
        );
        self.copy_file(source, &destination)
    }

    pub fn disable_conflicting_caddy_services(&self) -> Result<()> {
        for service in ["caddy.service", "caddy-api.service"] {
            let _ = self.run("systemctl", ["disable", "--now", service]);
        }
        Ok(())
    }

    pub fn daemon_reload(&self) -> Result<()> {
        self.run("systemctl", ["daemon-reload"])
    }

    pub fn enable_and_restart<'a>(
        &self,
        services: impl IntoIterator<Item = &'a str>,
    ) -> Result<()> {
        for service in services {
            self.run("systemctl", ["enable", service])?;
            self.run("systemctl", ["restart", service])?;
        }
        Ok(())
    }

    pub fn restart<'a>(&self, services: impl IntoIterator<Item = &'a str>) -> Result<()> {
        for service in services {
            self.run("systemctl", ["restart", service])?;
        }
        Ok(())
    }

    pub fn disable_and_stop<'a>(&self, services: impl IntoIterator<Item = &'a str>) -> Result<()> {
        for service in services {
            let _ = self.run("systemctl", ["disable", "--now", service]);
        }
        Ok(())
    }

    pub fn service_status(&self, service: &str) -> Result<String> {
        Ok(self
            .capture("systemctl", ["is-active", service])?
            .trim()
            .to_owned())
    }

    pub fn wait_for_http(&self, url: &str, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.status_ok(
                "curl",
                [
                    "--silent",
                    "--show-error",
                    "--fail",
                    "--connect-timeout",
                    "2",
                    "--max-time",
                    "5",
                    url,
                ],
            ) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        bail!("timeout waiting for {url}")
    }

    pub fn generate_self_signed_cert(
        &self,
        cert: &Path,
        key: &Path,
        common_name: &str,
    ) -> Result<()> {
        if cert.exists() && key.exists() {
            return Ok(());
        }
        if let Some(parent) = cert.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        self.run(
            "openssl",
            [
                "req",
                "-x509",
                "-nodes",
                "-newkey",
                "rsa:2048",
                "-keyout",
                key.to_str().ok_or_else(|| anyhow!("invalid key path"))?,
                "-out",
                cert.to_str().ok_or_else(|| anyhow!("invalid cert path"))?,
                "-subj",
                &format!("/CN={common_name}"),
                "-days",
                "3650",
            ],
        )
    }

    pub fn cleanup_runtime_placeholder_configs(&self, root: &Path) -> Result<()> {
        self.remove_placeholder(root.join("xray").join("config.json"), XRAY_PLACEHOLDER)?;
        self.remove_placeholder(
            root.join("singbox").join("config.json"),
            SINGBOX_PLACEHOLDER,
        )?;
        Ok(())
    }

    pub fn docker_compose_up(
        &self,
        stack_root: &Path,
        env_file: &Path,
        rebuild: bool,
    ) -> Result<()> {
        let compose_file = stack_root.join("compose.yml");
        let mut args = vec![
            "-f".to_owned(),
            compose_file.display().to_string(),
            "--env-file".to_owned(),
            env_file.display().to_string(),
        ];
        if rebuild {
            args.extend(["build".into()]);
            self.compose(&args)?;
            args.truncate(4);
        }
        args.extend(["up".into(), "-d".into()]);
        self.compose(&args)
    }

    pub fn docker_compose_restart(&self, stack_root: &Path, env_file: &Path) -> Result<()> {
        let compose_file = stack_root.join("compose.yml");
        self.compose(&[
            "-f".into(),
            compose_file.display().to_string(),
            "--env-file".into(),
            env_file.display().to_string(),
            "restart".into(),
        ])
    }

    pub fn docker_compose_down(&self, stack_root: &Path, env_file: &Path) -> Result<()> {
        let compose_file = stack_root.join("compose.yml");
        self.compose(&[
            "-f".into(),
            compose_file.display().to_string(),
            "--env-file".into(),
            env_file.display().to_string(),
            "down".into(),
            "-v".into(),
        ])
    }

    pub fn wait_for_agent_state(
        &self,
        state_path: &Path,
        timeout: Duration,
        agent_service: Option<&str>,
    ) -> Result<Value> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(raw) = fs::read_to_string(state_path) {
                let json: Value = serde_json::from_str(&raw)
                    .with_context(|| format!("failed to parse {}", state_path.display()))?;
                if agent_state_ready(&json) {
                    return Ok(json);
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        if let Some(service) = agent_service {
            let logs = self
                .capture(
                    "journalctl",
                    ["-u", service, "-n", "40", "--no-pager", "-l"],
                )
                .unwrap_or_default();
            let logs = logs.trim();
            if !logs.is_empty() {
                bail!(
                    "timeout waiting for node agent state at {}\nrecent {} logs:\n{}",
                    state_path.display(),
                    service,
                    logs
                );
            }
        }
        bail!(
            "timeout waiting for node agent state at {}",
            state_path.display()
        )
    }

    pub fn wait_for_runtime_rollout(
        &self,
        data_root: &Path,
        engines: &[&str],
        timeout: Duration,
        agent_service: &str,
        runtime_mode: DeploymentMode,
    ) -> Result<()> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let agent_active = match runtime_mode {
                DeploymentMode::Native => self
                    .service_status(agent_service)
                    .map(|status| status == "active")
                    .unwrap_or(false),
                DeploymentMode::Docker => true,
            };
            if !agent_active {
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
            let ready = engines.iter().all(|engine| match *engine {
                "xray" => data_root.join("xray").join("config.json").exists(),
                "singbox" => data_root.join("singbox").join("config.json").exists(),
                _ => false,
            });
            if ready {
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        bail!("timeout waiting for runtime rollout")
    }

    pub fn remove_path(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        if path.is_dir() {
            fs::remove_dir_all(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        Ok(())
    }

    pub fn parse_database_url(value: &str) -> Result<DatabaseParts> {
        let url = Url::parse(value).context("failed to parse database URL")?;
        let host = url
            .host_str()
            .ok_or_else(|| anyhow!("database host is required"))?
            .to_owned();
        let port = url.port().unwrap_or(5432);
        let name = url.path().trim_matches('/').to_owned();
        let user = url.username().to_owned();
        let password = url
            .password()
            .ok_or_else(|| anyhow!("database password is required"))?
            .to_owned();
        Ok(DatabaseParts {
            host,
            port,
            name,
            user,
            password,
        })
    }

    fn install_docker_packages(&self, os: &OsRelease) -> Result<()> {
        if self.docker_repository_supported_platform(os) {
            self.remove_conflicting_docker_packages()?;
            self.setup_docker_repository(os)?;
            self.run("apt-get", ["update"])?;
            self.run(
                "apt-get",
                [
                    "install",
                    "-y",
                    "curl",
                    "tar",
                    "ca-certificates",
                    "openssl",
                    "iproute2",
                    "docker-ce",
                    "docker-ce-cli",
                    "containerd.io",
                    "docker-buildx-plugin",
                    "docker-compose-plugin",
                ],
            )?;
        } else {
            self.run("apt-get", ["update"])?;
            self.run(
                "apt-get",
                [
                    "install",
                    "-y",
                    "curl",
                    "tar",
                    "ca-certificates",
                    "openssl",
                    "iproute2",
                    "docker.io",
                    "docker-compose-plugin",
                ],
            )?;
        }
        self.run("systemctl", ["enable", "--now", "docker"])?;
        Ok(())
    }

    fn remove_conflicting_docker_packages(&self) -> Result<()> {
        let mut packages = Vec::new();
        for package in [
            "docker.io",
            "docker-doc",
            "docker-compose",
            "docker-compose-v2",
            "podman-docker",
            "containerd",
            "runc",
        ] {
            if self.status_ok("dpkg", ["-s", package]) {
                packages.push(package);
            }
        }
        if !packages.is_empty() {
            let mut args = vec!["remove", "-y"];
            args.extend(packages);
            self.run("apt-get", args)?;
        }
        Ok(())
    }

    fn load_os_release(&self) -> Result<OsRelease> {
        let raw =
            fs::read_to_string("/etc/os-release").context("failed to read /etc/os-release")?;
        let mut id = String::new();
        let mut version_id = String::new();
        let mut codename = String::new();
        for line in raw.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim_matches('"');
                match key {
                    "ID" => id = value.to_owned(),
                    "VERSION_ID" => version_id = value.to_owned(),
                    "VERSION_CODENAME" | "UBUNTU_CODENAME" if codename.is_empty() => {
                        codename = value.to_owned();
                    }
                    _ => {}
                }
            }
        }
        if id.is_empty() {
            bail!("failed to determine OS ID");
        }
        Ok(OsRelease {
            id,
            version_id,
            codename,
        })
    }

    fn require_supported_platform(&self, os: &OsRelease) -> Result<()> {
        let supported = match os.id.as_str() {
            "debian" => matches!(os.version_id.as_str(), "10" | "11" | "12" | "13"),
            "ubuntu" => matches!(
                os.codename.as_str(),
                "jammy" | "noble" | "plucky" | "questing"
            ),
            _ => false,
        };
        if supported {
            Ok(())
        } else {
            bail!(
                "supported distributions are Debian 10/11/12/13 and Ubuntu 22.04/24.04/25.04/25.10"
            )
        }
    }

    fn setup_caddy_repository(&self) -> Result<()> {
        let keyring_path = "/usr/share/keyrings/caddy-stable-archive-keyring.asc";
        fs::create_dir_all("/usr/share/keyrings").context("failed to create caddy keyring dir")?;
        self.run(
            "curl",
            [
                "--fail",
                "--retry",
                "5",
                "--retry-all-errors",
                "--location",
                "--silent",
                "--show-error",
                "https://dl.cloudsmith.io/public/caddy/stable/gpg.key",
                "-o",
                keyring_path,
            ],
        )?;
        fs::write(
            "/etc/apt/sources.list.d/caddy-stable.list",
            format!(
                "deb [signed-by={keyring_path}] https://dl.cloudsmith.io/public/caddy/stable/deb/debian any-version main\n\
                 deb-src [signed-by={keyring_path}] https://dl.cloudsmith.io/public/caddy/stable/deb/debian any-version main\n"
            ),
        )
        .context("failed to write caddy repository")?;
        Ok(())
    }

    fn setup_postgres_repository(&self, os: &OsRelease) -> Result<()> {
        let keyring_dir = PathBuf::from("/usr/share/postgresql-common/pgdg");
        fs::create_dir_all(&keyring_dir)
            .with_context(|| format!("failed to create {}", keyring_dir.display()))?;
        let keyring_path = keyring_dir.join("apt.postgresql.org.asc");
        self.run(
            "curl",
            [
                "--fail",
                "--retry",
                "5",
                "--retry-all-errors",
                "--location",
                "--silent",
                "--show-error",
                "https://www.postgresql.org/media/keys/ACCC4CF8.asc",
                "-o",
                keyring_path
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid pgdg key path"))?,
            ],
        )?;
        let base_url = if os.id == "debian" && os.version_id == "10" {
            "https://apt-archive.postgresql.org/pub/repos/apt"
        } else {
            "https://apt.postgresql.org/pub/repos/apt"
        };
        fs::write(
            "/etc/apt/sources.list.d/pgdg.list",
            format!(
                "deb [signed-by={}] {base_url} {}-pgdg main\n",
                keyring_path.display(),
                os.codename
            ),
        )
        .context("failed to write pgdg repository")?;
        Ok(())
    }

    fn setup_docker_repository(&self, os: &OsRelease) -> Result<()> {
        let keyring_dir = PathBuf::from("/etc/apt/keyrings");
        fs::create_dir_all(&keyring_dir)
            .with_context(|| format!("failed to create {}", keyring_dir.display()))?;
        let keyring_path = keyring_dir.join("docker.asc");
        let base_url = if os.id == "ubuntu" {
            "https://download.docker.com/linux/ubuntu"
        } else {
            "https://download.docker.com/linux/debian"
        };
        self.run(
            "curl",
            [
                "--fail",
                "--retry",
                "5",
                "--retry-all-errors",
                "--location",
                "--silent",
                "--show-error",
                &format!("{base_url}/gpg"),
                "-o",
                keyring_path
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid docker key path"))?,
            ],
        )?;
        fs::write(
            "/etc/apt/sources.list.d/docker.sources",
            format!(
                "Types: deb\nURIs: {base_url}\nSuites: {}\nComponents: stable\nSigned-By: {}\n",
                os.codename,
                keyring_path.display()
            ),
        )
        .context("failed to write docker repository")?;
        Ok(())
    }

    fn docker_repository_supported_platform(&self, os: &OsRelease) -> bool {
        if os.id == "debian" {
            return matches!(os.version_id.as_str(), "11" | "12" | "13");
        }
        matches!(os.codename.as_str(), "jammy" | "noble" | "questing")
    }

    fn compose(&self, args: &[String]) -> Result<()> {
        if self.status_ok("docker", ["compose", "version"]) {
            let mut command = Command::new("docker");
            command.args(["compose"]);
            command.args(args);
            return self.check_output(command.output().context("failed to run docker compose")?);
        }
        let mut command = Command::new("docker-compose");
        command.args(args);
        self.check_output(command.output().context("failed to run docker-compose")?)
    }

    fn remove_placeholder(&self, path: PathBuf, placeholder: &str) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let current = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if current
            .chars()
            .filter(|char| !char.is_ascii_whitespace())
            .collect::<String>()
            == placeholder
        {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
        Ok(())
    }

    fn run<I, S>(&self, program: &str, args: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(program);
        command.args(args);
        self.check_output(
            command
                .output()
                .with_context(|| format!("failed to execute {program}"))?,
        )
    }

    fn capture<I, S>(&self, program: &str, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(program);
        command.args(args);
        let output = command
            .output()
            .with_context(|| format!("failed to execute {program}"))?;
        self.check_output(Output {
            status: output.status,
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
        })?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn status_ok<I, S>(&self, program: &str, args: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(program);
        command.args(args);
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.status().is_ok_and(|status| status.success())
    }

    fn check_output(&self, output: Output) -> Result<()> {
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !stderr.is_empty() {
            bail!(stderr);
        }
        if !stdout.is_empty() {
            bail!(stdout);
        }
        bail!("command exited with {}", output.status)
    }
}

fn agent_state_ready(json: &Value) -> bool {
    json.get("runtimes")
        .and_then(Value::as_object)
        .is_some_and(|runtimes| {
            !runtimes.is_empty()
                && runtimes.values().all(|runtime| {
                    runtime
                        .get("node_id")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty())
                        && runtime
                            .get("node_token")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.is_empty())
                })
        })
}

fn set_executable(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(_path)
            .with_context(|| format!("failed to stat {}", _path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(_path, permissions)
            .with_context(|| format!("failed to chmod {}", _path.display()))?;
    }
    Ok(())
}
