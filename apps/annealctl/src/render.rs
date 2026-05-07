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
    let replaced = replace_base_href(&raw, &panel_base_href(panel_path));
    fs::write(index_path, replaced)
        .with_context(|| format!("failed to write {}", index_path.display()))?;
    Ok(())
}

fn replace_base_href(raw: &str, base_href: &str) -> String {
    let Some(base_start) = raw.find("<base") else {
        return raw.to_owned();
    };
    let Some(base_end) = raw[base_start..]
        .find('>')
        .map(|offset| base_start + offset + 1)
    else {
        return raw.to_owned();
    };
    let base_tag = &raw[base_start..base_end];
    let Some(href_start) = base_tag.find("href=\"") else {
        return raw.replace("<base", &format!("<base href=\"{base_href}\""));
    };
    let value_start = base_start + href_start + "href=\"".len();
    let Some(value_end) = raw[value_start..]
        .find('"')
        .map(|offset| value_start + offset)
    else {
        return raw.to_owned();
    };

    let mut replaced = String::with_capacity(raw.len() + base_href.len());
    replaced.push_str(&raw[..value_start]);
    replaced.push_str(base_href);
    replaced.push_str(&raw[value_end..]);
    replaced
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

pub fn render_mihomo_config() -> String {
    [
        "mixed-port: 7890",
        "allow-lan: false",
        "mode: rule",
        "log-level: warning",
        "proxies: []",
        "proxy-groups:",
        "  - name: \"Anneal\"",
        "    type: select",
        "    proxies:",
        "      - DIRECT",
        "rules:",
        "  - MATCH,DIRECT",
        "",
    ]
    .join("\n")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_href_rewrite_updates_existing_secret_path() {
        let raw = r#"<base href="/old-secret/" />"#;

        let rewritten = replace_base_href(raw, "/new-secret/");

        assert_eq!(rewritten, r#"<base href="/new-secret/" />"#);
    }

    #[test]
    fn caddyfile_routes_generated_panel_path() {
        let dir = std::env::temp_dir().join(format!("anneal-render-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("tempdir");
        let template = dir.join("Caddyfile.tpl");
        fs::write(
            &template,
            "handle {{PANEL_BASE_PATH}}/assets/*\nhandle_path {{PANEL_BASE_PATH}}/*",
        )
        .expect("template");

        let rendered =
            render_caddyfile(&template, "panel.example.com", "generated-secret").expect("rendered");

        assert!(rendered.contains("handle /generated-secret/assets/*"));
        assert!(rendered.contains("handle_path /generated-secret/*"));
        assert!(!rendered.contains("/panel"));
        let _ = fs::remove_dir_all(dir);
    }
}
