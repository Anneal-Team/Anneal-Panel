use anneal_audit::PgAuditRepository;
use anneal_notifications::{NotificationService, PgNotificationRepository, TelegramNotifier};
use anneal_platform::{
    DeploymentJob, NotificationJob, Settings, connect_pool, init_telemetry, run_migrations,
};
use apalis::prelude::{Data, TaskSink, WorkerBuilder};
use apalis_postgres::{Config, PostgresStorage};
use chrono::Utc;
use sqlx::PgPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::from_env()?;
    init_telemetry("anneal-worker", &settings)?;
    let pool = connect_pool(&settings.database_url).await?;
    run_migrations(&pool, &settings.migrations_dir).await?;

    let deployment_storage =
        PostgresStorage::new_with_notify(&pool, &Config::new("deployment_jobs"));
    let notification_storage =
        PostgresStorage::new_with_notify(&pool, &Config::new("notification_jobs"));

    let deployment_worker = WorkerBuilder::new("anneal-deployment-worker")
        .backend(deployment_storage)
        .data(pool.clone())
        .build(process_deployment);
    let notification_worker = WorkerBuilder::new("anneal-notification-worker")
        .backend(notification_storage)
        .data(pool.clone())
        .data(settings.clone())
        .build(process_notification);

    let sweeper = tokio::spawn(run_node_offline_sweeper(
        pool.clone(),
        settings.clone(),
        PostgresStorage::new_with_config(&pool, &Config::new("notification_jobs")),
    ));

    tokio::try_join!(deployment_worker.run(), notification_worker.run())?;
    sweeper.abort();
    Ok(())
}

async fn process_deployment(job: DeploymentJob, pool: Data<PgPool>) -> anyhow::Result<()> {
    sqlx::query(
        "update deployment_rollouts set status = 'rendering', updated_at = now() at time zone 'utc' where id = $1",
    )
    .bind(job.rollout_id)
    .execute(&*pool)
    .await?;

    sqlx::query(
        "update deployment_rollouts set status = 'validating', updated_at = now() at time zone 'utc' where id = $1",
    )
    .bind(job.rollout_id)
    .execute(&*pool)
    .await?;

    let rollout = sqlx::query_as::<_, (String,)>(
        "select rendered_config from deployment_rollouts where id = $1",
    )
    .bind(job.rollout_id)
    .fetch_one(&*pool)
    .await?;

    let validation = serde_json::from_str::<serde_json::Value>(&rollout.0);
    match validation {
        Ok(_) => {
            sqlx::query(
                "update deployment_rollouts set status = 'ready', updated_at = now() at time zone 'utc' where id = $1",
            )
            .bind(job.rollout_id)
            .execute(&*pool)
            .await?;
        }
        Err(error) => {
            sqlx::query(
                "update deployment_rollouts set status = 'failed', failure_reason = $2, updated_at = now() at time zone 'utc' where id = $1",
            )
            .bind(job.rollout_id)
            .bind(error.to_string())
            .execute(&*pool)
            .await?;
        }
    }
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
    let _ = Utc::now();
    Ok(())
}

async fn run_node_offline_sweeper(
    pool: PgPool,
    settings: Settings,
    notification_queue: PostgresStorage<NotificationJob>,
) -> anyhow::Result<()> {
    let notifications = NotificationService::new(
        PgNotificationRepository::new(pool.clone()),
        TelegramNotifier::new(
            settings.telegram_bot_token.clone(),
            settings.telegram_chat_id.clone(),
        ),
    );
    let _audit = PgAuditRepository::new(pool.clone());
    loop {
        let stale_nodes = sqlx::query_as::<_, (uuid::Uuid, uuid::Uuid, String)>(
            r#"
            update nodes
            set status = 'offline', updated_at = now() at time zone 'utc'
            where status <> 'offline'
              and last_seen_at is not null
              and last_seen_at <= (now() at time zone 'utc' - interval '180 seconds')
            returning id, tenant_id, name
            "#,
        )
        .fetch_all(&pool)
        .await?;

        for (node_id, tenant_id, node_name) in stale_nodes {
            sqlx::query(
                r#"
                insert into audit_logs (id, actor_user_id, tenant_id, action, resource_type, resource_id, payload, created_at)
                values ($1, null, $2, 'nodes.offline', 'node', $3, $4, now() at time zone 'utc')
                "#,
            )
            .bind(uuid::Uuid::new_v4())
            .bind(tenant_id)
            .bind(node_id)
            .bind(serde_json::json!({ "name": node_name }))
            .execute(&pool)
            .await?;

            let event = notifications
                .create_event(
                    tenant_id,
                    anneal_notifications::NotificationKind::NodeOffline,
                    "Node offline".into(),
                    format!("Node {node_name} is offline"),
                )
                .await?;
            notification_queue
                .clone()
                .push(NotificationJob { event_id: event.id })
                .await?;
        }

        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}
