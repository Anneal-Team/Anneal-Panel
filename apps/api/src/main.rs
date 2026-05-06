mod app_state;
mod error;
mod extractors;
mod openapi;
mod transport;

use anneal_audit::PgAuditRepository;
use anneal_auth::{ArgonPasswordService, JwtService, OtpAuthTotpService, PgSessionRepository};
use anneal_notifications::PgNotificationRepository;
use anneal_platform::{Settings, connect_pool, init_telemetry, run_migrations};
use anneal_rbac::RbacService;
use anneal_subscriptions::PgSubscriptionRepository;
use anneal_usage::PgUsageRepository;
use anneal_users::PgUserRepository;
use axum::{
    Router,
    routing::{get, patch, post},
};
use openapi::ApiDoc;
use transport::{audit, auth, notifications, subscriptions, usage, users};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::app_state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;
    let secret_box = anneal_core::SecretBox::new(&settings.data_encryption_key)?;
    let token_hasher = anneal_core::TokenHasher::new(&settings.token_hash_key)?;
    init_telemetry("anneal-api", &settings)?;
    let pool = connect_pool(&settings.database_url).await?;
    run_migrations(&pool, &settings.migrations_dir).await?;
    anneal_platform::backfill_protected_data(&pool, &secret_box, &token_hasher).await?;
    let state = AppState {
        settings: settings.clone(),
        pool: pool.clone(),
        rbac: RbacService,
        users: PgUserRepository::new(pool.clone(), secret_box.clone()),
        audit: PgAuditRepository::new(pool.clone()),
        sessions: PgSessionRepository::new(pool.clone()),
        subscriptions: PgSubscriptionRepository::new(pool.clone(), secret_box.clone()),
        usage: PgUsageRepository::new(pool.clone()),
        notifications: PgNotificationRepository::new(pool.clone()),
        password_service: ArgonPasswordService,
        jwt_service: JwtService::new(&settings.access_jwt_secret, &settings.pre_auth_jwt_secret),
        totp_service: OtpAuthTotpService::new("Anneal"),
        token_hasher,
    };

    let app = Router::new()
        .route(
            "/api/v1/health",
            get(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
        )
        .route("/api/v1/bootstrap", post(auth::bootstrap))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/logout-all", post(auth::logout_all))
        .route("/api/v1/auth/totp/setup", post(auth::begin_totp_setup))
        .route("/api/v1/auth/totp/verify", post(auth::verify_totp))
        .route("/api/v1/auth/totp/disable", post(auth::disable_totp))
        .route("/api/v1/auth/password", post(auth::change_password))
        .route("/api/v1/auth/sessions", get(auth::list_sessions))
        .route("/api/v1/audit", get(audit::list_audit_logs))
        .route(
            "/api/v1/users",
            get(users::list_users).post(users::create_user),
        )
        .route(
            "/api/v1/users/{id}",
            patch(users::update_user).delete(users::delete_user),
        )
        .route(
            "/api/v1/resellers",
            get(users::list_resellers).post(users::create_reseller),
        )
        .route(
            "/api/v1/resellers/{id}",
            patch(users::update_reseller).delete(users::delete_reseller),
        )
        .route("/api/v1/usage", get(usage::list_usage))
        .route("/api/v1/devices", get(subscriptions::list_devices))
        .route(
            "/api/v1/subscriptions",
            get(subscriptions::list_subscriptions).post(subscriptions::create_subscription),
        )
        .route(
            "/api/v1/subscriptions/{id}",
            patch(subscriptions::update_subscription).delete(subscriptions::delete_subscription),
        )
        .route(
            "/api/v1/subscriptions/public/{token}",
            get(subscriptions::public_subscription),
        )
        .route(
            "/api/v1/subscriptions/{id}/rotate-link",
            post(subscriptions::rotate_subscription_link),
        )
        .route(
            "/api/v1/notifications",
            get(notifications::list_notifications),
        )
        .route("/s/{token}", get(subscriptions::resolve_subscription))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi()))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(settings.bind_address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
