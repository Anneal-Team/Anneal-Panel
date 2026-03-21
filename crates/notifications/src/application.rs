use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult};

use crate::domain::{NotificationEvent, NotificationKind};

#[async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn create_event(&self, event: NotificationEvent) -> ApplicationResult<NotificationEvent>;
    async fn mark_delivered(&self, event_id: Uuid) -> ApplicationResult<()>;
    async fn list_events(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<NotificationEvent>>;
    async fn get_event(&self, event_id: Uuid) -> ApplicationResult<Option<NotificationEvent>>;
}

#[async_trait]
impl<T> NotificationRepository for &T
where
    T: NotificationRepository + Send + Sync,
{
    async fn create_event(&self, event: NotificationEvent) -> ApplicationResult<NotificationEvent> {
        (*self).create_event(event).await
    }

    async fn mark_delivered(&self, event_id: Uuid) -> ApplicationResult<()> {
        (*self).mark_delivered(event_id).await
    }

    async fn list_events(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<NotificationEvent>> {
        (*self).list_events(tenant_id).await
    }

    async fn get_event(&self, event_id: Uuid) -> ApplicationResult<Option<NotificationEvent>> {
        (*self).get_event(event_id).await
    }
}

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn send(&self, event: &NotificationEvent) -> ApplicationResult<bool>;
}

pub struct NotificationService<R, N> {
    repository: R,
    notifier: N,
}

impl<R, N> NotificationService<R, N> {
    pub fn new(repository: R, notifier: N) -> Self {
        Self {
            repository,
            notifier,
        }
    }
}

impl<R, N> NotificationService<R, N>
where
    R: NotificationRepository,
    N: Notifier,
{
    pub async fn create_event(
        &self,
        tenant_id: Uuid,
        kind: NotificationKind,
        title: String,
        body: String,
    ) -> ApplicationResult<NotificationEvent> {
        self.repository
            .create_event(NotificationEvent {
                id: Uuid::new_v4(),
                tenant_id,
                kind,
                title,
                body,
                delivered_at: None,
                created_at: Utc::now(),
            })
            .await
    }

    pub async fn deliver(&self, event_id: Uuid) -> ApplicationResult<bool> {
        let event = self
            .repository
            .get_event(event_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("notification event not found".into()))?;
        let delivered = self.notifier.send(&event).await?;
        if delivered {
            self.repository.mark_delivered(event_id).await?;
        }
        Ok(delivered)
    }

    pub async fn list_events(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<NotificationEvent>> {
        self.repository.list_events(tenant_id).await
    }
}
