use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use anneal_core::ApplicationResult;

use crate::domain::AuditLog;

#[async_trait]
pub trait AuditRepository: Send + Sync {
    async fn create_log(&self, log: AuditLog) -> ApplicationResult<AuditLog>;
    async fn list_logs(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<AuditLog>>;
}

#[async_trait]
impl<T> AuditRepository for &T
where
    T: AuditRepository + Send + Sync,
{
    async fn create_log(&self, log: AuditLog) -> ApplicationResult<AuditLog> {
        (*self).create_log(log).await
    }

    async fn list_logs(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<AuditLog>> {
        (*self).list_logs(tenant_id).await
    }
}

pub struct AuditService<R> {
    repository: R,
}

impl<R> AuditService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R> AuditService<R>
where
    R: AuditRepository,
{
    pub async fn write(
        &self,
        actor_user_id: Option<Uuid>,
        tenant_id: Option<Uuid>,
        action: impl Into<String>,
        resource_type: impl Into<String>,
        resource_id: Option<Uuid>,
        payload: Value,
    ) -> ApplicationResult<AuditLog> {
        self.repository
            .create_log(AuditLog {
                id: Uuid::new_v4(),
                actor_user_id,
                tenant_id,
                action: action.into(),
                resource_type: resource_type.into(),
                resource_id,
                payload,
                created_at: chrono::Utc::now(),
            })
            .await
    }

    pub async fn list(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<AuditLog>> {
        self.repository.list_logs(tenant_id).await
    }
}
