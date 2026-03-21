use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult};

use crate::{application::AuditRepository, domain::AuditLog};

#[derive(Clone)]
pub struct PgAuditRepository {
    pool: PgPool,
}

impl PgAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditRepository for PgAuditRepository {
    async fn create_log(&self, log: AuditLog) -> ApplicationResult<AuditLog> {
        sqlx::query(
            r#"
            insert into audit_logs (
                id, actor_user_id, tenant_id, action, resource_type, resource_id, payload, created_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
        )
        .bind(log.id)
        .bind(log.actor_user_id)
        .bind(log.tenant_id)
        .bind(&log.action)
        .bind(&log.resource_type)
        .bind(log.resource_id)
        .bind(&log.payload)
        .bind(log.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(log)
    }

    async fn list_logs(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<AuditLog>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, AuditLog>(
                "select * from audit_logs where tenant_id = $1 order by created_at desc",
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, AuditLog>("select * from audit_logs order by created_at desc")
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }
}
