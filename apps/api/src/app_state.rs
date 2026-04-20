use anneal_audit::{AuditService, PgAuditRepository};
use anneal_auth::{
    ArgonPasswordService, AuthService, JwtService, OtpAuthTotpService, PgSessionRepository,
};
use anneal_core::TokenHasher;
use anneal_nodes::{NodeService, PgNodeRepository};
use anneal_notifications::{NotificationService, PgNotificationRepository, TelegramNotifier};
use anneal_platform::{DeploymentJob, NotificationJob, Settings};
use anneal_rbac::RbacService;
use anneal_subscriptions::{
    PgSubscriptionRepository, SubscriptionService, UnifiedSubscriptionService,
};
use anneal_usage::{PgUsageRepository, UsageService};
use anneal_users::{PgUserRepository, UserService};
use apalis_postgres::PostgresStorage;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub pool: PgPool,
    pub rbac: RbacService,
    pub users: PgUserRepository,
    pub audit: PgAuditRepository,
    pub sessions: PgSessionRepository,
    pub nodes: PgNodeRepository,
    pub subscriptions: PgSubscriptionRepository,
    pub usage: PgUsageRepository,
    pub notifications: PgNotificationRepository,
    pub password_service: ArgonPasswordService,
    pub jwt_service: JwtService,
    pub totp_service: OtpAuthTotpService,
    pub token_hasher: TokenHasher,
    pub deployment_queue: PostgresStorage<DeploymentJob>,
    pub notification_queue: PostgresStorage<NotificationJob>,
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

    pub fn node_service(&self) -> NodeService<&PgNodeRepository> {
        NodeService::with_public_base_url(
            &self.nodes,
            self.rbac,
            self.token_hasher.clone(),
            &self.settings.public_base_url,
        )
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
    ) -> UnifiedSubscriptionService<&PgSubscriptionRepository, &PgNodeRepository> {
        UnifiedSubscriptionService::with_token_hasher(
            &self.subscriptions,
            &self.nodes,
            self.token_hasher.clone(),
        )
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
}
