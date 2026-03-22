use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult, UserRole};

use crate::{
    application::UserRepository,
    domain::{Tenant, User},
};

#[derive(Clone)]
pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn create_user(&self, user: User) -> ApplicationResult<User> {
        sqlx::query(
            r#"
            insert into users (
                id, tenant_id, email, display_name, role, status, password_hash, totp_confirmed, created_at, updated_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            "#,
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .bind(&user.email)
        .bind(&user.display_name)
        .bind(user.role)
        .bind(user.status)
        .bind(&user.password_hash)
        .bind(user.totp_confirmed)
        .bind(user.created_at)
        .bind(user.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(user)
    }

    async fn create_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            r#"
            insert into tenants (id, name, owner_user_id, created_at, updated_at)
            values ($1,$2,$3,$4,$5)
            "#,
        )
        .bind(tenant.id)
        .bind(&tenant.name)
        .bind(tenant.owner_user_id)
        .bind(tenant.created_at)
        .bind(tenant.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            r#"
            insert into users (
                id, tenant_id, email, display_name, role, status, password_hash, totp_confirmed, created_at, updated_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            "#,
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .bind(&user.email)
        .bind(&user.display_name)
        .bind(user.role)
        .bind(user.status)
        .bind(&user.password_hash)
        .bind(user.totp_confirmed)
        .bind(user.created_at)
        .bind(user.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(user)
    }

    async fn get_user_by_email(&self, email: &str) -> ApplicationResult<Option<User>> {
        sqlx::query_as::<_, User>("select * from users where lower(email) = lower($1)")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn get_user_by_id(&self, user_id: Uuid) -> ApplicationResult<Option<User>> {
        sqlx::query_as::<_, User>("select * from users where id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn get_tenant_by_id(&self, tenant_id: Uuid) -> ApplicationResult<Option<Tenant>> {
        sqlx::query_as::<_, Tenant>("select * from tenants where id = $1")
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn get_tenant_by_owner_user_id(
        &self,
        user_id: Uuid,
    ) -> ApplicationResult<Option<Tenant>> {
        sqlx::query_as::<_, Tenant>("select * from tenants where owner_user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn count_superadmins(&self) -> ApplicationResult<i64> {
        sqlx::query_scalar::<_, i64>("select count(*) from users where role = $1")
            .bind(UserRole::Superadmin)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn list_users_by_tenant(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<User>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, User>("select * from users where tenant_id = $1 order by email asc")
                .bind(tenant_id)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query_as::<_, User>("select * from users order by email asc")
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }

    async fn list_resellers(&self) -> ApplicationResult<Vec<User>> {
        sqlx::query_as::<_, User>("select * from users where role = $1 order by email asc")
            .bind(UserRole::Reseller)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn update_user(&self, user: User) -> ApplicationResult<User> {
        sqlx::query(
            r#"
            update users
            set tenant_id = $2, email = $3, display_name = $4, role = $5, status = $6, password_hash = $7, totp_secret = $8, totp_confirmed = $9, updated_at = $10
            where id = $1
            "#,
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .bind(&user.email)
        .bind(&user.display_name)
        .bind(user.role)
        .bind(user.status)
        .bind(&user.password_hash)
        .bind(&user.totp_secret)
        .bind(user.totp_confirmed)
        .bind(user.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(user)
    }

    async fn update_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            "update tenants set name = $2, owner_user_id = $3, updated_at = $4 where id = $1",
        )
        .bind(tenant.id)
        .bind(&tenant.name)
        .bind(tenant.owner_user_id)
        .bind(tenant.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            r#"
            update users
            set tenant_id = $2, email = $3, display_name = $4, role = $5, status = $6, password_hash = $7, totp_secret = $8, totp_confirmed = $9, updated_at = $10
            where id = $1
            "#,
        )
        .bind(user.id)
        .bind(user.tenant_id)
        .bind(&user.email)
        .bind(&user.display_name)
        .bind(user.role)
        .bind(user.status)
        .bind(&user.password_hash)
        .bind(&user.totp_secret)
        .bind(user.totp_confirmed)
        .bind(user.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(user)
    }

    async fn delete_user(&self, user_id: Uuid) -> ApplicationResult<()> {
        sqlx::query("delete from users where id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn delete_tenant(&self, tenant_id: Uuid) -> ApplicationResult<()> {
        sqlx::query("delete from tenants where id = $1")
            .bind(tenant_id)
            .execute(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> ApplicationResult<()> {
        sqlx::query("update users set password_hash = $2, updated_at = now() at time zone 'utc' where id = $1")
            .bind(user_id)
            .bind(password_hash)
            .execute(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn save_totp_secret(&self, user_id: Uuid, secret: &str) -> ApplicationResult<()> {
        sqlx::query(
            "update users set totp_secret = $2, totp_confirmed = false, updated_at = now() at time zone 'utc' where id = $1",
        )
        .bind(user_id)
        .bind(secret)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn confirm_totp(&self, user_id: Uuid) -> ApplicationResult<()> {
        sqlx::query("update users set totp_confirmed = true, updated_at = now() at time zone 'utc' where id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn clear_totp(&self, user_id: Uuid) -> ApplicationResult<()> {
        sqlx::query(
            "update users set totp_secret = null, totp_confirmed = false, updated_at = now() at time zone 'utc' where id = $1",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }
}
