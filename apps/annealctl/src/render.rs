use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result};

use crate::config::{panel_base_href, panel_path_prefix};

pub fn render_caddyfile(template: &Path, domain: &str, panel_path: &str) -> Result<String> {
    let raw = fs::read_to_string(template)
        .with_context(|| format!("failed to read {}", template.display()))?;
    Ok(raw
        .replace("{{DOMAIN}}", domain)
        .replace("{{SITE_ADDRESS}}", domain)
        .replace("{{PANEL_BASE_PATH}}", &panel_path_prefix(panel_path)))
}

pub fn rewrite_panel_base_href(index_path: &Path, panel_path: &str) -> Result<()> {
    let raw = fs::read_to_string(index_path)
        .with_context(|| format!("failed to read {}", index_path.display()))?;
    let replaced = raw.replace(
        "<base href=\"/\" />",
        &format!("<base href=\"{}\" />", panel_base_href(panel_path)),
    );
    fs::write(index_path, replaced)
        .with_context(|| format!("failed to write {}", index_path.display()))?;
    Ok(())
}

pub fn write_kv_file(path: &Path, values: &BTreeMap<String, String>) -> Result<()> {
    let mut raw = String::new();
    for (key, value) in values {
        raw.push_str(key);
        raw.push('=');
        raw.push_str(&escape_env_value(value));
        raw.push('\n');
    }
    fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))?;
    set_owner_only_permissions(path)?;
    Ok(())
}

fn escape_env_value(value: &str) -> String {
    if value.chars().all(|char| {
        char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '.' | '/' | ':' | ',')
    }) {
        return value.to_owned();
    }
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
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
