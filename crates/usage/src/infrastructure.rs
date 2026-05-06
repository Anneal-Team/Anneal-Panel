use async_trait::async_trait;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult, QuotaState};

use crate::{
    application::UsageRepository,
    domain::{UsageOverview, UsageSample},
};

#[derive(Clone)]
pub struct PgUsageRepository {
    pool: PgPool,
}

impl PgUsageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UsageRepository for PgUsageRepository {
    async fn store_samples(&self, samples: Vec<UsageSample>) -> ApplicationResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        for sample in samples {
            sqlx::query(
                r#"
                insert into usage_samples (
                    id, tenant_id, subscription_id, device_id, bytes_in, bytes_out, measured_at, created_at
                ) values ($1,$2,$3,$4,$5,$6,$7,$8)
                "#,
            )
            .bind(sample.id)
            .bind(sample.tenant_id)
            .bind(sample.subscription_id)
            .bind(sample.device_id)
            .bind(sample.bytes_in)
            .bind(sample.bytes_out)
            .bind(sample.measured_at)
            .bind(sample.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
            let hour_bucket = hour_bucket(sample.measured_at);
            let day_bucket = sample.measured_at.date_naive();
            let total_bytes = sample.bytes_in + sample.bytes_out;
            sqlx::query(
                r#"
                insert into usage_rollups_hourly (subscription_id, bucket_start, total_bytes)
                values ($1,$2,$3)
                on conflict (subscription_id, bucket_start)
                do update set total_bytes = usage_rollups_hourly.total_bytes + excluded.total_bytes
                "#,
            )
            .bind(sample.subscription_id)
            .bind(hour_bucket)
            .bind(total_bytes)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
            sqlx::query(
                r#"
                insert into usage_rollups_daily (subscription_id, bucket_start, total_bytes)
                values ($1,$2,$3)
                on conflict (subscription_id, bucket_start)
                do update set total_bytes = usage_rollups_daily.total_bytes + excluded.total_bytes
                "#,
            )
            .bind(sample.subscription_id)
            .bind(day_bucket)
            .bind(total_bytes)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn update_subscription_usage(
        &self,
        subscription_id: Uuid,
        used_bytes: i64,
        quota_state: QuotaState,
        suspended: bool,
    ) -> ApplicationResult<()> {
        sqlx::query(
            "update subscriptions set used_bytes = $2, quota_state = $3, suspended = $4, updated_at = now() at time zone 'utc' where id = $1",
        )
        .bind(subscription_id)
        .bind(used_bytes)
        .bind(quota_state)
        .bind(suspended)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn list_usage_overview(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<UsageOverview>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, UsageOverview>(
                r#"
                select
                    s.id as subscription_id,
                    s.tenant_id,
                    s.name as subscription_name,
                    d.id as device_id,
                    d.name as device_name,
                    s.traffic_limit_bytes,
                    s.used_bytes,
                    s.quota_state,
                    s.suspended,
                    s.updated_at
                from subscriptions s
                join devices d on d.id = s.device_id
                where s.tenant_id = $1
                order by s.updated_at desc
                "#,
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, UsageOverview>(
                r#"
                select
                    s.id as subscription_id,
                    s.tenant_id,
                    s.name as subscription_name,
                    d.id as device_id,
                    d.name as device_name,
                    s.traffic_limit_bytes,
                    s.used_bytes,
                    s.quota_state,
                    s.suspended,
                    s.updated_at
                from subscriptions s
                join devices d on d.id = s.device_id
                order by s.updated_at desc
                "#,
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }
}

fn hour_bucket(value: DateTime<Utc>) -> DateTime<Utc> {
    value
        .with_minute(0)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or_else(|| {
            Utc.with_ymd_and_hms(value.year(), value.month(), value.day(), value.hour(), 0, 0)
                .single()
                .expect("valid hour bucket")
        })
}
