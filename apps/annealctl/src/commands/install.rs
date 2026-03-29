use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use uuid::Uuid;

use crate::{
    bootstrap::ApiClient,
    cli::{InstallArgs, ResumeArgs},
    config::{
        DeploymentMode, InstallConfig, InstallLayout, InstallRole, NodeConfig, engines_csv,
        protocols_csv,
    },
    i18n::{Language, Translator},
    release::ReleaseBundle,
    render::{render_caddyfile, rewrite_panel_base_href, write_kv_file},
    state::{InstallState, InstallStep},
    system::System,
    ui::{
        reporter::{InstallReporter, NullInstallReporter, make_reporter},
        wizard::prepare_install_args,
    },
};

pub async fn run(layout: InstallLayout, args: InstallArgs) -> Result<()> {
    let bundle_root = required_bundle_root(args.bundle_root.clone())?;
    let (args, translator) = prepare_install_args(args)?;
    let bundle = ReleaseBundle::load(&bundle_root)?;
    let mut config = InstallConfig::from_args(args)?;
    config.release_version = Some(bundle.manifest.version.clone());
    let state = InstallState::load_or_new(&layout.state_path, config.role, config.deployment_mode)?;
    let reporter = make_reporter(translator, config.role, &state);
    let mut installer = Installer::new(layout, bundle, config, state).with_reporter(reporter);
    installer.install().await
}

pub async fn resume(layout: InstallLayout, args: ResumeArgs) -> Result<()> {
    let bundle_root = required_bundle_root(args.bundle_root)?;
    let bundle = ReleaseBundle::load(&bundle_root)?;
    let mut config = InstallConfig::load(&layout.config_path)?;
    config.release_version = Some(bundle.manifest.version.clone());
    let state = InstallState::load_or_new(&layout.state_path, config.role, config.deployment_mode)?;
    let reporter = make_reporter(default_translator(), config.role, &state);
    let mut installer = Installer::new(layout, bundle, config, state).with_reporter(reporter);
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
    reporter: Box<dyn InstallReporter>,
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
            reporter: Box::new(NullInstallReporter),
        }
    }

    fn with_reporter(mut self, reporter: Box<dyn InstallReporter>) -> Self {
        self.reporter = reporter;
        self
    }

    async fn install(&mut self) -> Result<()> {
        self.system.require_root()?;
        self.bundle.validate_for(self.config.role)?;
        self.persist()?;
        self.reporter.start(&self.config, &self.state);
        self.begin_step(InstallStep::Prepare, "validated bundle")?;
        self.complete_step(InstallStep::Prepare, "validated bundle")?;
        if !self.state.is_completed(InstallStep::Packages) {
            let role = self.config.role;
            let deployment_mode = self.config.deployment_mode;
            self.begin_step(InstallStep::Packages, "installed packages")?;
            if let Err(error) = self.system.install_packages(role, deployment_mode) {
                self.fail_step(InstallStep::Packages, &error.to_string())?;
                return Err(error);
            }
            self.complete_step(InstallStep::Packages, "installed packages")?;
        }
        self.apply_files_and_services().await?;
        for step in bootstrap_execution_order(self.config.role) {
            match step {
                InstallStep::ControlPlaneBootstrap => self.bootstrap_control_plane().await?,
                InstallStep::StarterSubscription => self.ensure_starter_subscription().await?,
                InstallStep::NodeBootstrap => self.bootstrap_node().await?,
                _ => {}
            }
        }
        self.write_summary()?;
        self.cleanup_transient_secrets().await?;
        self.state.finish();
        self.persist()?;
        self.reporter.finish(&self.config, &self.state);
        Ok(())
    }

    async fn apply_files_and_services(&mut self) -> Result<()> {
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
        Ok(())
    }

    fn write_files(&mut self) -> Result<()> {
        match self.config.deployment_mode {
            DeploymentMode::Native => self.write_native_files(),
            DeploymentMode::Docker => self.write_docker_files(),
        }
    }

    fn start_services(&mut self) -> Result<()> {
        match self.config.deployment_mode {
            DeploymentMode::Native => self.start_native_services(),
            DeploymentMode::Docker => self.start_docker_services(),
        }
    }

    fn write_native_files(&mut self) -> Result<()> {
        self.system.ensure_user(&self.config, &self.layout)?;
        if let Some(control_plane) = self.config.control_plane.as_ref() {
            self.system.ensure_postgres(&control_plane.database_url)?;
            self.system
                .install_executable(&self.bundle.api_path(), &self.layout.bin_dir().join("api"))?;
            self.system.install_executable(
                &self.bundle.worker_path(),
                &self.layout.bin_dir().join("worker"),
            )?;
            self.system
                .sync_dir(&self.bundle.web_dir(), &self.layout.web_dir())?;
            rewrite_panel_base_href(
                &self.layout.web_dir().join("index.html"),
                &control_plane.panel_path,
            )?;
            self.system
                .sync_dir(&self.bundle.migrations_dir(), &self.layout.migrations_dir())?;
            let caddyfile = render_caddyfile(
                &self.bundle.deploy_asset("caddy/Caddyfile.tpl")?,
                &control_plane.domain,
                &control_plane.panel_path,
            )?;
            fs::write(&self.layout.caddyfile_path, caddyfile).with_context(|| {
                format!("failed to write {}", self.layout.caddyfile_path.display())
            })?;
            for unit in [
                "systemd/anneal-api.service",
                "systemd/anneal-worker.service",
                "systemd/anneal-caddy.service",
            ] {
                self.system.copy_systemd_unit(
                    &self.bundle.deploy_asset(unit)?,
                    &self.layout.systemd_dir,
                )?;
            }
        }
        if self.config.role.includes_node() {
            let node = self
                .config
                .node
                .as_ref()
                .ok_or_else(|| anyhow!("missing node config"))?;
            self.system.install_executable(
                &self.bundle.node_agent_path(),
                &self.layout.bin_dir().join("node-agent"),
            )?;
            self.system.install_executable(
                &self.bundle.xray_path(),
                &self.layout.bin_dir().join("xray"),
            )?;
            self.system.install_executable(
                &self.bundle.singbox_path(),
                &self.layout.bin_dir().join("hiddify-core"),
            )?;
            for unit in [
                "systemd/anneal-node-agent.service",
                "systemd/anneal-xray.service",
                "systemd/anneal-singbox.service",
            ] {
                self.system.copy_systemd_unit(
                    &self.bundle.deploy_asset(unit)?,
                    &self.layout.systemd_dir,
                )?;
            }
            self.install_runtime_defaults(node)?;
        }
        self.system
            .install_executable(&self.bundle.annealctl_path(), &self.layout.utility_path)?;
        self.system.install_executable(
            &self.bundle.annealctl_path(),
            &self.layout.bin_dir().join("annealctl"),
        )?;
        write_kv_file(&self.layout.env_path, &self.env_values_native())?;
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

    fn write_docker_files(&mut self) -> Result<()> {
        let control_stack = self.layout.docker_stack_root(InstallRole::ControlPlane);
        let node_stack = self.layout.docker_stack_root(InstallRole::Node);
        if self.config.role.includes_control_plane() {
            self.sync_docker_stack(&control_stack)?;
            self.write_control_plane_docker_stack(&control_stack)?;
        }
        if self.config.role.includes_node() {
            self.sync_docker_stack(&node_stack)?;
            self.write_node_docker_stack(&node_stack)?;
        }
        self.system
            .install_executable(&self.bundle.annealctl_path(), &self.layout.utility_path)?;
        Ok(())
    }

    fn start_native_services(&mut self) -> Result<()> {
        if self.config.role.includes_control_plane() {
            self.system.disable_conflicting_caddy_services()?;
            self.system.enable_and_restart([
                "postgresql",
                "anneal-api.service",
                "anneal-worker.service",
                "anneal-caddy.service",
            ])?;
        }
        if self.config.role.includes_node() && self.config.role != InstallRole::AllInOne {
            self.system
                .disable_and_stop(["anneal-xray.service", "anneal-singbox.service"])?;
        }
        Ok(())
    }

    fn start_docker_services(&mut self) -> Result<()> {
        if self.config.role.includes_control_plane() {
            let stack_root = self.layout.docker_stack_root(InstallRole::ControlPlane);
            self.system
                .docker_compose_up(&stack_root, &stack_root.join(".env"), true)?;
        }
        if self.config.role == InstallRole::Node {
            let stack_root = self.layout.docker_stack_root(InstallRole::Node);
            self.system
                .docker_compose_up(&stack_root, &stack_root.join(".env"), true)?;
        }
        Ok(())
    }

    async fn bootstrap_control_plane(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::ControlPlaneBootstrap) {
            return Ok(());
        }
        let control_plane = self
            .config
            .control_plane
            .as_ref()
            .ok_or_else(|| anyhow!("missing control-plane config"))?
            .clone();
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
        )?;
        Ok(())
    }

    async fn bootstrap_node(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::NodeBootstrap) {
            return Ok(());
        }
        let node = self
            .config
            .node
            .clone()
            .ok_or_else(|| anyhow!("missing node config"))?;
        self.begin_step(InstallStep::NodeBootstrap, "waiting for node bootstrap")?;
        match self.config.role {
            InstallRole::AllInOne => self.bootstrap_all_in_one_node(node).await?,
            InstallRole::Node => self.bootstrap_standalone_node(node).await?,
            InstallRole::ControlPlane => {}
        }
        self.complete_step(InstallStep::NodeBootstrap, "node bootstrap completed")?;
        Ok(())
    }

    async fn ensure_starter_subscription(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::StarterSubscription) {
            return Ok(());
        }
        self.begin_step(
            InstallStep::StarterSubscription,
            "ensuring starter subscription",
        )?;
        self.sync_starter_subscription().await?;
        self.complete_step(
            InstallStep::StarterSubscription,
            "starter subscription ensured",
        )?;
        Ok(())
    }

    async fn cleanup_transient_secrets(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::Cleanup) {
            return Ok(());
        }
        self.begin_step(InstallStep::Cleanup, "cleared bootstrap tokens")?;
        self.config.clear_control_plane_bootstrap_token();
        self.config.clear_node_bootstrap_token();
        self.persist()?;
        match self.config.deployment_mode {
            DeploymentMode::Native => {
                write_kv_file(&self.layout.env_path, &self.env_values_native())?;
                let mut services = Vec::new();
                if self.config.role.includes_control_plane() {
                    services.extend(["anneal-api.service", "anneal-worker.service"]);
                }
                if self.config.role.includes_node() {
                    services.push("anneal-node-agent.service");
                }
                if !services.is_empty() {
                    self.system.restart(services)?;
                }
            }
            DeploymentMode::Docker => {
                self.rewrite_docker_env()?;
            }
        }
        self.complete_step(InstallStep::Cleanup, "cleared bootstrap tokens")
    }

    fn write_summary(&mut self) -> Result<()> {
        if self.state.is_completed(InstallStep::Summary) {
            return Ok(());
        }
        self.begin_step(InstallStep::Summary, "wrote install summary")?;
        write_kv_file(&self.layout.summary_path, &self.summary_values())?;
        self.complete_step(InstallStep::Summary, "wrote install summary")
    }

    async fn bootstrap_all_in_one_node(&mut self, node: NodeConfig) -> Result<()> {
        let control_plane = self
            .config
            .control_plane
            .as_ref()
            .ok_or_else(|| anyhow!("missing control-plane config"))?;
        let api = ApiClient::local()?;
        let access_token = api
            .login_superadmin(
                &control_plane.superadmin.email,
                &control_plane.superadmin.password,
                &mut self.state,
            )
            .await?;
        let tenant_id = self.ensure_all_in_one_tenant(&api, &access_token).await?;
        if self.state.bootstrap.node_group_id.is_none() {
            self.state.bootstrap.node_group_id = Some(
                api.create_node(
                    &access_token,
                    tenant_id,
                    node.group_name.as_deref().unwrap_or(&node.name),
                )
                .await?,
            );
            self.persist()?;
        }
        let node_group_id = self
            .state
            .bootstrap
            .node_group_id
            .ok_or_else(|| anyhow!("missing node group id"))?;
        let session = api
            .create_bootstrap_session(&access_token, tenant_id, node_group_id, &node)
            .await?;
        if let Some(config_node) = self.config.node.as_mut() {
            config_node.bootstrap_token = Some(session.bootstrap_token);
        }
        self.persist()?;
        match self.config.deployment_mode {
            DeploymentMode::Native => {
                write_kv_file(&self.layout.env_path, &self.env_values_native())?;
                self.system
                    .enable_and_restart(["anneal-node-agent.service"])?;
                self.system.wait_for_agent_state(
                    &self.layout.data_root.join("agent-state.json"),
                    Duration::from_secs(120),
                    Some("anneal-node-agent.service"),
                )?;
                self.sync_starter_subscription().await?;
                let engines = node_engine_names(&node);
                self.system.wait_for_runtime_rollout(
                    &self.layout.data_root,
                    &engines,
                    Duration::from_secs(240),
                    "anneal-node-agent.service",
                    DeploymentMode::Native,
                )?;
            }
            DeploymentMode::Docker => {
                let stack_root = self.layout.docker_stack_root(InstallRole::Node);
                self.write_node_docker_stack(&stack_root)?;
                self.system
                    .docker_compose_up(&stack_root, &stack_root.join(".env"), true)?;
                self.system.wait_for_agent_state(
                    &stack_root.join("data").join("agent-state.json"),
                    Duration::from_secs(120),
                    None,
                )?;
                self.sync_starter_subscription().await?;
                let engines = node_engine_names(&node);
                self.system.wait_for_runtime_rollout(
                    &stack_root.join("data"),
                    &engines,
                    Duration::from_secs(240),
                    "node",
                    DeploymentMode::Docker,
                )?;
            }
        }
        Ok(())
    }

    async fn ensure_all_in_one_tenant(
        &mut self,
        api: &ApiClient,
        access_token: &str,
    ) -> Result<Uuid> {
        if let Some(tenant_id) = self.state.bootstrap.tenant_id {
            return Ok(tenant_id);
        }
        let control_plane = self
            .config
            .control_plane
            .as_ref()
            .ok_or_else(|| anyhow!("missing control-plane config"))?;
        let reseller = control_plane
            .reseller
            .as_ref()
            .ok_or_else(|| anyhow!("missing reseller config"))?;
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

    async fn sync_starter_subscription(&mut self) -> Result<()> {
        let control_plane = self
            .config
            .control_plane
            .as_ref()
            .ok_or_else(|| anyhow!("missing control-plane config"))?;
        let starter = control_plane
            .starter_subscription
            .clone()
            .ok_or_else(|| anyhow!("missing starter subscription config"))?;
        let api = ApiClient::local()?;
        let access_token = api
            .login_superadmin(
                &control_plane.superadmin.email,
                &control_plane.superadmin.password,
                &mut self.state,
            )
            .await?;
        let tenant_id = self.ensure_all_in_one_tenant(&api, &access_token).await?;
        let subscriptions = api.list_subscriptions(&access_token).await?;
        let tenant_subscriptions = subscriptions
            .into_iter()
            .filter(|subscription| subscription.tenant_id == tenant_id)
            .collect::<Vec<_>>();
        if let Some(subscription) = tenant_subscriptions.first() {
            api.touch_subscription(&access_token, subscription).await?;
            self.state.bootstrap.starter_subscription_name = Some(subscription.name.clone());
            self.state.bootstrap.starter_subscription_url = subscription.delivery_url.clone();
        } else {
            let delivery_url = api
                .create_subscription(&access_token, tenant_id, &starter)
                .await?;
            self.state.bootstrap.starter_subscription_name = Some(starter.name.clone());
            self.state.bootstrap.starter_subscription_url = Some(delivery_url);
        }
        Ok(())
    }

    async fn bootstrap_standalone_node(&mut self, node: NodeConfig) -> Result<()> {
        match self.config.deployment_mode {
            DeploymentMode::Native => {
                write_kv_file(&self.layout.env_path, &self.env_values_native())?;
                self.system
                    .enable_and_restart(["anneal-node-agent.service"])?;
                self.system.wait_for_agent_state(
                    &self.layout.data_root.join("agent-state.json"),
                    Duration::from_secs(120),
                    Some("anneal-node-agent.service"),
                )?;
            }
            DeploymentMode::Docker => {
                let stack_root = self.layout.docker_stack_root(InstallRole::Node);
                self.system
                    .docker_compose_up(&stack_root, &stack_root.join(".env"), true)?;
                self.system.wait_for_agent_state(
                    &stack_root.join("data").join("agent-state.json"),
                    Duration::from_secs(120),
                    None,
                )?;
            }
        }
        let engines = node_engine_names(&node);
        let data_root = match self.config.deployment_mode {
            DeploymentMode::Native => self.layout.data_root.clone(),
            DeploymentMode::Docker => self
                .layout
                .docker_stack_root(InstallRole::Node)
                .join("data"),
        };
        self.system.wait_for_runtime_rollout(
            &data_root,
            &engines,
            Duration::from_secs(240),
            "anneal-node-agent.service",
            self.config.deployment_mode,
        )?;
        Ok(())
    }

    fn install_runtime_defaults(&self, node: &NodeConfig) -> Result<()> {
        fs::create_dir_all(self.layout.data_root.join("xray"))?;
        fs::create_dir_all(self.layout.data_root.join("singbox"))?;
        fs::create_dir_all(self.layout.data_root.join("tls"))?;
        self.system
            .cleanup_runtime_placeholder_configs(&self.layout.data_root)?;
        let common_name = self
            .config
            .control_plane
            .as_ref()
            .map(|control_plane| control_plane.domain.as_str())
            .unwrap_or(node.name.as_str());
        self.system.generate_self_signed_cert(
            &self.layout.data_root.join("tls").join("server.crt"),
            &self.layout.data_root.join("tls").join("server.key"),
            common_name,
        )?;
        Ok(())
    }

    fn sync_docker_stack(&self, stack_root: &Path) -> Result<()> {
        self.system
            .sync_dir(&self.bundle.deploy_asset("docker/prebuilt")?, stack_root)?;
        let bundle_root = stack_root.join("bundle");
        self.system
            .sync_dir(&self.bundle.root.join("bin"), &bundle_root.join("bin"))?;
        self.system.sync_dir(
            &self.bundle.migrations_dir(),
            &bundle_root.join("migrations"),
        )?;
        self.system.sync_dir(
            &self.bundle.root.join("runtime"),
            &bundle_root.join("runtime"),
        )?;
        self.system
            .sync_dir(&self.bundle.web_dir(), &bundle_root.join("web"))?;
        Ok(())
    }

    fn write_control_plane_docker_stack(&self, stack_root: &Path) -> Result<()> {
        self.system.copy_file(
            &stack_root.join("control-plane.compose.yml"),
            &stack_root.join("compose.yml"),
        )?;
        let control_plane = self
            .config
            .control_plane
            .as_ref()
            .ok_or_else(|| anyhow!("missing control-plane config"))?;
        rewrite_panel_base_href(
            &stack_root.join("bundle").join("web").join("index.html"),
            &control_plane.panel_path,
        )?;
        let caddyfile = render_caddyfile(
            &stack_root.join("control-plane.Caddyfile.tpl"),
            &control_plane.domain,
            &control_plane.panel_path,
        )?;
        fs::write(stack_root.join("Caddyfile"), caddyfile)?;
        write_kv_file(
            &stack_root.join(".env"),
            &self.env_values_docker_control_plane(),
        )?;
        Ok(())
    }

    fn write_node_docker_stack(&self, stack_root: &Path) -> Result<()> {
        self.system.copy_file(
            &stack_root.join("node.compose.yml"),
            &stack_root.join("compose.yml"),
        )?;
        fs::create_dir_all(stack_root.join("data").join("xray"))?;
        fs::create_dir_all(stack_root.join("data").join("singbox"))?;
        fs::create_dir_all(stack_root.join("data").join("tls"))?;
        let node = self
            .config
            .node
            .as_ref()
            .ok_or_else(|| anyhow!("missing node config"))?;
        self.system
            .cleanup_runtime_placeholder_configs(&stack_root.join("data"))?;
        self.system.generate_self_signed_cert(
            &stack_root.join("data").join("tls").join("server.crt"),
            &stack_root.join("data").join("tls").join("server.key"),
            &node.name,
        )?;
        write_kv_file(&stack_root.join(".env"), &self.env_values_docker_node())?;
        Ok(())
    }

    fn env_values_native(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        if let Some(control_plane) = self.config.control_plane.as_ref() {
            values.insert("ANNEAL_BIND_ADDRESS".into(), "127.0.0.1:8080".into());
            values.insert(
                "ANNEAL_DATABASE_URL".into(),
                control_plane.database_url.clone(),
            );
            values.insert(
                "ANNEAL_MIGRATIONS_DIR".into(),
                self.layout.migrations_dir().display().to_string(),
            );
            if let Some(token) = control_plane.bootstrap_token.as_ref() {
                values.insert("ANNEAL_BOOTSTRAP_TOKEN".into(), token.clone());
            }
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
                "ANNEAL_PUBLIC_BASE_URL".into(),
                control_plane.public_base_url.clone(),
            );
            values.insert("ANNEAL_CADDY_DOMAIN".into(), control_plane.domain.clone());
            if let Some(endpoint) = control_plane.otlp_endpoint.as_ref() {
                values.insert("ANNEAL_OTLP_ENDPOINT".into(), endpoint.clone());
            }
            values.insert(
                "ANNEAL_SUPERADMIN_EMAIL".into(),
                control_plane.superadmin.email.clone(),
            );
            values.insert(
                "ANNEAL_SUPERADMIN_PASSWORD".into(),
                control_plane.superadmin.password.clone(),
            );
        }
        if let Some(node) = self.config.node.as_ref() {
            fill_node_env(
                &mut values,
                node,
                self.config.deployment_mode,
                &self.layout,
                self.config.release_version.as_deref().unwrap_or_default(),
            );
        }
        values
    }

    fn env_values_docker_control_plane(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        let control_plane = self
            .config
            .control_plane
            .as_ref()
            .expect("control-plane config");
        let database = System::parse_database_url(&control_plane.database_url).expect("database");
        values.insert("ANNEAL_DB_NAME".into(), database.name);
        values.insert("ANNEAL_DB_USER".into(), database.user);
        values.insert("ANNEAL_DB_PASSWORD".into(), database.password);
        values.insert("ANNEAL_BIND_ADDRESS".into(), "0.0.0.0:8080".into());
        values.insert(
            "ANNEAL_DATABASE_URL".into(),
            format!(
                "postgres://{}:{}@postgres:5432/{}",
                values["ANNEAL_DB_USER"], values["ANNEAL_DB_PASSWORD"], values["ANNEAL_DB_NAME"]
            ),
        );
        values.insert(
            "ANNEAL_MIGRATIONS_DIR".into(),
            "/opt/anneal/migrations".into(),
        );
        if let Some(token) = control_plane.bootstrap_token.as_ref() {
            values.insert("ANNEAL_BOOTSTRAP_TOKEN".into(), token.clone());
        }
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
            "ANNEAL_PUBLIC_BASE_URL".into(),
            control_plane.public_base_url.clone(),
        );
        values.insert("ANNEAL_CADDY_DOMAIN".into(), control_plane.domain.clone());
        if let Some(endpoint) = control_plane.otlp_endpoint.as_ref() {
            values.insert("ANNEAL_OTLP_ENDPOINT".into(), endpoint.clone());
        }
        values.insert(
            "ANNEAL_SUPERADMIN_EMAIL".into(),
            control_plane.superadmin.email.clone(),
        );
        values.insert(
            "ANNEAL_SUPERADMIN_PASSWORD".into(),
            control_plane.superadmin.password.clone(),
        );
        values
    }

    fn env_values_docker_node(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        let node = self.config.node.as_ref().expect("node config");
        fill_node_env(
            &mut values,
            node,
            DeploymentMode::Docker,
            &self.layout,
            self.config.release_version.as_deref().unwrap_or_default(),
        );
        values
    }

    fn summary_values(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        if let Some(control_plane) = self.config.control_plane.as_ref() {
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
                "ANNEAL_DATABASE_URL".into(),
                control_plane.database_url.clone(),
            );
        }
        if let Some(node) = self.config.node.as_ref() {
            values.insert("ANNEAL_AGENT_SERVER_URL".into(), node.server_url.clone());
            values.insert("ANNEAL_AGENT_NAME".into(), node.name.clone());
            values.insert("ANNEAL_AGENT_ENGINES".into(), engines_csv(&node.engines));
        }
        if let Some(tenant_name) = self
            .config
            .control_plane
            .as_ref()
            .and_then(|control_plane| control_plane.reseller.as_ref())
            .map(|reseller| reseller.tenant_name.clone())
        {
            values.insert("ANNEAL_RESELLER_TENANT_NAME".into(), tenant_name);
        }
        if let Some(reseller) = self
            .config
            .control_plane
            .as_ref()
            .and_then(|control_plane| control_plane.reseller.as_ref())
        {
            values.insert("ANNEAL_RESELLER_EMAIL".into(), reseller.email.clone());
            values.insert("ANNEAL_RESELLER_PASSWORD".into(), reseller.password.clone());
        }
        if let Some(node) = self.config.node.as_ref() {
            if let Some(group_name) = node.group_name.as_ref() {
                values.insert("ANNEAL_NODE_GROUP_NAME".into(), group_name.clone());
            }
        }
        if let Some(name) = self.state.bootstrap.starter_subscription_name.as_ref() {
            values.insert("ANNEAL_STARTER_SUBSCRIPTION_NAME".into(), name.clone());
        }
        if let Some(url) = self.state.bootstrap.starter_subscription_url.as_ref() {
            values.insert("ANNEAL_STARTER_SUBSCRIPTION_URL".into(), url.clone());
        }
        if let Some(version) = self.config.release_version.as_ref() {
            values.insert("ANNEAL_VERSION".into(), version.clone());
        }
        values
    }

    fn rewrite_docker_env(&self) -> Result<()> {
        if self.config.role.includes_control_plane() {
            let stack_root = self.layout.docker_stack_root(InstallRole::ControlPlane);
            write_kv_file(
                &stack_root.join(".env"),
                &self.env_values_docker_control_plane(),
            )?;
            self.system
                .docker_compose_up(&stack_root, &stack_root.join(".env"), false)?;
        }
        if self.config.role.includes_node() {
            let stack_root = self.layout.docker_stack_root(InstallRole::Node);
            write_kv_file(&stack_root.join(".env"), &self.env_values_docker_node())?;
            self.system
                .docker_compose_up(&stack_root, &stack_root.join(".env"), false)?;
        }
        Ok(())
    }

    fn begin_step(&mut self, step: InstallStep, detail: &str) -> Result<()> {
        self.state.mark_running(step, detail);
        self.persist()?;
        self.reporter.step_started(step, detail);
        Ok(())
    }

    fn complete_step(&mut self, step: InstallStep, detail: &str) -> Result<()> {
        self.state.mark_completed(step, detail);
        self.persist()?;
        self.reporter.step_completed(step, detail);
        Ok(())
    }

    fn fail_step(&mut self, step: InstallStep, detail: &str) -> Result<()> {
        self.state.mark_failed(step, detail);
        self.persist()?;
        self.reporter.step_failed(step, detail);
        Ok(())
    }

    fn persist(&mut self) -> Result<()> {
        self.config.save(&self.layout.config_path)?;
        self.state.release_version = self.config.release_version.clone();
        self.state.save(&self.layout.state_path)
    }
}

fn default_translator() -> Translator {
    Translator::new(Language::resolve(None))
}

fn required_bundle_root(value: Option<PathBuf>) -> Result<PathBuf> {
    value.ok_or_else(|| anyhow!("--bundle-root is required"))
}

fn fill_node_env(
    values: &mut BTreeMap<String, String>,
    node: &NodeConfig,
    deployment_mode: DeploymentMode,
    layout: &InstallLayout,
    version: &str,
) {
    values.insert("ANNEAL_AGENT_SERVER_URL".into(), node.server_url.clone());
    values.insert("ANNEAL_AGENT_NAME".into(), node.name.clone());
    values.insert("ANNEAL_AGENT_VERSION".into(), version.to_owned());
    values.insert("ANNEAL_AGENT_ENGINES".into(), engines_csv(&node.engines));
    values.insert(
        "ANNEAL_AGENT_PROTOCOLS_XRAY".into(),
        protocols_csv(&node.protocols_xray),
    );
    values.insert(
        "ANNEAL_AGENT_PROTOCOLS_SINGBOX".into(),
        protocols_csv(&node.protocols_singbox),
    );
    if let Some(token) = node.bootstrap_token.as_ref() {
        values.insert("ANNEAL_AGENT_BOOTSTRAP_TOKEN".into(), token.clone());
    }
    values.insert("ANNEAL_AGENT_CONFIG_ROOT".into(), "/var/lib/anneal".into());
    match deployment_mode {
        DeploymentMode::Native => {
            values.insert(
                "ANNEAL_AGENT_XRAY_BINARY".into(),
                layout.bin_dir().join("xray").display().to_string(),
            );
            values.insert(
                "ANNEAL_AGENT_SINGBOX_BINARY".into(),
                layout.bin_dir().join("hiddify-core").display().to_string(),
            );
            values.insert("ANNEAL_AGENT_RUNTIME_CONTROLLER".into(), "systemctl".into());
            values.insert(
                "ANNEAL_AGENT_SYSTEMCTL_BINARY".into(),
                "/usr/bin/systemctl".into(),
            );
            values.insert(
                "ANNEAL_AGENT_XRAY_SERVICE".into(),
                "anneal-xray.service".into(),
            );
            values.insert(
                "ANNEAL_AGENT_SINGBOX_SERVICE".into(),
                "anneal-singbox.service".into(),
            );
        }
        DeploymentMode::Docker => {
            values.insert(
                "ANNEAL_AGENT_RUNTIME_CONTROLLER".into(),
                "supervisorctl".into(),
            );
            values.insert(
                "ANNEAL_AGENT_SYSTEMCTL_BINARY".into(),
                "/usr/bin/supervisorctl".into(),
            );
            values.insert("ANNEAL_AGENT_XRAY_SERVICE".into(), "xray".into());
            values.insert("ANNEAL_AGENT_SINGBOX_SERVICE".into(), "singbox".into());
        }
    }
}

fn node_engine_names(node: &NodeConfig) -> Vec<&'static str> {
    node.engines
        .iter()
        .map(|engine| match engine {
            anneal_core::ProxyEngine::Xray => "xray",
            anneal_core::ProxyEngine::Singbox => "singbox",
        })
        .collect()
}

fn bootstrap_execution_order(role: InstallRole) -> Vec<InstallStep> {
    let mut steps = Vec::new();
    if role.includes_control_plane() {
        steps.push(InstallStep::ControlPlaneBootstrap);
    }
    if role == InstallRole::AllInOne {
        steps.push(InstallStep::StarterSubscription);
    }
    if role.includes_node() {
        steps.push(InstallStep::NodeBootstrap);
    }
    steps
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anneal_core::{ProtocolKind, ProxyEngine};
    use chrono::Utc;

    use super::*;
    use crate::{
        config::{
            AdminConfig, ControlPlaneConfig, InstallLayout, ResellerConfig,
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
            docker_root: PathBuf::from("/opt/test-anneal/docker"),
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
                    node_agent: "bin/node-agent".into(),
                    annealctl: Some("bin/annealctl".into()),
                    xray: "runtime/xray".into(),
                    singbox: "runtime/hiddify-core".into(),
                    web: "web".into(),
                    migrations: "migrations".into(),
                    deploy: "deploy".into(),
                },
            },
        }
    }

    fn sample_config(role: InstallRole, mode: DeploymentMode) -> InstallConfig {
        let control_plane = role.includes_control_plane().then(|| ControlPlaneConfig {
            domain: "panel.example.com".into(),
            panel_path: "private-path".into(),
            public_base_url: "https://panel.example.com/private-path".into(),
            database_url: "postgres://anneal:secret@127.0.0.1:5432/anneal".into(),
            bootstrap_token: Some("bootstrap-token".into()),
            data_encryption_key: "a".repeat(64),
            token_hash_key: "b".repeat(64),
            access_jwt_secret: "c".repeat(64),
            pre_auth_jwt_secret: "d".repeat(64),
            otlp_endpoint: Some("http://127.0.0.1:4317".into()),
            superadmin: AdminConfig {
                email: "admin@panel.example.com".into(),
                display_name: "Superadmin".into(),
                password: "superadmin-password".into(),
            },
            reseller: Some(ResellerConfig {
                tenant_name: "Default Tenant".into(),
                email: "tenant@panel.example.com".into(),
                display_name: "Tenant Admin".into(),
                password: "tenant-password".into(),
            }),
            starter_subscription: Some(StarterSubscriptionConfig {
                name: "Starter access".into(),
                traffic_limit_bytes: 1_099_511_627_776,
                days: 3650,
            }),
        });
        let node = role.includes_node().then(|| NodeConfig {
            server_url: if role == InstallRole::AllInOne {
                "http://127.0.0.1:8080".into()
            } else {
                "https://panel.example.com/private-path".into()
            },
            bootstrap_token: Some("node-bootstrap-token".into()),
            name: "edge-main".into(),
            group_name: Some("edge-main".into()),
            engines: vec![ProxyEngine::Xray, ProxyEngine::Singbox],
            protocols_xray: vec![
                ProtocolKind::VlessReality,
                ProtocolKind::Vmess,
                ProtocolKind::Trojan,
                ProtocolKind::Shadowsocks2022,
            ],
            protocols_singbox: vec![
                ProtocolKind::VlessReality,
                ProtocolKind::Vmess,
                ProtocolKind::Trojan,
                ProtocolKind::Shadowsocks2022,
                ProtocolKind::Tuic,
                ProtocolKind::Hysteria2,
            ],
        });
        InstallConfig {
            role,
            deployment_mode: mode,
            release_version: Some("0.1.0".into()),
            install_user: "anneal".into(),
            install_group: "anneal".into(),
            control_plane,
            node,
        }
    }

    fn sample_state(role: InstallRole, mode: DeploymentMode) -> InstallState {
        InstallState {
            role,
            deployment_mode: mode,
            release_version: Some("0.1.0".into()),
            started_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            steps: BTreeMap::new(),
            bootstrap: BootstrapState {
                superadmin_totp_secret: Some("totp-secret".into()),
                tenant_id: None,
                node_group_id: None,
                starter_subscription_name: Some("Starter access".into()),
                starter_subscription_url: Some("https://panel.example.com/s/token".into()),
            },
        }
    }

    fn installer(role: InstallRole, mode: DeploymentMode) -> Installer {
        Installer::new(
            test_layout(),
            release_bundle(),
            sample_config(role, mode),
            sample_state(role, mode),
        )
    }

    #[test]
    fn native_node_env_contains_runtime_launch_settings_for_both_cores() {
        let values = installer(InstallRole::AllInOne, DeploymentMode::Native).env_values_native();

        assert_eq!(
            values.get("ANNEAL_AGENT_SERVER_URL"),
            Some(&"http://127.0.0.1:8080".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_ENGINES"),
            Some(&"xray,singbox".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_PROTOCOLS_XRAY"),
            Some(&"vless_reality,vmess,trojan,shadowsocks_2022".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_PROTOCOLS_SINGBOX"),
            Some(&"vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2".into())
        );
        assert!(
            normalize_path(values.get("ANNEAL_AGENT_XRAY_BINARY").expect("xray binary"))
                .ends_with("/opt/test-anneal/bin/xray")
        );
        assert!(
            normalize_path(
                values
                    .get("ANNEAL_AGENT_SINGBOX_BINARY")
                    .expect("singbox binary")
            )
            .ends_with("/opt/test-anneal/bin/hiddify-core")
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_RUNTIME_CONTROLLER"),
            Some(&"systemctl".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_XRAY_SERVICE"),
            Some(&"anneal-xray.service".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_SINGBOX_SERVICE"),
            Some(&"anneal-singbox.service".into())
        );
    }

    #[test]
    fn docker_node_env_switches_to_supervisor_for_runtime_restart() {
        let values = installer(InstallRole::Node, DeploymentMode::Docker).env_values_docker_node();

        assert_eq!(
            values.get("ANNEAL_AGENT_RUNTIME_CONTROLLER"),
            Some(&"supervisorctl".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_SYSTEMCTL_BINARY"),
            Some(&"/usr/bin/supervisorctl".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_XRAY_SERVICE"),
            Some(&"xray".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_SINGBOX_SERVICE"),
            Some(&"singbox".into())
        );
    }

    #[test]
    fn docker_control_plane_env_rewrites_database_url_for_container_network() {
        let values = installer(InstallRole::ControlPlane, DeploymentMode::Docker)
            .env_values_docker_control_plane();

        assert_eq!(values.get("ANNEAL_DB_NAME"), Some(&"anneal".into()));
        assert_eq!(values.get("ANNEAL_DB_USER"), Some(&"anneal".into()));
        assert_eq!(values.get("ANNEAL_DB_PASSWORD"), Some(&"secret".into()));
        assert_eq!(
            values.get("ANNEAL_DATABASE_URL"),
            Some(&"postgres://anneal:secret@postgres:5432/anneal".into())
        );
    }

    #[test]
    fn summary_keeps_generated_access_and_runtime_data() {
        let values = installer(InstallRole::AllInOne, DeploymentMode::Native).summary_values();

        assert_eq!(
            values.get("ANNEAL_PUBLIC_BASE_URL"),
            Some(&"https://panel.example.com/private-path".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_SERVER_URL"),
            Some(&"http://127.0.0.1:8080".into())
        );
        assert_eq!(
            values.get("ANNEAL_PANEL_PATH"),
            Some(&"private-path".into())
        );
        assert_eq!(
            values.get("ANNEAL_AGENT_ENGINES"),
            Some(&"xray,singbox".into())
        );
        assert_eq!(
            values.get("ANNEAL_STARTER_SUBSCRIPTION_URL"),
            Some(&"https://panel.example.com/s/token".into())
        );
        assert_eq!(
            values.get("ANNEAL_RESELLER_EMAIL"),
            Some(&"tenant@panel.example.com".into())
        );
    }

    #[test]
    fn service_templates_restart_automatically_after_vps_reboot() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for file_name in [
            "anneal-api.service",
            "anneal-worker.service",
            "anneal-node-agent.service",
            "anneal-caddy.service",
            "anneal-xray.service",
            "anneal-singbox.service",
        ] {
            let path = root.join("deploy").join("systemd").join(file_name);
            let raw = fs::read_to_string(&path).expect("unit");
            assert!(raw.contains("Restart=always"), "{file_name}");
            assert!(raw.contains("WantedBy=multi-user.target"), "{file_name}");
        }
    }

    #[test]
    fn docker_templates_restart_services_and_runtime_cores() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let control_plane_compose = fs::read_to_string(
            root.join("deploy")
                .join("docker")
                .join("prebuilt")
                .join("control-plane.compose.yml"),
        )
        .expect("control-plane compose");
        let node_compose = fs::read_to_string(
            root.join("deploy")
                .join("docker")
                .join("prebuilt")
                .join("node.compose.yml"),
        )
        .expect("node compose");
        let supervisor = fs::read_to_string(
            root.join("deploy")
                .join("docker")
                .join("prebuilt")
                .join("node-supervisord.conf"),
        )
        .expect("supervisord");

        assert!(control_plane_compose.contains("restart: unless-stopped"));
        assert!(node_compose.contains("restart: unless-stopped"));
        assert!(supervisor.contains("[program:xray]"));
        assert!(supervisor.contains("[program:singbox]"));
        assert!(supervisor.contains("[program:node-agent]"));
        assert!(supervisor.contains("autorestart=true"));
    }

    #[test]
    fn all_in_one_bootstrap_order_creates_subscription_before_node_wait() {
        assert_eq!(
            bootstrap_execution_order(InstallRole::AllInOne),
            vec![
                InstallStep::ControlPlaneBootstrap,
                InstallStep::StarterSubscription,
                InstallStep::NodeBootstrap,
            ]
        );
    }

    #[test]
    fn standalone_node_bootstrap_order_skips_starter_subscription() {
        assert_eq!(
            bootstrap_execution_order(InstallRole::Node),
            vec![InstallStep::NodeBootstrap]
        );
    }

    fn normalize_path(value: &str) -> String {
        value.replace('\\', "/")
    }
}
