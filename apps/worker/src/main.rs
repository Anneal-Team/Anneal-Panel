use anneal_core::SecretBox;
use anneal_notifications::{NotificationService, PgNotificationRepository, TelegramNotifier};
use anneal_platform::{
    NotificationJob, Settings, backfill_protected_data, connect_pool, init_telemetry,
    run_migrations,
};
use apalis::prelude::{Data, WorkerBuilder};
use apalis_postgres::{Config, PostgresStorage};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;
    let secret_box = SecretBox::new(&settings.data_encryption_key)?;
    let token_hasher = anneal_core::TokenHasher::new(&settings.token_hash_key)?;
    init_telemetry("anneal-worker", &settings)?;
    let pool = connect_pool(&settings.database_url).await?;
    run_migrations(&pool, &settings.migrations_dir).await?;
    backfill_protected_data(&pool, &secret_box, &token_hasher).await?;

    let notification_storage =
        PostgresStorage::new_with_notify(&pool, &Config::new("notification_jobs"));
    let notification_worker = WorkerBuilder::new("anneal-notification-worker")
        .backend(notification_storage)
        .data(pool.clone())
        .data(settings.clone())
        .build(process_notification);

    notification_worker.run().await?;
    Ok(())
}

async fn process_notification(
    job: NotificationJob,
    pool: Data<PgPool>,
    settings: Data<Settings>,
) -> anyhow::Result<()> {
    let service = NotificationService::new(
        PgNotificationRepository::new((*pool).clone()),
        TelegramNotifier::new(
            settings.telegram_bot_token.clone(),
            settings.telegram_chat_id.clone(),
        ),
    );
    let delivered = service.deliver(job.event_id).await?;
    if !delivered {
        sqlx::query(
            "update notification_events set delivered_at = delivered_at where id = $1 and delivered_at is null",
        )
        .bind(job.event_id)
        .execute(&*pool)
        .await?;
    }
    Ok(())
}
