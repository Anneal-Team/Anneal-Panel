use anneal_audit::{AuditService, PgAuditRepository};
use anneal_auth::{
    ArgonPasswordService, AuthService, JwtService, OtpAuthTotpService, PgSessionRepository,
};
use anneal_core::TokenHasher;
use anneal_notifications::{NotificationService, PgNotificationRepository, TelegramNotifier};
use anneal_platform::Settings;
use anneal_rbac::RbacService;
use anneal_subscriptions::{
    DeliveryEndpoint, PgSubscriptionRepository, StaticDeliveryEndpointCatalog, SubscriptionService,
    UnifiedSubscriptionService,
};
use anneal_usage::{PgUsageRepository, UsageService};
use anneal_users::{PgUserRepository, UserService};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub pool: PgPool,
    pub rbac: RbacService,
    pub users: PgUserRepository,
    pub audit: PgAuditRepository,
    pub sessions: PgSessionRepository,
    pub subscriptions: PgSubscriptionRepository,
    pub usage: PgUsageRepository,
    pub notifications: PgNotificationRepository,
    pub password_service: ArgonPasswordService,
    pub jwt_service: JwtService,
    pub totp_service: OtpAuthTotpService,
    pub token_hasher: TokenHasher,
}

pub type RuntimeAuthService<'a> = AuthService<
    &'a PgUserRepository,
    &'a PgSessionRepository,
    &'a ArgonPasswordService,
    &'a OtpAuthTotpService,
    &'a JwtService,
>;

impl AppState {
    pub fn auth_service(&self) -> RuntimeAuthService<'_> {
        AuthService::new(
            &self.users,
            &self.sessions,
            &self.password_service,
            &self.totp_service,
            &self.jwt_service,
        )
    }

    pub fn user_service(&self) -> UserService<&PgUserRepository> {
        UserService::new(&self.users, self.rbac)
    }

    pub fn audit_service(&self) -> AuditService<&PgAuditRepository> {
        AuditService::new(&self.audit)
    }

    pub fn subscription_service(&self) -> SubscriptionService<&PgSubscriptionRepository> {
        SubscriptionService::with_token_hasher(
            &self.subscriptions,
            self.rbac,
            self.token_hasher.clone(),
        )
    }

    pub fn unified_subscription_service(
        &self,
    ) -> UnifiedSubscriptionService<&PgSubscriptionRepository, StaticDeliveryEndpointCatalog> {
        UnifiedSubscriptionService::new(&self.subscriptions, self.mihomo_catalog())
    }

    pub fn usage_service(&self) -> UsageService<&PgUsageRepository> {
        UsageService::new(&self.usage)
    }

    pub fn notification_service(
        &self,
    ) -> NotificationService<&PgNotificationRepository, TelegramNotifier> {
        NotificationService::new(
            &self.notifications,
            TelegramNotifier::new(
                self.settings.telegram_bot_token.clone(),
                self.settings.telegram_chat_id.clone(),
            ),
        )
    }

    fn mihomo_catalog(&self) -> StaticDeliveryEndpointCatalog {
        StaticDeliveryEndpointCatalog::new(mihomo_endpoints_from_settings(&self.settings))
    }
}

fn mihomo_endpoints_from_settings(settings: &Settings) -> Vec<DeliveryEndpoint> {
    settings
        .mihomo_protocols
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter_map(|value| parse_protocol(value).ok())
        .map(|protocol| {
            let server_name = settings
                .mihomo_server_name
                .clone()
                .unwrap_or_else(|| settings.mihomo_public_host.clone());
            DeliveryEndpoint {
                name: "mihomo".into(),
                protocol,
                listen_host: "::".into(),
                listen_port: settings.mihomo_public_port,
                public_host: settings.mihomo_public_host.clone(),
                public_port: settings.mihomo_public_port,
                transport: anneal_config_engine::TransportKind::Tcp,
                security: if protocol == anneal_core::ProtocolKind::Shadowsocks2022 {
                    anneal_config_engine::SecurityKind::None
                } else if protocol == anneal_core::ProtocolKind::VlessReality
                    && settings.mihomo_reality_public_key.is_some()
                    && settings.mihomo_reality_short_id.is_some()
                {
                    anneal_config_engine::SecurityKind::Reality
                } else {
                    anneal_config_engine::SecurityKind::Tls
                },
                server_name: Some(server_name),
                host_header: None,
                path: None,
                service_name: None,
                flow: (protocol == anneal_core::ProtocolKind::VlessReality)
                    .then_some("xtls-rprx-vision".into()),
                reality_public_key: settings.mihomo_reality_public_key.clone(),
                reality_private_key: None,
                reality_short_id: settings.mihomo_reality_short_id.clone(),
                fingerprint: Some("chrome".into()),
                alpn: vec!["h2".into(), "http/1.1".into()],
                cipher: (protocol == anneal_core::ProtocolKind::Shadowsocks2022)
                    .then_some(settings.mihomo_cipher.clone()),
                tls_certificate_path: None,
                tls_key_path: None,
            }
        })
        .collect()
}

fn parse_protocol(value: &str) -> anneal_core::ApplicationResult<anneal_core::ProtocolKind> {
    match value {
        "vless_reality" | "vless" => Ok(anneal_core::ProtocolKind::VlessReality),
        "vmess" => Ok(anneal_core::ProtocolKind::Vmess),
        "trojan" => Ok(anneal_core::ProtocolKind::Trojan),
        "shadowsocks_2022" | "ss2022" | "ss" => Ok(anneal_core::ProtocolKind::Shadowsocks2022),
        "tuic" => Ok(anneal_core::ProtocolKind::Tuic),
        "hysteria2" | "hy2" => Ok(anneal_core::ProtocolKind::Hysteria2),
        other => Err(anneal_core::ApplicationError::Validation(format!(
            "unsupported mihomo protocol: {other}"
        ))),
    }
}
