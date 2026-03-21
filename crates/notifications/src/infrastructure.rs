use async_trait::async_trait;
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult};

use crate::{
    application::{NotificationRepository, Notifier},
    domain::NotificationEvent,
};

#[derive(Clone)]
pub struct PgNotificationRepository {
    pool: PgPool,
}

impl PgNotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationRepository for PgNotificationRepository {
    async fn create_event(&self, event: NotificationEvent) -> ApplicationResult<NotificationEvent> {
        sqlx::query(
            "insert into notification_events (id, tenant_id, kind, title, body, delivered_at, created_at) values ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(event.id)
        .bind(event.tenant_id)
        .bind(event.kind)
        .bind(&event.title)
        .bind(&event.body)
        .bind(event.delivered_at)
        .bind(event.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(event)
    }

    async fn mark_delivered(&self, event_id: Uuid) -> ApplicationResult<()> {
        sqlx::query(
            "update notification_events set delivered_at = now() at time zone 'utc' where id = $1",
        )
        .bind(event_id)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn list_events(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<NotificationEvent>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, NotificationEvent>(
                "select * from notification_events where tenant_id = $1 order by created_at desc",
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, NotificationEvent>(
                "select * from notification_events order by created_at desc",
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }

    async fn get_event(&self, event_id: Uuid) -> ApplicationResult<Option<NotificationEvent>> {
        sqlx::query_as::<_, NotificationEvent>("select * from notification_events where id = $1")
            .bind(event_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }
}

#[derive(Clone)]
pub struct TelegramNotifier {
    client: Client,
    bot_token: Option<String>,
    chat_id: Option<String>,
}

impl TelegramNotifier {
    pub fn new(bot_token: Option<String>, chat_id: Option<String>) -> Self {
        Self {
            client: Client::new(),
            bot_token,
            chat_id,
        }
    }
}

#[async_trait]
impl Notifier for TelegramNotifier {
    async fn send(&self, event: &NotificationEvent) -> ApplicationResult<bool> {
        let (Some(bot_token), Some(chat_id)) = (&self.bot_token, &self.chat_id) else {
            return Ok(false);
        };
        let endpoint = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
        self.client
            .post(endpoint)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": format!("{}\n\n{}", event.title, event.body),
            }))
            .send()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(true)
    }
}
