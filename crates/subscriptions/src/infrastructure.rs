use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult};

use crate::{
    application::SubscriptionRepository,
    domain::{Device, ResolvedSubscriptionContext, Subscription, SubscriptionLink},
};

#[derive(Clone)]
pub struct PgSubscriptionRepository {
    pool: PgPool,
}

impl PgSubscriptionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SubscriptionRepository for PgSubscriptionRepository {
    async fn create_device(&self, device: Device) -> ApplicationResult<Device> {
        sqlx::query(
            "insert into devices (id, tenant_id, user_id, name, device_token, suspended, created_at, updated_at) values ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(device.id)
        .bind(device.tenant_id)
        .bind(device.user_id)
        .bind(&device.name)
        .bind(&device.device_token)
        .bind(device.suspended)
        .bind(device.created_at)
        .bind(device.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(device)
    }

    async fn create_subscription(
        &self,
        device: Device,
        subscription: Subscription,
        link: SubscriptionLink,
    ) -> ApplicationResult<(Subscription, SubscriptionLink)> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            "insert into devices (id, tenant_id, user_id, name, device_token, suspended, created_at, updated_at) values ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(device.id)
        .bind(device.tenant_id)
        .bind(device.user_id)
        .bind(&device.name)
        .bind(&device.device_token)
        .bind(device.suspended)
        .bind(device.created_at)
        .bind(device.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            r#"
            insert into subscriptions (
                id, tenant_id, user_id, device_id, name, note, access_key, traffic_limit_bytes, used_bytes, quota_state, suspended, expires_at, created_at, updated_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            "#,
        )
        .bind(subscription.id)
        .bind(subscription.tenant_id)
        .bind(subscription.user_id)
        .bind(subscription.device_id)
        .bind(&subscription.name)
        .bind(&subscription.note)
        .bind(&subscription.access_key)
        .bind(subscription.traffic_limit_bytes)
        .bind(subscription.used_bytes)
        .bind(subscription.quota_state)
        .bind(subscription.suspended)
        .bind(subscription.expires_at)
        .bind(subscription.created_at)
        .bind(subscription.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            "insert into subscription_links (id, subscription_id, token, revoked_at, created_at) values ($1,$2,$3,$4,$5)",
        )
        .bind(link.id)
        .bind(link.subscription_id)
        .bind(&link.token)
        .bind(link.revoked_at)
        .bind(link.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok((subscription, link))
    }

    async fn list_devices(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<Device>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, Device>(
                "select * from devices where tenant_id = $1 order by name asc",
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, Device>("select * from devices order by name asc")
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }

    async fn list_subscriptions(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<Subscription>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, Subscription>(
                r#"
                select
                    s.*,
                    active_link.token as current_token
                from subscriptions s
                left join lateral (
                    select token
                    from subscription_links
                    where subscription_id = s.id and revoked_at is null
                    order by created_at desc
                    limit 1
                ) active_link on true
                where s.tenant_id = $1
                order by s.name asc
                "#,
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, Subscription>(
                r#"
                select
                    s.*,
                    active_link.token as current_token
                from subscriptions s
                left join lateral (
                    select token
                    from subscription_links
                    where subscription_id = s.id and revoked_at is null
                    order by created_at desc
                    limit 1
                ) active_link on true
                order by s.name asc
                "#,
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }

    async fn get_subscription(&self, subscription_id: Uuid) -> ApplicationResult<Option<Subscription>> {
        sqlx::query_as::<_, Subscription>(
            r#"
            select
                s.*,
                active_link.token as current_token
            from subscriptions s
            left join lateral (
                select token
                from subscription_links
                where subscription_id = s.id and revoked_at is null
                order by created_at desc
                limit 1
            ) active_link on true
            where s.id = $1
            "#,
        )
        .bind(subscription_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn update_subscription(&self, subscription: Subscription) -> ApplicationResult<Subscription> {
        sqlx::query(
            r#"
            update subscriptions
            set name = $2, note = $3, traffic_limit_bytes = $4, used_bytes = $5, quota_state = $6, suspended = $7, expires_at = $8, updated_at = $9
            where id = $1
            "#,
        )
        .bind(subscription.id)
        .bind(&subscription.name)
        .bind(&subscription.note)
        .bind(subscription.traffic_limit_bytes)
        .bind(subscription.used_bytes)
        .bind(subscription.quota_state)
        .bind(subscription.suspended)
        .bind(subscription.expires_at)
        .bind(subscription.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(subscription)
    }

    async fn delete_subscription(&self, subscription_id: Uuid) -> ApplicationResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let device_id = sqlx::query_scalar::<_, Uuid>(
            "select device_id from subscriptions where id = $1",
        )
        .bind(subscription_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query("delete from subscriptions where id = $1")
            .bind(subscription_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        if let Some(device_id) = device_id {
            sqlx::query(
                r#"
                delete from devices
                where id = $1
                  and not exists (
                    select 1 from subscriptions where device_id = $1
                  )
                "#,
            )
            .bind(device_id)
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

    async fn rotate_device_token(
        &self,
        device_id: Uuid,
        device_token: &str,
    ) -> ApplicationResult<()> {
        sqlx::query(
            "update devices set device_token = $2, updated_at = now() at time zone 'utc' where id = $1",
        )
        .bind(device_id)
        .bind(device_token)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn rotate_subscription_token(
        &self,
        subscription_id: Uuid,
        token: &str,
    ) -> ApplicationResult<SubscriptionLink> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            "update subscription_links set revoked_at = now() at time zone 'utc' where subscription_id = $1 and revoked_at is null",
        )
        .bind(subscription_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let link = SubscriptionLink {
            id: Uuid::new_v4(),
            subscription_id,
            token: token.into(),
            revoked_at: None,
            created_at: chrono::Utc::now(),
        };
        sqlx::query(
            "insert into subscription_links (id, subscription_id, token, revoked_at, created_at) values ($1,$2,$3,$4,$5)",
        )
        .bind(link.id)
        .bind(link.subscription_id)
        .bind(&link.token)
        .bind(link.revoked_at)
        .bind(link.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(link)
    }

    async fn find_by_subscription_token(
        &self,
        token: &str,
    ) -> ApplicationResult<Option<ResolvedSubscriptionContext>> {
        let link = sqlx::query_as::<_, SubscriptionLink>(
            "select * from subscription_links where token = $1 and revoked_at is null limit 1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let Some(link) = link else {
            return Ok(None);
        };
        let subscription =
            sqlx::query_as::<_, Subscription>("select * from subscriptions where id = $1")
                .bind(link.subscription_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(subscription.map(|subscription| ResolvedSubscriptionContext { subscription, link }))
    }
}
