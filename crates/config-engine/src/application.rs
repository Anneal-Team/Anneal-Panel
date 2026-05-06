use anneal_core::{ApplicationError, ApplicationResult, ProtocolKind, ProxyEngine};

use crate::domain::{
    CanonicalConfig, ClientCredential, InboundProfile, SecurityKind, TransportKind,
};

pub trait RendererStrategy: Send + Sync {
    fn render(&self, config: &CanonicalConfig) -> ApplicationResult<String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ConfigRenderer;

impl ConfigRenderer {
    pub fn render(&self, config: &CanonicalConfig) -> ApplicationResult<String> {
        match config.engine {
            ProxyEngine::Mihomo => MihomoRenderer.render(config),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MihomoRenderer;

impl RendererStrategy for MihomoRenderer {
    fn render(&self, config: &CanonicalConfig) -> ApplicationResult<String> {
        validate_profiles(&config.inbound_profiles)?;
        if config.credentials.is_empty() {
            return Err(ApplicationError::Validation(
                "at least one credential is required for mihomo config".into(),
            ));
        }

        let mut proxies = Vec::new();
        for profile in &config.inbound_profiles {
            for credential in &config.credentials {
                proxies.push(render_mihomo_proxy(config, profile, credential)?);
            }
        }
        let proxy_names = proxies
            .iter()
            .filter_map(|proxy| proxy.lines().next())
            .filter_map(|line| line.strip_prefix("  - name: "))
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let mut lines = vec![
            "mixed-port: 7890".to_owned(),
            "allow-lan: false".to_owned(),
            "mode: rule".to_owned(),
            "log-level: warning".to_owned(),
            "proxies:".to_owned(),
        ];
        lines.extend(proxies);
        lines.extend([
            "proxy-groups:".to_owned(),
            "  - name: \"Anneal\"".to_owned(),
            "    type: select".to_owned(),
            "    proxies:".to_owned(),
        ]);
        lines.extend(
            proxy_names
                .into_iter()
                .map(|name| format!("      - {name}")),
        );
        lines.extend([
            "      - DIRECT".to_owned(),
            "rules:".to_owned(),
            "  - MATCH,Anneal".to_owned(),
        ]);
        Ok(lines.join("\n"))
    }
}

fn validate_profiles(profiles: &[InboundProfile]) -> ApplicationResult<()> {
    if profiles.is_empty() {
        return Err(ApplicationError::Validation(
            "at least one mihomo profile is required".into(),
        ));
    }
    for profile in profiles {
        if profile.public_host.trim().is_empty() {
            return Err(ApplicationError::Validation(
                "mihomo profile public_host is required".into(),
            ));
        }
        if profile.security == SecurityKind::Reality
            && (profile.server_name.is_none()
                || profile.reality_public_key.is_none()
                || profile.reality_short_id.is_none())
        {
            return Err(ApplicationError::Validation(
                "mihomo reality profile requires server_name, public key and short_id".into(),
            ));
        }
        if profile.protocol == ProtocolKind::Shadowsocks2022 && profile.cipher.is_none() {
            return Err(ApplicationError::Validation(
                "shadowsocks_2022 profile requires cipher".into(),
            ));
        }
    }
    Ok(())
}

fn render_mihomo_proxy(
    config: &CanonicalConfig,
    profile: &InboundProfile,
    credential: &ClientCredential,
) -> ApplicationResult<String> {
    let name = proxy_name(config, profile, credential);
    let mut lines = vec![
        format!("  - name: {}", yaml_quote(&name)),
        format!("    type: {}", mihomo_protocol_name(profile.protocol)),
        format!("    server: {}", yaml_quote(&profile.public_host)),
        format!("    port: {}", profile.public_port),
    ];

    match profile.protocol {
        ProtocolKind::VlessReality => {
            lines.push(format!("    uuid: {}", yaml_quote(&credential.uuid)));
            lines.push("    udp: true".into());
            lines.push(format!(
                "    network: {}",
                mihomo_transport_name(profile.transport)
            ));
            append_tls_flags(&mut lines, profile);
            if let Some(flow) = &profile.flow {
                lines.push(format!("    flow: {}", yaml_quote(flow)));
            }
            append_reality_options(&mut lines, profile);
            append_transport_options(&mut lines, profile);
        }
        ProtocolKind::Vmess => {
            lines.push(format!("    uuid: {}", yaml_quote(&credential.uuid)));
            lines.push("    alterId: 0".into());
            lines.push("    cipher: auto".into());
            lines.push("    udp: true".into());
            lines.push(format!(
                "    network: {}",
                mihomo_transport_name(profile.transport)
            ));
            append_tls_flags(&mut lines, profile);
            append_transport_options(&mut lines, profile);
        }
        ProtocolKind::Trojan => {
            lines.push(format!(
                "    password: {}",
                yaml_quote(required_password(credential, profile.protocol)?)
            ));
            lines.push("    udp: true".into());
            lines.push(format!(
                "    network: {}",
                mihomo_transport_name(profile.transport)
            ));
            append_tls_flags(&mut lines, profile);
            append_transport_options(&mut lines, profile);
        }
        ProtocolKind::Shadowsocks2022 => {
            lines.push(format!(
                "    cipher: {}",
                yaml_quote(profile.cipher.as_deref().unwrap_or_default())
            ));
            lines.push(format!(
                "    password: {}",
                yaml_quote(required_password(credential, profile.protocol)?)
            ));
            lines.push("    udp: true".into());
        }
        ProtocolKind::Tuic => {
            lines.push(format!("    uuid: {}", yaml_quote(&credential.uuid)));
            lines.push(format!(
                "    password: {}",
                yaml_quote(required_password(credential, profile.protocol)?)
            ));
            lines.push("    udp: true".into());
            lines.push("    congestion-controller: bbr".into());
            append_tls_flags(&mut lines, profile);
            append_alpn(&mut lines, profile);
        }
        ProtocolKind::Hysteria2 => {
            lines.push(format!(
                "    password: {}",
                yaml_quote(required_password(credential, profile.protocol)?)
            ));
            lines.push("    udp: true".into());
            append_tls_flags(&mut lines, profile);
            append_alpn(&mut lines, profile);
        }
    }

    Ok(lines.join("\n"))
}

fn proxy_name(
    config: &CanonicalConfig,
    profile: &InboundProfile,
    credential: &ClientCredential,
) -> String {
    format!(
        "{} {} {} {} {}",
        config.tag,
        credential.email,
        protocol_tag(profile.protocol),
        profile.public_host,
        profile.public_port
    )
}

fn append_tls_flags(lines: &mut Vec<String>, profile: &InboundProfile) {
    lines.push(format!(
        "    tls: {}",
        if profile.security == SecurityKind::None {
            "false"
        } else {
            "true"
        }
    ));
    if let Some(server_name) = &profile.server_name {
        lines.push(format!("    servername: {}", yaml_quote(server_name)));
        lines.push(format!("    sni: {}", yaml_quote(server_name)));
    }
    if let Some(fingerprint) = &profile.fingerprint {
        lines.push(format!(
            "    client-fingerprint: {}",
            yaml_quote(fingerprint)
        ));
    }
}

fn append_reality_options(lines: &mut Vec<String>, profile: &InboundProfile) {
    if profile.security != SecurityKind::Reality {
        return;
    }
    lines.push("    reality-opts:".into());
    if let Some(public_key) = &profile.reality_public_key {
        lines.push(format!("      public-key: {}", yaml_quote(public_key)));
    }
    if let Some(short_id) = &profile.reality_short_id {
        lines.push(format!("      short-id: {}", yaml_quote(short_id)));
    }
}

fn append_transport_options(lines: &mut Vec<String>, profile: &InboundProfile) {
    match profile.transport {
        TransportKind::Tcp => {}
        TransportKind::Ws | TransportKind::HttpUpgrade => {
            lines.push("    ws-opts:".into());
            if let Some(path) = &profile.path {
                lines.push(format!("      path: {}", yaml_quote(path)));
            }
            if let Some(host_header) = &profile.host_header {
                lines.push("      headers:".into());
                lines.push(format!("        Host: {}", yaml_quote(host_header)));
            }
        }
        TransportKind::Grpc => {
            lines.push("    grpc-opts:".into());
            if let Some(service_name) = &profile.service_name {
                lines.push(format!(
                    "      grpc-service-name: {}",
                    yaml_quote(service_name)
                ));
            }
        }
    }
}

fn append_alpn(lines: &mut Vec<String>, profile: &InboundProfile) {
    if profile.alpn.is_empty() {
        return;
    }
    lines.push("    alpn:".into());
    lines.extend(
        profile
            .alpn
            .iter()
            .map(|value| format!("      - {}", yaml_quote(value))),
    );
}

fn required_password(
    credential: &ClientCredential,
    protocol: ProtocolKind,
) -> ApplicationResult<&str> {
    credential
        .password
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApplicationError::Validation(format!(
                "{} profile requires credential password",
                protocol_tag(protocol)
            ))
        })
}

fn yaml_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn mihomo_protocol_name(protocol: ProtocolKind) -> &'static str {
    match protocol {
        ProtocolKind::VlessReality => "vless",
        ProtocolKind::Vmess => "vmess",
        ProtocolKind::Trojan => "trojan",
        ProtocolKind::Shadowsocks2022 => "ss",
        ProtocolKind::Tuic => "tuic",
        ProtocolKind::Hysteria2 => "hysteria2",
    }
}

fn protocol_tag(protocol: ProtocolKind) -> &'static str {
    match protocol {
        ProtocolKind::VlessReality => "vless",
        ProtocolKind::Vmess => "vmess",
        ProtocolKind::Trojan => "trojan",
        ProtocolKind::Shadowsocks2022 => "ss2022",
        ProtocolKind::Tuic => "tuic",
        ProtocolKind::Hysteria2 => "hy2",
    }
}

fn mihomo_transport_name(transport: TransportKind) -> &'static str {
    match transport {
        TransportKind::Tcp => "tcp",
        TransportKind::Ws => "ws",
        TransportKind::Grpc => "grpc",
        TransportKind::HttpUpgrade => "ws",
    }
}

#[cfg(test)]
mod tests {
    use anneal_core::{ProtocolKind, ProxyEngine};

    use crate::{
        application::{ConfigRenderer, MihomoRenderer, RendererStrategy},
        domain::{CanonicalConfig, ClientCredential, InboundProfile, SecurityKind, TransportKind},
    };

    fn fixture(protocol: ProtocolKind) -> CanonicalConfig {
        CanonicalConfig {
            engine: ProxyEngine::Mihomo,
            tag: "tenant-main".into(),
            server_name: Some("edge.example.com".into()),
            credentials: vec![ClientCredential {
                email: "user@example.com".into(),
                uuid: "11111111-1111-1111-1111-111111111111".into(),
                password: Some("secret".into()),
            }],
            inbound_profiles: vec![InboundProfile {
                protocol,
                listen_host: "::".into(),
                listen_port: 443,
                public_host: "edge.example.com".into(),
                public_port: 443,
                transport: TransportKind::Tcp,
                security: if protocol == ProtocolKind::VlessReality {
                    SecurityKind::Reality
                } else {
                    SecurityKind::Tls
                },
                server_name: Some("edge.example.com".into()),
                host_header: None,
                path: None,
                service_name: None,
                flow: Some("xtls-rprx-vision".into()),
                reality_public_key: Some("public-key".into()),
                reality_private_key: Some("private-key".into()),
                reality_short_id: Some("abcd1234".into()),
                fingerprint: Some("chrome".into()),
                alpn: vec!["h2".into(), "http/1.1".into()],
                cipher: Some("2022-blake3-aes-128-gcm".into()),
                tls_certificate_path: None,
                tls_key_path: None,
            }],
        }
    }

    #[test]
    fn mihomo_renderer_outputs_clash_compatible_yaml() {
        let rendered = MihomoRenderer
            .render(&fixture(ProtocolKind::VlessReality))
            .expect("render");

        assert!(rendered.contains("mixed-port: 7890"));
        assert!(rendered.contains("type: vless"));
        assert!(rendered.contains("reality-opts:"));
        assert!(rendered.contains("proxy-groups:"));
    }

    #[test]
    fn dispatch_renderer_uses_mihomo_strategy() {
        let rendered = ConfigRenderer
            .render(&fixture(ProtocolKind::Trojan))
            .expect("render");

        assert!(rendered.contains("type: trojan"));
        assert!(rendered.contains("MATCH,Anneal"));
    }

    #[test]
    fn password_protocols_require_password() {
        let mut config = fixture(ProtocolKind::Trojan);
        config.credentials[0].password = None;

        let error = ConfigRenderer.render(&config).expect_err("must fail");

        assert_eq!(
            error.to_string(),
            "trojan profile requires credential password"
        );
    }
}
