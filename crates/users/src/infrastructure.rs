use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult, SecretBox, UserRole};

use crate::{
    application::UserRepository,
    domain::{Tenant, User},
};

#[derive(Clone)]
pub struct PgUserRepository {
    pool: PgPool,
    secret_box: SecretBox,
}

impl PgUserRepository {
    pub fn new(pool: PgPool, secret_box: SecretBox) -> Self {
        Self { pool, secret_box }
    }

    fn decrypt_user(&self, mut user: User) -> ApplicationResult<User> {
        user.totp_secret = self
            .secret_box
            .decrypt_option(user.totp_secret.as_deref())?;
        Ok(user)
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn bootstrap_superadmin(&self, user: User) -> ApplicationResult<User> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query("select pg_advisory_xact_lock(hashtext('anneal.bootstrap_superadmin'))")
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let count = sqlx::query_scalar::<_, i64>("select count(*) from users where role = $1")
            .bind(UserRole::Superadmin)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        if count > 0 {
            return Err(ApplicationError::Conflict(
                "bootstrap already completed".into(),
            ));
        }
        let existing = sqlx::query_scalar::<_, i64>(
            "select count(*) from users where lower(email) = lower($1)",
        )
        .bind(&user.email)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        if existing > 0 {
            return Err(ApplicationError::Conflict("email already exists".into()));
        }
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
        let user = sqlx::query_as::<_, User>("select * from users where lower(email) = lower($1)")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        user.map(|user| self.decrypt_user(user)).transpose()
    }

    async fn get_user_by_id(&self, user_id: Uuid) -> ApplicationResult<Option<User>> {
        let user = sqlx::query_as::<_, User>("select * from users where id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        user.map(|user| self.decrypt_user(user)).transpose()
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
        rows.into_iter().map(|user| self.decrypt_user(user)).collect()
    }

    async fn list_resellers(&self) -> ApplicationResult<Vec<User>> {
        let rows = sqlx::query_as::<_, User>("select * from users where role = $1 order by email asc")
            .bind(UserRole::Reseller)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        rows.into_iter().map(|user| self.decrypt_user(user)).collect()
    }

    async fn update_user(&self, user: User) -> ApplicationResult<User> {
        let encrypted_totp_secret = self
            .secret_box
            .encrypt_option(user.totp_secret.as_deref())?;
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
        .bind(&encrypted_totp_secret)
        .bind(user.totp_confirmed)
        .bind(user.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(user)
    }

    async fn update_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User> {
        let encrypted_totp_secret = self
            .secret_box
            .encrypt_option(user.totp_secret.as_deref())?;
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
        .bind(&encrypted_totp_secret)
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
        let encrypted_secret = self.secret_box.encrypt(secret)?;
        sqlx::query(
            "update users set totp_secret = $2, totp_confirmed = false, updated_at = now() at time zone 'utc' where id = $1",
        )
        .bind(user_id)
        .bind(encrypted_secret)
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
