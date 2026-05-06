use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result, anyhow};

use crate::{
    bootstrap::ApiClient,
    cli::{InstallArgs, ResumeArgs},
    config::{InstallConfig, InstallLayout, ResellerConfig},
    release::ReleaseBundle,
    render::{render_caddyfile, render_mihomo_config, rewrite_panel_base_href, write_kv_file},
    state::{InstallState, InstallStep},
    system::System,
};

pub async fn run(layout: InstallLayout, args: InstallArgs) -> Result<()> {
    let bundle_root = required_bundle_root(args.bundle_root.clone())?;
    let bundle = ReleaseBundle::load(&bundle_root)?;
    let mut config = InstallConfig::from_args(args)?;
    config.release_version = Some(bundle.manifest.version.clone());
    let state = InstallState::load_or_new(&layout.state_path, config.role, config.deployment_mode)?;
    let mut installer = Installer::new(layout, bundle, config, state);
    installer.install().await
}

pub async fn resume(layout: InstallLayout, args: ResumeArgs) -> Result<()> {
    let bundle_root = required_bundle_root(args.bundle_root)?;
    let bundle = ReleaseBundle::load(&bundle_root)?;
    let mut config = InstallConfig::load(&layout.config_path)?;
    config.release_version = Some(bundle.manifest.version.clone());
    let state = InstallState::load_or_new(&layout.state_path, config.role, config.deployment_mode)?;
    let mut installer = Installer::new(layout, bundle, config, state);
    installer.install().await
}

pub async fn update_existing(layout: InstallLayout, bundle_root: PathBuf) -> Result<()> {
    let bundle = ReleaseBundle::load(&bundle_root)?;
    let mut config = InstallConfig::load(&layout.config_path)?;
    config.release_version = Some(bundle.manifest.version.clone());
    let state = InstallState::load_or_new(&layout.state_path, config.role, config.deployment_mode)?;
    let mut installer = Installer::new(layout, bundle, config, state);
    installer.write_files()?;
    installer.start_services()?;
    installer.persist()?;
    installer.write_summary()?;
    Ok(())
}

struct Installer {
    layout: InstallLayout,
    bundle: ReleaseBundle,
    config: InstallConfig,
    state: InstallState,
    system: System,
}

impl Installer {
    fn new(
        layout: InstallLayout,
        bundle: ReleaseBundle,
        config: InstallConfig,
        state: InstallState,
    ) -> Self {
        Self {
            layout,
            bundle,
            config,
            state,
            system: System::new(),
        }
    }

    async fn install(&mut self) -> Result<()> {
        self.system.require_root()?;
        self.bundle.validate()?;
        self.persist()?;
        self.begin_step(InstallStep::Prepare, "validated bundle")?;
        self.complete_step(InstallStep::Prepare, "validated bundle")?;

        if !self.state.is_completed(InstallStep::Packages) {
            self.begin_step(InstallStep::Packages, "installed packages")?;
            if let Err(error) = self.system.install_packages() {
                self.fail_step(InstallStep::Packages, &error.to_string())?;
                return Err(error);
            }
            self.complete_step(InstallStep::Packages, "installed packages")?;
        }

        if !self.state.is_completed(InstallStep::Files) {
            self.begin_step(InstallStep::Files, "synced files")?;
            if let Err(error) = self.write_files() {
                self.fail_step(InstallStep::Files, &error.to_string())?;
                return Err(error);
            }
            self.complete_step(InstallStep::Files, "synced files")?;
        }

        if !self.state.is_completed(InstallStep::Services) {
            self.begin_step(InstallStep::Services, "restarted services")?;
            if let Err(error) = self.start_services() {
                self.fail_step(InstallStep::Services, &error.to_string())?;
                return Err(error);
            }
            self.complete_step(InstallStep::Services, "restarted services")?;
        }

        self.bootstrap_control_plane().await?;
        self.ensure_starter_subscription().await?;
        self.write_summary()?;
        self.cleanup_transient_secrets().await?;
        self.state.finish();
        self.persist()?;
        Ok(())
    }

    fn write_files(&mut self) -> Result<()> {
        self.system.ensure_user(&self.config, &self.layout)?;
        self.system
            .ensure_postgres(&self.config.control_plane.database_url)?;
        self.system
            .install_executable(&self.bundle.api_path(), &self.layout.bin_dir().join("api"))?;
        self.system.install_executable(
            &self.bundle.worker_path(),
            &self.layout.bin_dir().join("worker"),
        )?;
        self.system.install_executable(
            &self.bundle.mihomo_path(),
            &self.layout.bin_dir().join("mihomo"),
        )?;
        self.system
            .install_executable(&self.bundle.annealctl_path(), &self.layout.utility_path)?;
        self.system.install_executable(
            &self.bundle.annealctl_path(),
            &self.layout.bin_dir().join("annealctl"),
        )?;
        self.system
            .sync_dir(&self.bundle.web_dir(), &self.layout.web_dir())?;
        rewrite_panel_base_href(
            &self.layout.web_dir().join("index.html"),
            &self.config.control_plane.panel_path,
        )?;
        self.system
            .sync_dir(&self.bundle.migrations_dir(), &self.layout.migrations_dir())?;
        let caddyfile = render_caddyfile(
            &self.bundle.deploy_asset("caddy/Caddyfile.tpl")?,
            &self.config.control_plane.domain,
            &self.config.control_plane.panel_path,
        )?;
        fs::write(&self.layout.caddyfile_path, caddyfile)
            .with_context(|| format!("failed to write {}", self.layout.caddyfile_path.display()))?;
        fs::write(self.layout.mihomo_config_path(), render_mihomo_config()).with_context(|| {
            format!(
                "failed to write {}",
                self.layout.mihomo_config_path().display()
            )
        })?;
        for unit in [
            "systemd/anneal-api.service",
            "systemd/anneal-worker.service",
            "systemd/anneal-caddy.service",
            "systemd/anneal-mihomo.service",
        ] {
            self.system
                .copy_systemd_unit(&self.bundle.deploy_asset(unit)?, &self.layout.systemd_dir)?;
        }
        write_kv_file(&self.layout.env_path, &self.env_values())?;
        self.system.chown_recursive(
            &self.config.install_user,
            &self.config.install_group,
            &self.layout.install_root,
        )?;
        self.system.chown_recursive(
            &self.config.install_user,
            &self.config.install_group,
            &self.layout.data_root,
        )?;
        self.system.daemon_reload()?;
        Ok(())
    }

    fn start_services(&mut self) -> Result<()> {
        self.system.disable_conflicting_caddy_services()?;
        self.system.enable_and_restart([
            "postgresql",
            "anneal-api.service",
            "anneal-worker.service",
            "anneal-caddy.service",
            "anneal-mihomo.service",
        ])
    }

    async fn bootstrap_control_plane(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::ControlPlaneBootstrap) {
            return Ok(());
        }
        let control_plane = self.config.control_plane.clone();
        self.begin_step(
            InstallStep::ControlPlaneBootstrap,
            "waiting for control-plane",
        )?;
        self.system.wait_for_http(
            "http://127.0.0.1:8080/api/v1/health",
            Duration::from_secs(240),
        )?;
        let api = ApiClient::local()?;
        if let Some(bootstrap_token) = control_plane.bootstrap_token.as_deref() {
            api.bootstrap_superadmin(
                bootstrap_token,
                &control_plane.superadmin.email,
                &control_plane.superadmin.display_name,
                &control_plane.superadmin.password,
            )
            .await?;
        }
        self.complete_step(
            InstallStep::ControlPlaneBootstrap,
            "control-plane bootstrap completed",
        )
    }

    async fn ensure_starter_subscription(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::StarterSubscription) {
            return Ok(());
        }
        self.begin_step(
            InstallStep::StarterSubscription,
            "ensuring starter subscription",
        )?;
        let api = ApiClient::local()?;
        let control_plane = self.config.control_plane.clone();
        let access_token = api
            .login_superadmin(
                &control_plane.superadmin.email,
                &control_plane.superadmin.password,
                &mut self.state,
            )
            .await?;
        let tenant_id = self
            .ensure_reseller_tenant(&api, &access_token, control_plane.reseller.as_ref())
            .await?;
        if let Some(starter) = control_plane.starter_subscription.as_ref() {
            let existing = api
                .list_subscriptions(&access_token)
                .await?
                .into_iter()
                .find(|subscription| {
                    subscription.tenant_id == tenant_id && subscription.name == starter.name
                });
            let delivery_url = if let Some(subscription) = existing {
                api.touch_subscription(&access_token, &subscription).await?;
                subscription.delivery_url.unwrap_or_default()
            } else {
                api.create_subscription(&access_token, tenant_id, starter)
                    .await?
            };
            self.state.bootstrap.starter_subscription_name = Some(starter.name.clone());
            self.state.bootstrap.starter_subscription_url = Some(delivery_url);
            self.persist()?;
        }
        self.complete_step(
            InstallStep::StarterSubscription,
            "starter subscription ensured",
        )
    }

    async fn ensure_reseller_tenant(
        &mut self,
        api: &ApiClient,
        access_token: &str,
        reseller: Option<&ResellerConfig>,
    ) -> Result<uuid::Uuid> {
        if let Some(tenant_id) = self.state.bootstrap.tenant_id {
            return Ok(tenant_id);
        }
        let reseller = reseller.ok_or_else(|| anyhow!("missing reseller config"))?;
        let tenant_id = api
            .create_reseller(
                access_token,
                &reseller.tenant_name,
                &reseller.email,
                &reseller.display_name,
                &reseller.password,
            )
            .await?;
        self.state.bootstrap.tenant_id = Some(tenant_id);
        self.persist()?;
        Ok(tenant_id)
    }

    async fn cleanup_transient_secrets(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::Cleanup) {
            return Ok(());
        }
        self.begin_step(InstallStep::Cleanup, "cleared bootstrap token")?;
        self.config.clear_control_plane_bootstrap_token();
        self.persist()?;
        write_kv_file(&self.layout.env_path, &self.env_values())?;
        self.system
            .restart(["anneal-api.service", "anneal-worker.service"])?;
        self.complete_step(InstallStep::Cleanup, "cleared bootstrap token")
    }

    fn write_summary(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::Summary) {
            return Ok(());
        }
        self.begin_step(InstallStep::Summary, "wrote install summary")?;
        write_kv_file(&self.layout.summary_path, &self.summary_values())?;
        self.complete_step(InstallStep::Summary, "wrote install summary")
    }

    fn env_values(&self) -> BTreeMap<String, String> {
        let control_plane = &self.config.control_plane;
        let mut values = BTreeMap::new();
        values.insert("ANNEAL_BIND_ADDRESS".into(), "0.0.0.0:8080".into());
        values.insert(
            "ANNEAL_MIGRATIONS_DIR".into(),
            self.layout.migrations_dir().display().to_string(),
        );
        values.insert(
            "ANNEAL_PUBLIC_BASE_URL".into(),
            control_plane.public_base_url.clone(),
        );
        values.insert("ANNEAL_CADDY_DOMAIN".into(), control_plane.domain.clone());
        values.insert("ANNEAL_PANEL_PATH".into(), control_plane.panel_path.clone());
        values.insert(
            "ANNEAL_DATABASE_URL".into(),
            control_plane.database_url.clone(),
        );
        values.insert(
            "ANNEAL_DATA_ENCRYPTION_KEY".into(),
            control_plane.data_encryption_key.clone(),
        );
        values.insert(
            "ANNEAL_TOKEN_HASH_KEY".into(),
            control_plane.token_hash_key.clone(),
        );
        values.insert(
            "ANNEAL_ACCESS_JWT_SECRET".into(),
            control_plane.access_jwt_secret.clone(),
        );
        values.insert(
            "ANNEAL_PRE_AUTH_JWT_SECRET".into(),
            control_plane.pre_auth_jwt_secret.clone(),
        );
        values.insert(
            "ANNEAL_MIHOMO_PUBLIC_HOST".into(),
            self.config.mihomo.public_host.clone(),
        );
        values.insert(
            "ANNEAL_MIHOMO_PUBLIC_PORT".into(),
            self.config.mihomo.public_port.to_string(),
        );
        values.insert(
            "ANNEAL_MIHOMO_PROTOCOLS".into(),
            self.config.mihomo.protocols.clone(),
        );
        if let Some(server_name) = self.config.mihomo.server_name.as_ref() {
            values.insert("ANNEAL_MIHOMO_SERVER_NAME".into(), server_name.clone());
        }
        if let Some(public_key) = self.config.mihomo.reality_public_key.as_ref() {
            values.insert(
                "ANNEAL_MIHOMO_REALITY_PUBLIC_KEY".into(),
                public_key.clone(),
            );
        }
        if let Some(short_id) = self.config.mihomo.reality_short_id.as_ref() {
            values.insert("ANNEAL_MIHOMO_REALITY_SHORT_ID".into(), short_id.clone());
        }
        values.insert(
            "ANNEAL_MIHOMO_CIPHER".into(),
            self.config.mihomo.cipher.clone(),
        );
        if let Some(token) = control_plane.bootstrap_token.as_ref() {
            values.insert("ANNEAL_BOOTSTRAP_TOKEN".into(), token.clone());
        }
        if let Some(otlp) = control_plane.otlp_endpoint.as_ref() {
            values.insert("ANNEAL_OTLP_ENDPOINT".into(), otlp.clone());
        }
        if let Some(version) = self.config.release_version.as_ref() {
            values.insert("ANNEAL_VERSION".into(), version.clone());
        }
        values
    }

    fn summary_values(&self) -> BTreeMap<String, String> {
        let control_plane = &self.config.control_plane;
        let mut values = BTreeMap::new();
        values.insert(
            "ANNEAL_PUBLIC_BASE_URL".into(),
            control_plane.public_base_url.clone(),
        );
        values.insert("ANNEAL_PANEL_PATH".into(), control_plane.panel_path.clone());
        values.insert(
            "ANNEAL_SUPERADMIN_EMAIL".into(),
            control_plane.superadmin.email.clone(),
        );
        values.insert(
            "ANNEAL_SUPERADMIN_PASSWORD".into(),
            control_plane.superadmin.password.clone(),
        );
        values.insert(
            "ANNEAL_MIHOMO_PUBLIC_HOST".into(),
            self.config.mihomo.public_host.clone(),
        );
        values.insert(
            "ANNEAL_MIHOMO_PUBLIC_PORT".into(),
            self.config.mihomo.public_port.to_string(),
        );
        if let Some(reseller) = control_plane.reseller.as_ref() {
            values.insert(
                "ANNEAL_RESELLER_TENANT_NAME".into(),
                reseller.tenant_name.clone(),
            );
            values.insert("ANNEAL_RESELLER_EMAIL".into(), reseller.email.clone());
            values.insert("ANNEAL_RESELLER_PASSWORD".into(), reseller.password.clone());
        }
        if let Some(name) = self.state.bootstrap.starter_subscription_name.as_ref() {
            values.insert("ANNEAL_STARTER_SUBSCRIPTION_NAME".into(), name.clone());
        }
        if let Some(url) = self.state.bootstrap.starter_subscription_url.as_ref() {
            values.insert("ANNEAL_STARTER_SUBSCRIPTION_URL".into(), url.clone());
        }
        values
    }

    fn begin_step(&mut self, step: InstallStep, detail: &str) -> Result<()> {
        self.state.mark_running(step, detail);
        self.persist()
    }

    fn complete_step(&mut self, step: InstallStep, detail: &str) -> Result<()> {
        self.state.mark_completed(step, detail);
        self.persist()
    }

    fn fail_step(&mut self, step: InstallStep, detail: &str) -> Result<()> {
        self.state.mark_failed(step, detail);
        self.persist()
    }

    fn persist(&mut self) -> Result<()> {
        self.config.save(&self.layout.config_path)?;
        self.state.release_version = self.config.release_version.clone();
        self.state.save(&self.layout.state_path)
    }
}

fn required_bundle_root(value: Option<PathBuf>) -> Result<PathBuf> {
    value.ok_or_else(|| anyhow!("--bundle-root is required"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Utc;

    use super::*;
    use crate::{
        config::{
            AdminConfig, ControlPlaneConfig, DeploymentMode, InstallLayout, InstallRole,
            StarterSubscriptionConfig,
        },
        release::{ManifestPaths, ReleaseManifest},
        state::BootstrapState,
    };

    fn test_layout() -> InstallLayout {
        InstallLayout {
            install_root: PathBuf::from("/opt/test-anneal"),
            config_dir: PathBuf::from("/etc/test-anneal"),
            data_root: PathBuf::from("/var/lib/test-anneal"),
            config_path: PathBuf::from("/etc/test-anneal/install.toml"),
            state_path: PathBuf::from("/var/lib/test-anneal/install-state.json"),
            env_path: PathBuf::from("/etc/test-anneal/anneal.env"),
            summary_path: PathBuf::from("/etc/test-anneal/admin-summary.env"),
            caddyfile_path: PathBuf::from("/etc/test-anneal/Caddyfile"),
            systemd_dir: PathBuf::from("/etc/systemd/system"),
            utility_path: PathBuf::from("/usr/local/bin/annealctl"),
        }
    }

    fn release_bundle() -> ReleaseBundle {
        ReleaseBundle {
            root: PathBuf::from("/tmp/anneal-bundle"),
            manifest: ReleaseManifest {
                version: "0.1.0".into(),
                paths: ManifestPaths {
                    api: "bin/api".into(),
                    worker: "bin/worker".into(),
                    annealctl: Some("bin/annealctl".into()),
                    mihomo: "runtime/mihomo".into(),
                    web: "web".into(),
                    migrations: "migrations".into(),
                    deploy: "deploy".into(),
                },
            },
        }
    }

    fn sample_config() -> InstallConfig {
        InstallConfig {
            role: InstallRole::ControlPlane,
            deployment_mode: DeploymentMode::Native,
            release_version: Some("0.1.0".into()),
            install_user: "anneal".into(),
            install_group: "anneal".into(),
            control_plane: ControlPlaneConfig {
                domain: "panel.example.com".into(),
                panel_path: "private-path".into(),
                public_base_url: "https://panel.example.com/private-path".into(),
                database_url: "postgres://anneal:secret@127.0.0.1:5432/anneal".into(),
                bootstrap_token: Some("bootstrap-token".into()),
                data_encryption_key: "a".repeat(64),
                token_hash_key: "b".repeat(64),
                access_jwt_secret: "c".repeat(64),
                pre_auth_jwt_secret: "d".repeat(64),
                otlp_endpoint: None,
                superadmin: AdminConfig {
                    email: "admin@panel.example.com".into(),
                    display_name: "Superadmin".into(),
                    password: "superadmin-password".into(),
                },
                reseller: None,
                starter_subscription: Some(StarterSubscriptionConfig {
                    name: "Starter access".into(),
                    traffic_limit_bytes: 1_099_511_627_776,
                    days: 3650,
                }),
            },
            mihomo: crate::config::MihomoConfig {
                public_host: "panel.example.com".into(),
                public_port: 443,
                protocols: "vless_reality,vmess".into(),
                server_name: Some("panel.example.com".into()),
                reality_public_key: None,
                reality_short_id: None,
                cipher: "2022-blake3-aes-128-gcm".into(),
            },
        }
    }

    fn sample_state() -> InstallState {
        InstallState {
            role: InstallRole::ControlPlane,
            deployment_mode: DeploymentMode::Native,
            release_version: Some("0.1.0".into()),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            steps: BTreeMap::new(),
            bootstrap: BootstrapState {
                superadmin_totp_secret: Some("totp-secret".into()),
                tenant_id: None,
                starter_subscription_name: Some("Starter access".into()),
                starter_subscription_url: Some("https://panel.example.com/s/token".into()),
            },
        }
    }

    #[test]
    fn env_contains_mihomo_settings() {
        let installer = Installer::new(
            test_layout(),
            release_bundle(),
            sample_config(),
            sample_state(),
        );
        let values = installer.env_values();

        assert_eq!(
            values.get("ANNEAL_MIHOMO_PUBLIC_HOST"),
            Some(&"panel.example.com".into())
        );
        assert_eq!(values.get("ANNEAL_MIHOMO_PUBLIC_PORT"), Some(&"443".into()));
        assert_eq!(
            values.get("ANNEAL_MIHOMO_PROTOCOLS"),
            Some(&"vless_reality,vmess".into())
        );
    }
}
