use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::config::InstallRole;

#[derive(Debug, Clone)]
pub struct ReleaseBundle {
    pub root: PathBuf,
    pub manifest: ReleaseManifest,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub paths: ManifestPaths,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestPaths {
    pub api: String,
    pub worker: String,
    pub node_agent: String,
    pub annealctl: Option<String>,
    pub xray: String,
    pub singbox: String,
    pub web: String,
    pub migrations: String,
    pub deploy: String,
}

impl ReleaseBundle {
    pub fn load(root: &Path) -> Result<Self> {
        let manifest_path = root.join("release-manifest.json");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let manifest: ReleaseManifest =
            serde_json::from_str(&manifest_raw).context("failed to parse release manifest")?;
        let bundle = Self {
            root: root.to_path_buf(),
            manifest,
        };
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn validate(&self) -> Result<()> {
        let required = [
            self.api_path(),
            self.worker_path(),
            self.node_agent_path(),
            self.annealctl_path(),
            self.xray_path(),
            self.singbox_path(),
            self.web_dir(),
            self.migrations_dir(),
            self.deploy_dir(),
        ];
        for path in required {
            if !path.exists() {
                bail!("missing bundle path: {}", path.display());
            }
        }
        Ok(())
    }

    pub fn validate_for(&self, role: InstallRole) -> Result<()> {
        if role.includes_control_plane() {
            self.require(&self.api_path())?;
            self.require(&self.worker_path())?;
            self.require(&self.web_dir())?;
            self.require(&self.migrations_dir())?;
        }
        if role.includes_node() {
            self.require(&self.node_agent_path())?;
            self.require(&self.xray_path())?;
            self.require(&self.singbox_path())?;
        }
        self.require(&self.annealctl_path())?;
        Ok(())
    }

    pub fn api_path(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.api)
    }

    pub fn worker_path(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.worker)
    }

    pub fn node_agent_path(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.node_agent)
    }

    pub fn annealctl_path(&self) -> PathBuf {
        self.root.join(
            self.manifest
                .paths
                .annealctl
                .as_deref()
                .unwrap_or("bin/annealctl"),
        )
    }

    pub fn xray_path(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.xray)
    }

    pub fn singbox_path(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.singbox)
    }

    pub fn web_dir(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.web)
    }

    pub fn migrations_dir(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.migrations)
    }

    pub fn deploy_dir(&self) -> PathBuf {
        self.root.join(&self.manifest.paths.deploy)
    }

    pub fn deploy_asset(&self, relative: &str) -> Result<PathBuf> {
        let path = self.deploy_dir().join(relative);
        self.require(&path)?;
        Ok(path)
    }

    fn require(&self, path: &Path) -> Result<()> {
        if path.exists() {
            Ok(())
        } else {
            Err(anyhow!("missing bundle path: {}", path.display()))
        }
    }
}
