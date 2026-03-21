use base64::{Engine as _, engine::general_purpose};
use serde_json::json;
use urlencoding::encode;

use anneal_core::{ApplicationError, ApplicationResult, ProtocolKind};

use crate::domain::{ClientCredential, InboundProfile, SecurityKind, TransportKind};

pub trait ShareLinkStrategy: Send + Sync {
    fn render(
        &self,
        profile: &InboundProfile,
        credential: &ClientCredential,
        label: &str,
    ) -> ApplicationResult<String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ShareLinkRenderer;

impl ShareLinkStrategy for ShareLinkRenderer {
    fn render(
        &self,
        profile: &InboundProfile,
        credential: &ClientCredential,
        label: &str,
    ) -> ApplicationResult<String> {
        validate_share_profile(profile, credential)?;
        match profile.protocol {
            ProtocolKind::VlessReality => Ok(render_vless(profile, credential, label)),
            ProtocolKind::Vmess => Ok(render_vmess(profile, credential, label)),
            ProtocolKind::Trojan => Ok(render_trojan(profile, credential, label)),
            ProtocolKind::Shadowsocks2022 => Ok(render_shadowsocks(profile, credential, label)),
            ProtocolKind::Tuic => Ok(render_tuic(profile, credential, label)),
            ProtocolKind::Hysteria2 => Ok(render_hysteria2(profile, credential, label)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionDocumentFormat {
    Raw,
    Base64,
    ClashMeta,
    SingBox,
    HiddifyJson,
}

#[derive(Debug, Clone)]
pub struct RenderedShareLink {
    pub label: String,
    pub uri: String,
    pub profile: InboundProfile,
    pub credential: ClientCredential,
}

#[derive(Debug, Clone)]
pub struct RenderedSubscriptionDocument {
    pub content: String,
    pub content_type: &'static str,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SubscriptionDocumentRenderer;

impl SubscriptionDocumentRenderer {
    pub fn render(
        &self,
        links: &[RenderedShareLink],
        format: SubscriptionDocumentFormat,
    ) -> ApplicationResult<RenderedSubscriptionDocument> {
        let raw = render_raw_links(links);
        match format {
            SubscriptionDocumentFormat::Raw => Ok(RenderedSubscriptionDocument {
                content: raw,
                content_type: "text/plain; charset=utf-8",
            }),
            SubscriptionDocumentFormat::Base64 => Ok(RenderedSubscriptionDocument {
                content: general_purpose::STANDARD.encode(raw.as_bytes()),
                content_type: "text/plain; charset=utf-8",
            }),
            SubscriptionDocumentFormat::ClashMeta => Ok(RenderedSubscriptionDocument {
                content: render_clash_meta(links),
                content_type: "application/yaml; charset=utf-8",
            }),
            SubscriptionDocumentFormat::SingBox => Ok(RenderedSubscriptionDocument {
                content: serde_json::to_string_pretty(&render_sing_box_document(links))
                    .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?,
                content_type: "application/json; charset=utf-8",
            }),
            SubscriptionDocumentFormat::HiddifyJson => Ok(RenderedSubscriptionDocument {
                content: serde_json::to_string_pretty(&render_hiddify_json(links))
                    .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?,
                content_type: "application/json; charset=utf-8",
            }),
        }
    }
}

fn render_raw_links(links: &[RenderedShareLink]) -> String {
    links
        .iter()
        .map(|entry| entry.uri.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_hiddify_json(links: &[RenderedShareLink]) -> serde_json::Value {
    json!({
        "version": 1,
        "profile_count": links.len(),
        "profiles": links.iter().map(|entry| json!({
            "name": entry.label,
            "protocol": protocol_name(entry.profile.protocol),
            "uri": entry.uri,
            "server": entry.profile.public_host,
            "port": entry.profile.public_port,
            "transport": transport_name(entry.profile.transport),
            "security": security_name(entry.profile.security),
            "sni": entry.profile.server_name,
            "alpn": entry.profile.alpn,
            "host": entry.profile.host_header,
            "path": entry.profile.path,
            "service_name": entry.profile.service_name,
            "flow": entry.profile.flow,
            "fingerprint": entry.profile.fingerprint,
            "cipher": entry.profile.cipher,
        })).collect::<Vec<_>>()
    })
}

fn render_sing_box_document(links: &[RenderedShareLink]) -> serde_json::Value {
    let tags = links
        .iter()
        .map(|entry| entry.label.clone())
        .collect::<Vec<_>>();
    json!({
        "log": {
            "level": "warn"
        },
        "outbounds": links.iter().map(render_sing_box_outbound).chain([
            json!({
                "type": "selector",
                "tag": "anneal",
                "outbounds": tags,
                "default": "urltest"
            }),
            json!({
                "type": "urltest",
                "tag": "urltest",
                "outbounds": links.iter().map(|entry| entry.label.clone()).collect::<Vec<_>>(),
                "url": "https://www.gstatic.com/generate_204",
                "interval": "10m"
            }),
            json!({
                "type": "direct",
                "tag": "direct"
            })
        ]).collect::<Vec<_>>(),
        "route": {
            "auto_detect_interface": true,
            "final": "anneal"
        }
    })
}

fn render_sing_box_outbound(entry: &RenderedShareLink) -> serde_json::Value {
    let tls = match entry.profile.security {
        SecurityKind::None => serde_json::Value::Null,
        SecurityKind::Tls => json!({
            "enabled": true,
            "server_name": entry.profile.server_name,
            "alpn": nullable_vec(&entry.profile.alpn),
            "utls": nullable_json(entry.profile.fingerprint.as_ref().map(|fingerprint| json!({
                "enabled": true,
                "fingerprint": fingerprint
            }))),
        }),
        SecurityKind::Reality => json!({
            "enabled": true,
            "server_name": entry.profile.server_name,
            "alpn": nullable_vec(&entry.profile.alpn),
            "utls": nullable_json(entry.profile.fingerprint.as_ref().map(|fingerprint| json!({
                "enabled": true,
                "fingerprint": fingerprint
            }))),
            "reality": {
                "enabled": true,
                "public_key": entry.profile.reality_public_key,
                "short_id": entry.profile.reality_short_id
            }
        }),
    };

    let transport = match entry.profile.transport {
        TransportKind::Tcp => serde_json::Value::Null,
        TransportKind::Ws => json!({
            "type": "ws",
            "path": entry.profile.path,
            "headers": nullable_json(entry.profile.host_header.as_ref().map(|host| json!({"Host": host}))),
        }),
        TransportKind::Grpc => json!({
            "type": "grpc",
            "service_name": entry.profile.service_name,
        }),
        TransportKind::HttpUpgrade => json!({
            "type": "httpupgrade",
            "path": entry.profile.path,
            "host": entry.profile.host_header,
        }),
    };

    match entry.profile.protocol {
        ProtocolKind::VlessReality => json!({
            "type": "vless",
            "tag": entry.label,
            "server": entry.profile.public_host,
            "server_port": entry.profile.public_port,
            "uuid": entry.credential.uuid,
            "flow": entry.profile.flow,
            "tls": tls,
            "transport": transport,
        }),
        ProtocolKind::Vmess => json!({
            "type": "vmess",
            "tag": entry.label,
            "server": entry.profile.public_host,
            "server_port": entry.profile.public_port,
            "uuid": entry.credential.uuid,
            "security": "auto",
            "alter_id": 0,
            "tls": tls,
            "transport": transport,
        }),
        ProtocolKind::Trojan => json!({
            "type": "trojan",
            "tag": entry.label,
            "server": entry.profile.public_host,
            "server_port": entry.profile.public_port,
            "password": entry.credential.password,
            "tls": tls,
            "transport": transport,
        }),
        ProtocolKind::Shadowsocks2022 => json!({
            "type": "shadowsocks",
            "tag": entry.label,
            "server": entry.profile.public_host,
            "server_port": entry.profile.public_port,
            "method": entry.profile.cipher,
            "password": entry.credential.password,
        }),
        ProtocolKind::Tuic => json!({
            "type": "tuic",
            "tag": entry.label,
            "server": entry.profile.public_host,
            "server_port": entry.profile.public_port,
            "uuid": entry.credential.uuid,
            "password": entry.credential.password,
            "congestion_control": "bbr",
            "tls": tls,
        }),
        ProtocolKind::Hysteria2 => json!({
            "type": "hysteria2",
            "tag": entry.label,
            "server": entry.profile.public_host,
            "server_port": entry.profile.public_port,
            "password": entry.credential.password,
            "tls": tls,
        }),
    }
}

fn render_clash_meta(links: &[RenderedShareLink]) -> String {
    let proxies = links
        .iter()
        .map(render_clash_proxy)
        .collect::<Vec<_>>()
        .join("\n");
    let proxy_names = links
        .iter()
        .map(|entry| format!("      - {}", yaml_quote(&entry.label)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "mixed-port: 7890\nallow-lan: false\nmode: rule\nproxies:\n{proxies}\nproxy-groups:\n  - name: \"Anneal\"\n    type: select\n    proxies:\n{proxy_names}\nrules:\n  - MATCH,Anneal\n"
    )
}

fn render_clash_proxy(entry: &RenderedShareLink) -> String {
    let mut lines = vec![
        format!("  - name: {}", yaml_quote(&entry.label)),
        format!("    type: {}", clash_protocol_name(entry.profile.protocol)),
        format!("    server: {}", yaml_quote(&entry.profile.public_host)),
        format!("    port: {}", entry.profile.public_port),
    ];

    match entry.profile.protocol {
        ProtocolKind::VlessReality => {
            lines.push(format!("    uuid: {}", yaml_quote(&entry.credential.uuid)));
            lines.push(String::from("    udp: true"));
            lines.push(format!(
                "    network: {}",
                clash_transport_name(entry.profile.transport)
            ));
            lines.push(String::from("    tls: true"));
            append_server_name(&mut lines, &entry.profile);
            if let Some(flow) = &entry.profile.flow {
                lines.push(format!("    flow: {}", yaml_quote(flow)));
            }
            if let Some(fingerprint) = &entry.profile.fingerprint {
                lines.push(format!(
                    "    client-fingerprint: {}",
                    yaml_quote(fingerprint)
                ));
            }
            lines.push(String::from("    reality-opts:"));
            if let Some(public_key) = &entry.profile.reality_public_key {
                lines.push(format!("      public-key: {}", yaml_quote(public_key)));
            }
            if let Some(short_id) = &entry.profile.reality_short_id {
                lines.push(format!("      short-id: {}", yaml_quote(short_id)));
            }
            append_clash_transport(&mut lines, &entry.profile);
        }
        ProtocolKind::Vmess => {
            lines.push(format!("    uuid: {}", yaml_quote(&entry.credential.uuid)));
            lines.push(String::from("    alterId: 0"));
            lines.push(String::from("    cipher: auto"));
            lines.push(String::from("    udp: true"));
            lines.push(format!(
                "    network: {}",
                clash_transport_name(entry.profile.transport)
            ));
            lines.push(format!(
                "    tls: {}",
                if entry.profile.security == SecurityKind::None {
                    "false"
                } else {
                    "true"
                }
            ));
            append_server_name(&mut lines, &entry.profile);
            append_clash_transport(&mut lines, &entry.profile);
        }
        ProtocolKind::Trojan => {
            lines.push(format!(
                "    password: {}",
                yaml_quote(entry.credential.password.as_deref().unwrap_or_default())
            ));
            lines.push(String::from("    udp: true"));
            lines.push(format!(
                "    network: {}",
                clash_transport_name(entry.profile.transport)
            ));
            lines.push(format!(
                "    tls: {}",
                if entry.profile.security == SecurityKind::None {
                    "false"
                } else {
                    "true"
                }
            ));
            append_server_name(&mut lines, &entry.profile);
            append_clash_transport(&mut lines, &entry.profile);
        }
        ProtocolKind::Shadowsocks2022 => {
            lines.push(format!(
                "    cipher: {}",
                yaml_quote(entry.profile.cipher.as_deref().unwrap_or_default())
            ));
            lines.push(format!(
                "    password: {}",
                yaml_quote(entry.credential.password.as_deref().unwrap_or_default())
            ));
            lines.push(String::from("    udp: true"));
        }
        ProtocolKind::Tuic => {
            lines.push(format!("    uuid: {}", yaml_quote(&entry.credential.uuid)));
            lines.push(format!(
                "    password: {}",
                yaml_quote(entry.credential.password.as_deref().unwrap_or_default())
            ));
            lines.push(String::from("    udp: true"));
            append_server_name(&mut lines, &entry.profile);
            append_alpn(&mut lines, &entry.profile);
            if let Some(fingerprint) = &entry.profile.fingerprint {
                lines.push(format!(
                    "    client-fingerprint: {}",
                    yaml_quote(fingerprint)
                ));
            }
        }
        ProtocolKind::Hysteria2 => {
            lines.push(format!(
                "    password: {}",
                yaml_quote(entry.credential.password.as_deref().unwrap_or_default())
            ));
            lines.push(String::from("    udp: true"));
            append_server_name(&mut lines, &entry.profile);
            append_alpn(&mut lines, &entry.profile);
        }
    }

    lines.join("\n")
}

fn append_server_name(lines: &mut Vec<String>, profile: &InboundProfile) {
    if let Some(server_name) = &profile.server_name {
        lines.push(format!("    servername: {}", yaml_quote(server_name)));
        lines.push(format!("    sni: {}", yaml_quote(server_name)));
    }
}

fn append_alpn(lines: &mut Vec<String>, profile: &InboundProfile) {
    if profile.alpn.is_empty() {
        return;
    }
    lines.push(String::from("    alpn:"));
    lines.extend(
        profile
            .alpn
            .iter()
            .map(|value| format!("      - {}", yaml_quote(value))),
    );
}

fn append_clash_transport(lines: &mut Vec<String>, profile: &InboundProfile) {
    match profile.transport {
        TransportKind::Ws | TransportKind::HttpUpgrade => {
            lines.push(String::from("    ws-opts:"));
            if let Some(path) = &profile.path {
                lines.push(format!("      path: {}", yaml_quote(path)));
            }
            if let Some(host_header) = &profile.host_header {
                lines.push(String::from("      headers:"));
                lines.push(format!("        Host: {}", yaml_quote(host_header)));
            }
        }
        TransportKind::Grpc => {
            lines.push(String::from("    grpc-opts:"));
            if let Some(service_name) = &profile.service_name {
                lines.push(format!(
                    "      grpc-service-name: {}",
                    yaml_quote(service_name)
                ));
            }
        }
        TransportKind::Tcp => {}
    }
}

fn yaml_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn nullable_vec(values: &[String]) -> serde_json::Value {
    if values.is_empty() {
        serde_json::Value::Null
    } else {
        json!(values)
    }
}

fn nullable_json(value: Option<serde_json::Value>) -> serde_json::Value {
    value.unwrap_or(serde_json::Value::Null)
}

fn protocol_name(protocol: ProtocolKind) -> &'static str {
    match protocol {
        ProtocolKind::VlessReality => "vless_reality",
        ProtocolKind::Vmess => "vmess",
        ProtocolKind::Trojan => "trojan",
        ProtocolKind::Shadowsocks2022 => "shadowsocks_2022",
        ProtocolKind::Tuic => "tuic",
        ProtocolKind::Hysteria2 => "hysteria2",
    }
}

fn security_name(security: SecurityKind) -> &'static str {
    match security {
        SecurityKind::None => "none",
        SecurityKind::Tls => "tls",
        SecurityKind::Reality => "reality",
    }
}

fn clash_protocol_name(protocol: ProtocolKind) -> &'static str {
    match protocol {
        ProtocolKind::VlessReality => "vless",
        ProtocolKind::Vmess => "vmess",
        ProtocolKind::Trojan => "trojan",
        ProtocolKind::Shadowsocks2022 => "ss",
        ProtocolKind::Tuic => "tuic",
        ProtocolKind::Hysteria2 => "hysteria2",
    }
}

fn clash_transport_name(transport: TransportKind) -> &'static str {
    match transport {
        TransportKind::Tcp => "tcp",
        TransportKind::Ws => "ws",
        TransportKind::Grpc => "grpc",
        TransportKind::HttpUpgrade => "ws",
    }
}

fn validate_share_profile(
    profile: &InboundProfile,
    credential: &ClientCredential,
) -> ApplicationResult<()> {
    if profile.public_host.trim().is_empty() {
        return Err(ApplicationError::Validation(
            "public_host is required for subscription rendering".into(),
        ));
    }
    if matches!(
        profile.protocol,
        ProtocolKind::Trojan
            | ProtocolKind::Shadowsocks2022
            | ProtocolKind::Tuic
            | ProtocolKind::Hysteria2
    ) && credential
        .password
        .as_deref()
        .unwrap_or_default()
        .is_empty()
    {
        return Err(ApplicationError::Validation(
            "password is required for password-based protocols".into(),
        ));
    }
    if profile.security == SecurityKind::Reality
        && (profile.reality_public_key.is_none()
            || profile.reality_short_id.is_none()
            || profile.server_name.is_none())
    {
        return Err(ApplicationError::Validation(
            "reality share link requires public key, short id and server_name".into(),
        ));
    }
    if profile.protocol == ProtocolKind::Shadowsocks2022 && profile.cipher.is_none() {
        return Err(ApplicationError::Validation(
            "shadowsocks_2022 share link requires cipher".into(),
        ));
    }
    Ok(())
}

fn render_vless(profile: &InboundProfile, credential: &ClientCredential, label: &str) -> String {
    let mut params = vec![format!("type={}", transport_name(profile.transport))];
    append_common_transport_params(profile, &mut params);
    match profile.security {
        SecurityKind::None => params.push("security=none".into()),
        SecurityKind::Tls => {
            params.push("security=tls".into());
            append_tls_params(profile, &mut params);
        }
        SecurityKind::Reality => {
            params.push("security=reality".into());
            append_reality_params(profile, &mut params);
        }
    }
    if let Some(flow) = &profile.flow {
        params.push(format!("flow={}", encode(flow)));
    }
    format!(
        "vless://{}@{}:{}?{}#{}",
        credential.uuid,
        profile.public_host,
        profile.public_port,
        params.join("&"),
        encode(label)
    )
}

fn render_vmess(profile: &InboundProfile, credential: &ClientCredential, label: &str) -> String {
    let vmess = json!({
        "v": "2",
        "ps": label,
        "add": profile.public_host,
        "port": profile.public_port.to_string(),
        "id": credential.uuid,
        "aid": "0",
        "scy": "auto",
        "net": transport_name(profile.transport),
        "type": "none",
        "host": profile.host_header,
        "path": profile.path,
        "tls": if profile.security == SecurityKind::None { "" } else { "tls" },
        "sni": profile.server_name,
        "alpn": if profile.alpn.is_empty() {
            None
        } else {
            Some(profile.alpn.join(","))
        },
    });
    format!(
        "vmess://{}",
        general_purpose::STANDARD.encode(vmess.to_string().as_bytes())
    )
}

fn render_trojan(profile: &InboundProfile, credential: &ClientCredential, label: &str) -> String {
    let mut params = vec![format!("type={}", transport_name(profile.transport))];
    append_common_transport_params(profile, &mut params);
    if profile.security != SecurityKind::None {
        params.push("security=tls".into());
        append_tls_params(profile, &mut params);
    } else {
        params.push("security=none".into());
    }
    format!(
        "trojan://{}@{}:{}?{}#{}",
        credential.password.as_deref().unwrap_or_default(),
        profile.public_host,
        profile.public_port,
        params.join("&"),
        encode(label)
    )
}

fn render_shadowsocks(
    profile: &InboundProfile,
    credential: &ClientCredential,
    label: &str,
) -> String {
    let secret = format!(
        "{}:{}",
        profile.cipher.as_deref().unwrap_or_default(),
        credential.password.as_deref().unwrap_or_default()
    );
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(secret.as_bytes());
    format!(
        "ss://{}@{}:{}#{}",
        encoded,
        profile.public_host,
        profile.public_port,
        encode(label)
    )
}

fn render_tuic(profile: &InboundProfile, credential: &ClientCredential, label: &str) -> String {
    let mut params = vec!["congestion_control=bbr".into()];
    append_tls_params(profile, &mut params);
    if !profile.alpn.is_empty() {
        params.push(format!("alpn={}", encode(&profile.alpn.join(","))));
    }
    format!(
        "tuic://{}:{}@{}:{}?{}#{}",
        credential.uuid,
        credential.password.as_deref().unwrap_or_default(),
        profile.public_host,
        profile.public_port,
        params.join("&"),
        encode(label)
    )
}

fn render_hysteria2(
    profile: &InboundProfile,
    credential: &ClientCredential,
    label: &str,
) -> String {
    let mut params = Vec::new();
    append_tls_params(profile, &mut params);
    if !profile.alpn.is_empty() {
        params.push(format!("alpn={}", encode(&profile.alpn.join(","))));
    }
    format!(
        "hysteria2://{}@{}:{}?{}#{}",
        credential.password.as_deref().unwrap_or_default(),
        profile.public_host,
        profile.public_port,
        params.join("&"),
        encode(label)
    )
}

fn append_common_transport_params(profile: &InboundProfile, params: &mut Vec<String>) {
    match profile.transport {
        TransportKind::Ws | TransportKind::HttpUpgrade => {
            if let Some(path) = &profile.path {
                params.push(format!("path={}", encode(path)));
            }
            if let Some(host_header) = &profile.host_header {
                params.push(format!("host={}", encode(host_header)));
            }
        }
        TransportKind::Grpc => {
            if let Some(service_name) = &profile.service_name {
                params.push(format!("serviceName={}", encode(service_name)));
            }
        }
        TransportKind::Tcp => {}
    }
}

fn append_tls_params(profile: &InboundProfile, params: &mut Vec<String>) {
    if let Some(server_name) = &profile.server_name {
        params.push(format!("sni={}", encode(server_name)));
    }
    if let Some(fingerprint) = &profile.fingerprint {
        params.push(format!("fp={}", encode(fingerprint)));
    }
    if !profile.alpn.is_empty() {
        params.push(format!("alpn={}", encode(&profile.alpn.join(","))));
    }
}

fn append_reality_params(profile: &InboundProfile, params: &mut Vec<String>) {
    append_tls_params(profile, params);
    if let Some(public_key) = &profile.reality_public_key {
        params.push(format!("pbk={}", encode(public_key)));
    }
    if let Some(short_id) = &profile.reality_short_id {
        params.push(format!("sid={}", encode(short_id)));
    }
    params.push("spx=%2F".into());
}

fn transport_name(transport: TransportKind) -> &'static str {
    match transport {
        TransportKind::Tcp => "tcp",
        TransportKind::Ws => "ws",
        TransportKind::Grpc => "grpc",
        TransportKind::HttpUpgrade => "httpupgrade",
    }
}

#[cfg(test)]
mod tests {
    use anneal_core::ProtocolKind;

    use crate::{
        domain::{ClientCredential, InboundProfile, SecurityKind, TransportKind},
        subscription::{
            RenderedShareLink, ShareLinkRenderer, ShareLinkStrategy, SubscriptionDocumentFormat,
            SubscriptionDocumentRenderer,
        },
    };

    fn credential() -> ClientCredential {
        ClientCredential {
            email: "user@example.com".into(),
            uuid: "11111111-1111-1111-1111-111111111111".into(),
            password: Some("secret-pass".into()),
        }
    }

    fn profile(protocol: ProtocolKind) -> InboundProfile {
        InboundProfile {
            protocol,
            listen_host: "::".into(),
            listen_port: 443,
            public_host: "edge.example.com".into(),
            public_port: 443,
            transport: TransportKind::Ws,
            security: if protocol == ProtocolKind::VlessReality {
                SecurityKind::Reality
            } else {
                SecurityKind::Tls
            },
            server_name: Some("edge.example.com".into()),
            host_header: Some("cdn.example.com".into()),
            path: Some("/ws".into()),
            service_name: Some("grpc".into()),
            flow: Some("xtls-rprx-vision".into()),
            reality_public_key: Some("public-key".into()),
            reality_private_key: Some("private-key".into()),
            reality_short_id: Some("deadbeef".into()),
            fingerprint: Some("chrome".into()),
            alpn: vec!["h2".into(), "http/1.1".into()],
            cipher: Some("2022-blake3-aes-128-gcm".into()),
            tls_certificate_path: Some("/var/lib/anneal/tls/server.crt".into()),
            tls_key_path: Some("/var/lib/anneal/tls/server.key".into()),
        }
    }

    #[test]
    fn vless_share_link_contains_reality_fields() {
        let rendered = ShareLinkRenderer
            .render(
                &profile(ProtocolKind::VlessReality),
                &credential(),
                "edge-vless",
            )
            .expect("render");
        assert!(rendered.contains("security=reality"));
        assert!(rendered.contains("pbk=public-key"));
        assert!(rendered.contains("sid=deadbeef"));
    }

    #[test]
    fn tuic_share_link_contains_password_pair() {
        let rendered = ShareLinkRenderer
            .render(&profile(ProtocolKind::Tuic), &credential(), "edge-tuic")
            .expect("render");
        assert!(rendered.starts_with("tuic://11111111-1111-1111-1111-111111111111:secret-pass@"));
    }

    #[test]
    fn subscription_document_encodes_base64_bundle() {
        let entries = vec![
            RenderedShareLink {
                label: "one".into(),
                uri: "vless://one".into(),
                profile: profile(ProtocolKind::VlessReality),
                credential: credential(),
            },
            RenderedShareLink {
                label: "two".into(),
                uri: "vmess://two".into(),
                profile: profile(ProtocolKind::Vmess),
                credential: credential(),
            },
        ];
        let raw = SubscriptionDocumentRenderer
            .render(&entries, SubscriptionDocumentFormat::Raw)
            .expect("raw");
        let encoded = SubscriptionDocumentRenderer
            .render(&entries, SubscriptionDocumentFormat::Base64)
            .expect("base64");
        assert_eq!(raw.content, "vless://one\nvmess://two");
        assert!(encoded.content.len() > raw.content.len());
    }

    #[test]
    fn subscription_document_renders_hiddify_json_bundle() {
        let entries = vec![RenderedShareLink {
            label: "edge-vless".into(),
            uri: "vless://one".into(),
            profile: profile(ProtocolKind::VlessReality),
            credential: credential(),
        }];
        let rendered = SubscriptionDocumentRenderer
            .render(&entries, SubscriptionDocumentFormat::HiddifyJson)
            .expect("json");
        assert_eq!(rendered.content_type, "application/json; charset=utf-8");
        assert!(rendered.content.contains("\"profiles\""));
        assert!(rendered.content.contains("\"protocol\": \"vless_reality\""));
    }

    #[test]
    fn subscription_document_renders_clash_meta_bundle() {
        let entries = vec![RenderedShareLink {
            label: "edge-vmess".into(),
            uri: "vmess://two".into(),
            profile: profile(ProtocolKind::Vmess),
            credential: credential(),
        }];
        let rendered = SubscriptionDocumentRenderer
            .render(&entries, SubscriptionDocumentFormat::ClashMeta)
            .expect("clash");
        assert_eq!(rendered.content_type, "application/yaml; charset=utf-8");
        assert!(rendered.content.contains("proxies:"));
        assert!(rendered.content.contains("type: vmess"));
    }
}
