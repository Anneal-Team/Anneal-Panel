use anneal_core::{ApplicationError, ApplicationResult, ProtocolKind, ProxyEngine};
use serde_json::json;

use crate::domain::{CanonicalConfig, InboundProfile, SecurityKind, TransportKind};

const DEFAULT_TLS_CERTIFICATE_PATH: &str = "/var/lib/anneal/tls/server.crt";
const DEFAULT_TLS_KEY_PATH: &str = "/var/lib/anneal/tls/server.key";

pub trait RendererStrategy: Send + Sync {
    fn render(&self, config: &CanonicalConfig) -> ApplicationResult<String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ConfigRenderer;

impl ConfigRenderer {
    pub fn render(&self, config: &CanonicalConfig) -> ApplicationResult<String> {
        match config.engine {
            ProxyEngine::Xray => XrayRenderer.render(config),
            ProxyEngine::Singbox => SingboxRenderer.render(config),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct XrayRenderer;

impl RendererStrategy for XrayRenderer {
    fn render(&self, config: &CanonicalConfig) -> ApplicationResult<String> {
        validate_profiles(config.engine, &config.inbound_profiles)?;
        let inbounds = config
            .inbound_profiles
            .iter()
            .map(|profile| render_xray_inbound(config, profile))
            .collect::<ApplicationResult<Vec<_>>>()?;
        serde_json::to_string_pretty(&json!({
            "log": { "loglevel": "warning" },
            "inbounds": inbounds,
            "outbounds": [{ "protocol": "freedom", "tag": "direct" }],
        }))
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SingboxRenderer;

impl RendererStrategy for SingboxRenderer {
    fn render(&self, config: &CanonicalConfig) -> ApplicationResult<String> {
        validate_profiles(config.engine, &config.inbound_profiles)?;
        let inbounds = config
            .inbound_profiles
            .iter()
            .map(|profile| render_singbox_inbound(config, profile))
            .collect::<ApplicationResult<Vec<_>>>()?;
        serde_json::to_string_pretty(&json!({
            "log": { "level": "warn" },
            "experimental": { "v2ray_api": { "listen": "127.0.0.1:10085" } },
            "inbounds": inbounds,
            "outbounds": [{ "type": "direct", "tag": "direct" }],
        }))
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }
}

fn validate_profiles(engine: ProxyEngine, profiles: &[InboundProfile]) -> ApplicationResult<()> {
    for profile in profiles {
        if engine == ProxyEngine::Xray
            && matches!(
                profile.protocol,
                ProtocolKind::Tuic | ProtocolKind::Hysteria2
            )
        {
            return Err(ApplicationError::Validation(
                "xray does not support tuic or hysteria2 in v1".into(),
            ));
        }
        if profile.security == SecurityKind::Reality
            && profile.protocol != ProtocolKind::VlessReality
        {
            return Err(ApplicationError::Validation(
                "reality is supported only for vless_reality".into(),
            ));
        }
        if profile.security == SecurityKind::Reality
            && (profile.server_name.is_none()
                || profile.reality_public_key.is_none()
                || profile.reality_private_key.is_none()
                || profile.reality_short_id.is_none())
        {
            return Err(ApplicationError::Validation(
                "reality profile requires server_name, reality keys and short_id".into(),
            ));
        }
        if profile.security == SecurityKind::Tls && profile.server_name.is_none() {
            return Err(ApplicationError::Validation(
                "tls profile requires server_name".into(),
            ));
        }
        if matches!(
            profile.protocol,
            ProtocolKind::Trojan | ProtocolKind::Tuic | ProtocolKind::Hysteria2
        ) && profile.security != SecurityKind::Tls
        {
            return Err(ApplicationError::Validation(
                "trojan, tuic and hysteria2 profiles require tls security".into(),
            ));
        }
        if profile.protocol == ProtocolKind::Shadowsocks2022 && profile.cipher.is_none() {
            return Err(ApplicationError::Validation(
                "shadowsocks_2022 profile requires cipher".into(),
            ));
        }
        if matches!(
            profile.protocol,
            ProtocolKind::Tuic | ProtocolKind::Hysteria2
        ) && profile.alpn.is_empty()
        {
            return Err(ApplicationError::Validation(
                "tuic and hysteria2 profiles require alpn".into(),
            ));
        }
    }
    Ok(())
}

fn render_xray_inbound(
    config: &CanonicalConfig,
    profile: &InboundProfile,
) -> ApplicationResult<serde_json::Value> {
    let tag = format!(
        "{}-{}-{}",
        config.tag,
        protocol_tag(profile.protocol),
        profile.listen_port
    );
    let stream_settings = json!({
        "network": transport_to_wire(profile.transport),
        "security": security_name(profile.security),
        "tlsSettings": render_xray_tls_settings(profile),
        "realitySettings": render_xray_reality_settings(profile),
        "wsSettings": render_xray_ws_settings(profile),
        "grpcSettings": render_xray_grpc_settings(profile),
        "httpupgradeSettings": render_xray_http_upgrade_settings(profile),
    });
    let sniffing = json!({
        "enabled": true,
        "destOverride": ["http", "tls", "quic"]
    });
    match profile.protocol {
        ProtocolKind::VlessReality => Ok(json!({
            "tag": tag,
            "listen": profile.listen_host,
            "port": profile.listen_port,
            "protocol": "vless",
            "settings": {
                "clients": config.credentials.iter().map(|credential| json!({
                    "email": credential.email,
                    "id": credential.uuid,
                    "flow": profile.flow,
                })).collect::<Vec<_>>(),
                "decryption": "none"
            },
            "streamSettings": stream_settings,
            "sniffing": sniffing,
        })),
        ProtocolKind::Vmess => Ok(json!({
            "tag": tag,
            "listen": profile.listen_host,
            "port": profile.listen_port,
            "protocol": "vmess",
            "settings": {
                "clients": config.credentials.iter().map(|credential| json!({
                    "email": credential.email,
                    "id": credential.uuid,
                    "alterId": 0,
                })).collect::<Vec<_>>()
            },
            "streamSettings": stream_settings,
            "sniffing": sniffing,
        })),
        ProtocolKind::Trojan => Ok(json!({
            "tag": tag,
            "listen": profile.listen_host,
            "port": profile.listen_port,
            "protocol": "trojan",
            "settings": {
                "clients": config.credentials.iter().map(|credential| json!({
                    "email": credential.email,
                    "password": credential.password,
                })).collect::<Vec<_>>()
            },
            "streamSettings": stream_settings,
            "sniffing": sniffing,
        })),
        ProtocolKind::Shadowsocks2022 => Ok(json!({
            "tag": tag,
            "listen": profile.listen_host,
            "port": profile.listen_port,
            "protocol": "shadowsocks",
            "settings": {
                "method": profile.cipher,
                "password": config.credentials.first().and_then(|credential| credential.password.clone()),
                "network": "tcp,udp"
            },
            "sniffing": sniffing,
        })),
        ProtocolKind::Tuic | ProtocolKind::Hysteria2 => Err(ApplicationError::Validation(
            "xray does not support tuic or hysteria2 in v1".into(),
        )),
    }
}

fn render_singbox_inbound(
    config: &CanonicalConfig,
    profile: &InboundProfile,
) -> ApplicationResult<serde_json::Value> {
    let tag = format!(
        "{}-{}-{}",
        config.tag,
        protocol_tag(profile.protocol),
        profile.listen_port
    );
    let tls = render_singbox_tls(profile);
    let transport = render_singbox_transport(profile);
    match profile.protocol {
        ProtocolKind::VlessReality => Ok(json!({
            "type": "vless",
            "tag": tag,
            "listen": profile.listen_host,
            "listen_port": profile.listen_port,
            "users": config.credentials.iter().map(|credential| json!({
                "name": credential.email,
                "uuid": credential.uuid,
                "flow": profile.flow,
            })).collect::<Vec<_>>(),
            "tls": tls,
            "transport": transport,
        })),
        ProtocolKind::Vmess => Ok(json!({
            "type": "vmess",
            "tag": tag,
            "listen": profile.listen_host,
            "listen_port": profile.listen_port,
            "users": config.credentials.iter().map(|credential| json!({
                "name": credential.email,
                "uuid": credential.uuid,
            })).collect::<Vec<_>>(),
            "tls": tls,
            "transport": transport,
        })),
        ProtocolKind::Trojan => Ok(json!({
            "type": "trojan",
            "tag": tag,
            "listen": profile.listen_host,
            "listen_port": profile.listen_port,
            "users": config.credentials.iter().map(|credential| json!({
                "name": credential.email,
                "password": credential.password,
            })).collect::<Vec<_>>(),
            "tls": tls,
            "transport": transport,
        })),
        ProtocolKind::Shadowsocks2022 => Ok(json!({
            "type": "shadowsocks",
            "tag": tag,
            "listen": profile.listen_host,
            "listen_port": profile.listen_port,
            "method": profile.cipher,
            "password": config.credentials.first().and_then(|credential| credential.password.clone()),
            "network": "tcp,udp",
        })),
        ProtocolKind::Tuic => Ok(json!({
            "type": "tuic",
            "tag": tag,
            "listen": profile.listen_host,
            "listen_port": profile.listen_port,
            "tcp_fast_open": true,
            "sniff": true,
            "sniff_override_destination": true,
            "domain_strategy": "prefer_ipv4",
            "users": config.credentials.iter().map(|credential| json!({
                "name": credential.email,
                "uuid": credential.uuid,
                "password": credential.password,
            })).collect::<Vec<_>>(),
            "tls": tls,
            "congestion_control": "bbr",
            "auth_timeout": "3s",
            "zero_rtt_handshake": true,
            "heartbeat": "10s",
        })),
        ProtocolKind::Hysteria2 => Ok(json!({
            "type": "hysteria2",
            "tag": tag,
            "listen": profile.listen_host,
            "listen_port": profile.listen_port,
            "users": config.credentials.iter().map(|credential| json!({
                "name": credential.email,
                "password": credential.password,
            })).collect::<Vec<_>>(),
            "up_mbps": 1000,
            "down_mbps": 1000,
            "masquerade": format!("http://{}:80/", profile.server_name.clone().unwrap_or_else(|| profile.public_host.clone())),
            "tls": tls,
        })),
    }
}

fn render_xray_tls_settings(profile: &InboundProfile) -> serde_json::Value {
    if profile.security != SecurityKind::Tls {
        return serde_json::Value::Null;
    }
    json!({
        "serverName": profile.server_name,
        "alpn": profile.alpn,
        "minVersion": "1.2",
        "maxVersion": "1.3",
        "certificates": [{
            "certificateFile": tls_certificate_path(profile),
            "keyFile": tls_key_path(profile),
        }],
    })
}

fn render_xray_reality_settings(profile: &InboundProfile) -> serde_json::Value {
    if profile.security != SecurityKind::Reality {
        return serde_json::Value::Null;
    }
    let server_name = profile.server_name.clone().unwrap_or_default();
    json!({
        "show": false,
        "dest": format!("{server_name}:443"),
        "serverNames": [server_name],
        "privateKey": profile.reality_private_key,
        "shortIds": [profile.reality_short_id],
    })
}

fn render_xray_ws_settings(profile: &InboundProfile) -> serde_json::Value {
    if profile.transport != TransportKind::Ws {
        return serde_json::Value::Null;
    }
    json!({
        "path": profile.path,
        "headers": { "Host": profile.host_header }
    })
}

fn render_xray_grpc_settings(profile: &InboundProfile) -> serde_json::Value {
    if profile.transport != TransportKind::Grpc {
        return serde_json::Value::Null;
    }
    json!({
        "serviceName": profile.service_name,
    })
}

fn render_xray_http_upgrade_settings(profile: &InboundProfile) -> serde_json::Value {
    if profile.transport != TransportKind::HttpUpgrade {
        return serde_json::Value::Null;
    }
    json!({
        "path": profile.path,
        "host": profile.host_header,
    })
}

fn render_singbox_tls(profile: &InboundProfile) -> serde_json::Value {
    match profile.security {
        SecurityKind::None => serde_json::Value::Null,
        SecurityKind::Tls => json!({
            "enabled": true,
            "server_name": profile.server_name,
            "alpn": profile.alpn,
            "min_version": "1.2",
            "max_version": "1.3",
            "certificate_path": tls_certificate_path(profile),
            "key_path": tls_key_path(profile),
        }),
        SecurityKind::Reality => json!({
            "enabled": true,
            "server_name": profile.server_name,
            "reality": {
                "enabled": true,
                "private_key": profile.reality_private_key,
                "short_id": profile.reality_short_id,
            }
        }),
    }
}

fn render_singbox_transport(profile: &InboundProfile) -> serde_json::Value {
    match profile.transport {
        TransportKind::Tcp => serde_json::Value::Null,
        TransportKind::Ws => json!({
            "type": "ws",
            "path": profile.path,
            "headers": { "Host": profile.host_header }
        }),
        TransportKind::Grpc => json!({
            "type": "grpc",
            "service_name": profile.service_name,
        }),
        TransportKind::HttpUpgrade => json!({
            "type": "httpupgrade",
            "path": profile.path,
            "host": profile.host_header,
        }),
    }
}

fn tls_certificate_path(profile: &InboundProfile) -> &str {
    profile
        .tls_certificate_path
        .as_deref()
        .unwrap_or(DEFAULT_TLS_CERTIFICATE_PATH)
}

fn tls_key_path(profile: &InboundProfile) -> &str {
    profile
        .tls_key_path
        .as_deref()
        .unwrap_or(DEFAULT_TLS_KEY_PATH)
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

fn transport_to_wire(transport: TransportKind) -> &'static str {
    match transport {
        TransportKind::Tcp => "tcp",
        TransportKind::Ws => "ws",
        TransportKind::Grpc => "grpc",
        TransportKind::HttpUpgrade => "httpupgrade",
    }
}

fn security_name(security: SecurityKind) -> &'static str {
    match security {
        SecurityKind::None => "none",
        SecurityKind::Tls => "tls",
        SecurityKind::Reality => "reality",
    }
}

#[cfg(test)]
mod tests {
    use anneal_core::{ProtocolKind, ProxyEngine};

    use crate::{
        application::{ConfigRenderer, RendererStrategy, SingboxRenderer, XrayRenderer},
        domain::{CanonicalConfig, ClientCredential, InboundProfile, SecurityKind, TransportKind},
    };

    fn fixture(engine: ProxyEngine, protocol: ProtocolKind) -> CanonicalConfig {
        CanonicalConfig {
            engine,
            tag: "tenant-main".into(),
            server_name: Some("node.example.com".into()),
            credentials: vec![ClientCredential {
                email: "user@example.com".into(),
                uuid: "11111111-1111-1111-1111-111111111111".into(),
                password: Some("secret".into()),
            }],
            inbound_profiles: vec![InboundProfile {
                protocol,
                listen_host: "::".into(),
                listen_port: 443,
                public_host: "node.example.com".into(),
                public_port: 443,
                transport: TransportKind::Tcp,
                security: if protocol == ProtocolKind::VlessReality {
                    SecurityKind::Reality
                } else {
                    SecurityKind::Tls
                },
                server_name: Some("node.example.com".into()),
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
                tls_certificate_path: Some("/var/lib/anneal/tls/server.crt".into()),
                tls_key_path: Some("/var/lib/anneal/tls/server.key".into()),
            }],
        }
    }

    #[test]
    fn xray_renderer_rejects_tuic() {
        let error = XrayRenderer
            .render(&fixture(ProxyEngine::Xray, ProtocolKind::Tuic))
            .expect_err("must reject");
        assert_eq!(
            error.to_string(),
            "xray does not support tuic or hysteria2 in v1"
        );
    }

    #[test]
    fn singbox_renderer_supports_hysteria2() {
        let rendered = SingboxRenderer
            .render(&fixture(ProxyEngine::Singbox, ProtocolKind::Hysteria2))
            .expect("render");
        assert!(rendered.contains("\"type\": \"hysteria2\""));
    }

    #[test]
    fn dispatch_renderer_uses_engine_strategy() {
        let rendered = ConfigRenderer
            .render(&fixture(ProxyEngine::Xray, ProtocolKind::VlessReality))
            .expect("render");
        assert!(rendered.contains("\"protocol\": \"vless\""));
    }

    #[test]
    fn vless_tls_profile_is_supported() {
        let mut config = fixture(ProxyEngine::Xray, ProtocolKind::VlessReality);
        config.inbound_profiles[0].security = SecurityKind::Tls;
        config.inbound_profiles[0].transport = TransportKind::Ws;
        config.inbound_profiles[0].path = Some("/vless-ws".into());
        config.inbound_profiles[0].flow = None;
        config.inbound_profiles[0].reality_public_key = None;
        config.inbound_profiles[0].reality_private_key = None;
        config.inbound_profiles[0].reality_short_id = None;
        let rendered = ConfigRenderer.render(&config).expect("render");
        assert!(rendered.contains("\"protocol\": \"vless\""));
        assert!(rendered.contains("\"security\": \"tls\""));
    }

    #[test]
    fn reality_requires_keys() {
        let mut config = fixture(ProxyEngine::Xray, ProtocolKind::VlessReality);
        config.inbound_profiles[0].reality_public_key = None;
        let error = ConfigRenderer.render(&config).expect_err("must fail");
        assert_eq!(
            error.to_string(),
            "reality profile requires server_name, reality keys and short_id"
        );
    }

    #[test]
    fn trojan_requires_tls() {
        let mut config = fixture(ProxyEngine::Singbox, ProtocolKind::Trojan);
        config.inbound_profiles[0].security = SecurityKind::None;
        let error = ConfigRenderer.render(&config).expect_err("must fail");
        assert_eq!(
            error.to_string(),
            "trojan, tuic and hysteria2 profiles require tls security"
        );
    }

    #[test]
    fn singbox_tls_includes_certificate_paths() {
        let rendered = SingboxRenderer
            .render(&fixture(ProxyEngine::Singbox, ProtocolKind::Tuic))
            .expect("render");
        assert!(rendered.contains("\"certificate_path\": \"/var/lib/anneal/tls/server.crt\""));
        assert!(rendered.contains("\"key_path\": \"/var/lib/anneal/tls/server.key\""));
    }
}
