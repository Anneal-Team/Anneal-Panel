use async_trait::async_trait;
use sqlx::PgPool;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult, SecretBox};

use crate::{
    application::SubscriptionRepository,
    domain::{Device, ResolvedSubscriptionContext, Subscription, SubscriptionLink},
};

#[derive(Clone)]
pub struct PgSubscriptionRepository {
    pool: PgPool,
    secret_box: SecretBox,
}

impl PgSubscriptionRepository {
    pub fn new(pool: PgPool, secret_box: SecretBox) -> Self {
        Self { pool, secret_box }
    }

    fn decrypt_device(&self, mut device: Device) -> ApplicationResult<Device> {
        device.device_token = self.secret_box.decrypt(&device.device_token)?;
        Ok(device)
    }

    fn decrypt_subscription(&self, mut subscription: Subscription) -> ApplicationResult<Subscription> {
        subscription.access_key = self.secret_box.decrypt(&subscription.access_key)?;
        subscription.current_token = self
            .secret_box
            .decrypt_option(subscription.current_token.as_deref())?;
        Ok(subscription)
    }

    fn decrypt_link(&self, mut link: SubscriptionLink) -> ApplicationResult<SubscriptionLink> {
        link.token = self.secret_box.decrypt(&link.token)?;
        Ok(link)
    }
}

#[async_trait]
impl SubscriptionRepository for PgSubscriptionRepository {
    async fn tenant_owns_user(&self, tenant_id: Uuid, user_id: Uuid) -> ApplicationResult<bool> {
        sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from users where id = $1 and tenant_id = $2)",
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn tenant_owns_device(
        &self,
        tenant_id: Uuid,
        device_id: Uuid,
    ) -> ApplicationResult<bool> {
        sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from devices where id = $1 and tenant_id = $2)",
        )
        .bind(device_id)
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn create_device(&self, device: Device) -> ApplicationResult<Device> {
        let encrypted_device_token = self.secret_box.encrypt(&device.device_token)?;
        sqlx::query(
            "insert into devices (id, tenant_id, user_id, name, device_token, suspended, created_at, updated_at) values ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(device.id)
        .bind(device.tenant_id)
        .bind(device.user_id)
        .bind(&device.name)
        .bind(&encrypted_device_token)
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
        let encrypted_device_token = self.secret_box.encrypt(&device.device_token)?;
        let encrypted_access_key = self.secret_box.encrypt(&subscription.access_key)?;
        let encrypted_link_token = self.secret_box.encrypt(&link.token)?;
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
        .bind(&encrypted_device_token)
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
        .bind(&encrypted_access_key)
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
            "insert into subscription_links (id, subscription_id, token, token_hash, revoked_at, created_at) values ($1,$2,$3,$4,$5,$6)",
        )
        .bind(link.id)
        .bind(link.subscription_id)
        .bind(&encrypted_link_token)
        .bind(&link.token_hash)
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
        rows.into_iter()
            .map(|device| self.decrypt_device(device))
            .collect()
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
        rows.into_iter()
            .map(|subscription| self.decrypt_subscription(subscription))
            .collect()
    }

    async fn get_subscription(
        &self,
        subscription_id: Uuid,
    ) -> ApplicationResult<Option<Subscription>> {
        let subscription = sqlx::query_as::<_, Subscription>(
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
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        subscription
            .map(|subscription| self.decrypt_subscription(subscription))
            .transpose()
    }

    async fn update_subscription(
        &self,
        subscription: Subscription,
    ) -> ApplicationResult<Subscription> {
        let encrypted_access_key = self.secret_box.encrypt(&subscription.access_key)?;
        sqlx::query(
            r#"
            update subscriptions
            set name = $2, note = $3, access_key = $4, traffic_limit_bytes = $5, used_bytes = $6, quota_state = $7, suspended = $8, expires_at = $9, updated_at = $10
            where id = $1
            "#,
        )
        .bind(subscription.id)
        .bind(&subscription.name)
        .bind(&subscription.note)
        .bind(&encrypted_access_key)
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
        let device_id =
            sqlx::query_scalar::<_, Uuid>("select device_id from subscriptions where id = $1")
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
        let encrypted_device_token = self.secret_box.encrypt(device_token)?;
        sqlx::query(
            "update devices set device_token = $2, updated_at = now() at time zone 'utc' where id = $1",
        )
        .bind(device_id)
        .bind(encrypted_device_token)
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
        let encrypted_token = self.secret_box.encrypt(token)?;
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
            token_hash: hash_token(token),
            revoked_at: None,
            created_at: chrono::Utc::now(),
        };
        sqlx::query(
            "insert into subscription_links (id, subscription_id, token, token_hash, revoked_at, created_at) values ($1,$2,$3,$4,$5,$6)",
        )
        .bind(link.id)
        .bind(link.subscription_id)
        .bind(&encrypted_token)
        .bind(&link.token_hash)
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
            "select * from subscription_links where token_hash = $1 and revoked_at is null limit 1",
        )
        .bind(hash_token(token))
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
        let link = self.decrypt_link(link)?;
        let subscription = subscription
            .map(|subscription| self.decrypt_subscription(subscription))
            .transpose()?;
        Ok(subscription.map(|subscription| ResolvedSubscriptionContext { subscription, link }))
    }
}

fn hash_token(token: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(token.as_bytes());
    format!("{:x}", digest.finalize())
}
