use std::{collections::HashMap, sync::RwLock};

use anneal_core::{Actor, ApplicationError, ApplicationResult, UserRole, UserStatus};
use anneal_rbac::{AccessScope, Permission, RbacService};
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use crate::domain::{
    CreateResellerCommand, CreateUserCommand, Tenant, UpdateResellerCommand, UpdateUserCommand,
    User,
};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create_user(&self, user: User) -> ApplicationResult<User>;
    async fn create_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User>;
    async fn get_user_by_email(&self, email: &str) -> ApplicationResult<Option<User>>;
    async fn get_user_by_id(&self, user_id: Uuid) -> ApplicationResult<Option<User>>;
    async fn get_tenant_by_id(&self, tenant_id: Uuid) -> ApplicationResult<Option<Tenant>>;
    async fn get_tenant_by_owner_user_id(&self, user_id: Uuid) -> ApplicationResult<Option<Tenant>>;
    async fn count_superadmins(&self) -> ApplicationResult<i64>;
    async fn list_users_by_tenant(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<User>>;
    async fn list_resellers(&self) -> ApplicationResult<Vec<User>>;
    async fn update_user(&self, user: User) -> ApplicationResult<User>;
    async fn update_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User>;
    async fn delete_user(&self, user_id: Uuid) -> ApplicationResult<()>;
    async fn delete_tenant(&self, tenant_id: Uuid) -> ApplicationResult<()>;
    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> ApplicationResult<()>;
    async fn save_totp_secret(&self, user_id: Uuid, secret: &str) -> ApplicationResult<()>;
    async fn confirm_totp(&self, user_id: Uuid) -> ApplicationResult<()>;
    async fn clear_totp(&self, user_id: Uuid) -> ApplicationResult<()>;
}

#[async_trait]
impl<T> UserRepository for &T
where
    T: UserRepository + Send + Sync,
{
    async fn create_user(&self, user: User) -> ApplicationResult<User> {
        (*self).create_user(user).await
    }

    async fn create_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User> {
        (*self).create_reseller_bundle(tenant, user).await
    }

    async fn get_user_by_email(&self, email: &str) -> ApplicationResult<Option<User>> {
        (*self).get_user_by_email(email).await
    }

    async fn get_user_by_id(&self, user_id: Uuid) -> ApplicationResult<Option<User>> {
        (*self).get_user_by_id(user_id).await
    }

    async fn get_tenant_by_id(&self, tenant_id: Uuid) -> ApplicationResult<Option<Tenant>> {
        (*self).get_tenant_by_id(tenant_id).await
    }

    async fn get_tenant_by_owner_user_id(&self, user_id: Uuid) -> ApplicationResult<Option<Tenant>> {
        (*self).get_tenant_by_owner_user_id(user_id).await
    }

    async fn count_superadmins(&self) -> ApplicationResult<i64> {
        (*self).count_superadmins().await
    }

    async fn list_users_by_tenant(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<User>> {
        (*self).list_users_by_tenant(tenant_id).await
    }

    async fn list_resellers(&self) -> ApplicationResult<Vec<User>> {
        (*self).list_resellers().await
    }

    async fn update_user(&self, user: User) -> ApplicationResult<User> {
        (*self).update_user(user).await
    }

    async fn update_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User> {
        (*self).update_reseller_bundle(tenant, user).await
    }

    async fn delete_user(&self, user_id: Uuid) -> ApplicationResult<()> {
        (*self).delete_user(user_id).await
    }

    async fn delete_tenant(&self, tenant_id: Uuid) -> ApplicationResult<()> {
        (*self).delete_tenant(tenant_id).await
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> ApplicationResult<()> {
        (*self).update_password_hash(user_id, password_hash).await
    }

    async fn save_totp_secret(&self, user_id: Uuid, secret: &str) -> ApplicationResult<()> {
        (*self).save_totp_secret(user_id, secret).await
    }

    async fn confirm_totp(&self, user_id: Uuid) -> ApplicationResult<()> {
        (*self).confirm_totp(user_id).await
    }

    async fn clear_totp(&self, user_id: Uuid) -> ApplicationResult<()> {
        (*self).clear_totp(user_id).await
    }
}

pub struct UserService<R> {
    repository: R,
    rbac: RbacService,
}

impl<R> UserService<R> {
    pub fn new(repository: R, rbac: RbacService) -> Self {
        Self { repository, rbac }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }
}

impl<R> UserService<R>
where
    R: UserRepository,
{
    pub async fn bootstrap_superadmin(
        &self,
        email: String,
        display_name: String,
        password_hash: String,
    ) -> ApplicationResult<User> {
        if self.repository.count_superadmins().await? > 0 {
            return Err(ApplicationError::Conflict(
                "bootstrap already completed".into(),
            ));
        }
        if self.repository.get_user_by_email(&email).await?.is_some() {
            return Err(ApplicationError::Conflict(
                "superadmin already exists".into(),
            ));
        }
        let now = Utc::now();
        let user = User {
            id: Uuid::new_v4(),
            tenant_id: None,
            tenant_name: None,
            email,
            display_name,
            role: UserRole::Superadmin,
            status: UserStatus::Active,
            password_hash,
            totp_secret: None,
            totp_confirmed: false,
            created_at: now,
            updated_at: now,
        };
        self.repository.create_user(user).await
    }

    pub async fn create_reseller(
        &self,
        actor: &Actor,
        command: CreateResellerCommand,
    ) -> ApplicationResult<User> {
        self.rbac.authorize(
            actor,
            Permission::ManageResellers,
            AccessScope {
                target_tenant_id: None,
            },
        )?;
        if self
            .repository
            .get_user_by_email(&command.email)
            .await?
            .is_some()
        {
            return Err(ApplicationError::Conflict("email already exists".into()));
        }
        let now = Utc::now();
        let owner_user_id = Uuid::new_v4();
        let tenant = Tenant {
            id: Uuid::new_v4(),
            name: command.tenant_name,
            owner_user_id,
            created_at: now,
            updated_at: now,
        };
        let user = User {
            id: owner_user_id,
            tenant_id: Some(tenant.id),
            tenant_name: Some(tenant.name.clone()),
            email: command.email,
            display_name: command.display_name,
            role: UserRole::Reseller,
            status: UserStatus::Active,
            password_hash: command.password_hash,
            totp_secret: None,
            totp_confirmed: false,
            created_at: now,
            updated_at: now,
        };
        let created = self.repository.create_reseller_bundle(tenant, user).await?;
        self.attach_tenant_name(created).await
    }

    pub async fn create_user(
        &self,
        actor: &Actor,
        command: CreateUserCommand,
    ) -> ApplicationResult<User> {
        let tenant_id = match actor.role {
            UserRole::Reseller => actor.tenant_id,
            _ => command.target_tenant_id,
        };
        let permission = if actor.role == UserRole::Reseller {
            Permission::ManageTenantUsers
        } else {
            Permission::ManageGlobalUsers
        };
        self.rbac.authorize(
            actor,
            permission,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        if command.role == UserRole::Superadmin && actor.role != UserRole::Superadmin {
            return Err(ApplicationError::Forbidden);
        }
        if self
            .repository
            .get_user_by_email(&command.email)
            .await?
            .is_some()
        {
            return Err(ApplicationError::Conflict("email already exists".into()));
        }
        let now = Utc::now();
        let user = User {
            id: Uuid::new_v4(),
            tenant_id,
            tenant_name: None,
            email: command.email,
            display_name: command.display_name,
            role: command.role,
            status: UserStatus::Active,
            password_hash: command.password_hash,
            totp_secret: None,
            totp_confirmed: false,
            created_at: now,
            updated_at: now,
        };
        let created = self.repository.create_user(user).await?;
        self.attach_tenant_name(created).await
    }

    pub async fn list_users(&self, actor: &Actor) -> ApplicationResult<Vec<User>> {
        let tenant_id = match actor.role {
            UserRole::Reseller => actor.tenant_id,
            _ => None,
        };
        let permission = if actor.role == UserRole::Reseller {
            Permission::ManageTenantUsers
        } else {
            Permission::ManageGlobalUsers
        };
        self.rbac.authorize(
            actor,
            permission,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        let users = self.repository.list_users_by_tenant(tenant_id).await?;
        self.attach_tenant_names(users).await
    }

    pub async fn list_resellers(&self, actor: &Actor) -> ApplicationResult<Vec<User>> {
        self.rbac.authorize(
            actor,
            Permission::ManageResellers,
            AccessScope {
                target_tenant_id: None,
            },
        )?;
        let users = self.repository.list_resellers().await?;
        self.attach_tenant_names(users).await
    }

    pub async fn update_user(
        &self,
        actor: &Actor,
        user_id: Uuid,
        command: UpdateUserCommand,
    ) -> ApplicationResult<User> {
        let mut user = self
            .repository
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        if user.role == UserRole::Reseller {
            return Err(ApplicationError::Validation(
                "reseller accounts must be updated through reseller settings".into(),
            ));
        }
        let tenant_id = if actor.role == UserRole::Reseller {
            actor.tenant_id
        } else {
            user.tenant_id
        };
        let permission = if actor.role == UserRole::Reseller {
            Permission::ManageTenantUsers
        } else {
            Permission::ManageGlobalUsers
        };
        self.rbac.authorize(
            actor,
            permission,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        if actor.role != UserRole::Superadmin && user.role == UserRole::Superadmin {
            return Err(ApplicationError::Forbidden);
        }
        if command.role == UserRole::Superadmin && actor.role != UserRole::Superadmin {
            return Err(ApplicationError::Forbidden);
        }
        if command.role == UserRole::Reseller {
            return Err(ApplicationError::Validation(
                "changing role to reseller is not supported here".into(),
            ));
        }
        if let Some(existing) = self.repository.get_user_by_email(&command.email).await? {
            if existing.id != user.id {
                return Err(ApplicationError::Conflict("email already exists".into()));
            }
        }
        user.email = command.email;
        user.display_name = command.display_name;
        user.role = command.role;
        user.status = command.status;
        if let Some(password_hash) = command.password_hash {
            user.password_hash = password_hash;
        }
        user.updated_at = Utc::now();
        let updated = self.repository.update_user(user).await?;
        self.attach_tenant_name(updated).await
    }

    pub async fn delete_user(&self, actor: &Actor, user_id: Uuid) -> ApplicationResult<()> {
        let user = self
            .repository
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        if user.role == UserRole::Reseller {
            return Err(ApplicationError::Validation(
                "reseller accounts must be deleted through reseller settings".into(),
            ));
        }
        if user.role == UserRole::Superadmin {
            return Err(ApplicationError::Validation(
                "superadmin cannot be deleted".into(),
            ));
        }
        let tenant_id = if actor.role == UserRole::Reseller {
            actor.tenant_id
        } else {
            user.tenant_id
        };
        let permission = if actor.role == UserRole::Reseller {
            Permission::ManageTenantUsers
        } else {
            Permission::ManageGlobalUsers
        };
        self.rbac.authorize(
            actor,
            permission,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        self.repository.delete_user(user_id).await
    }

    pub async fn update_reseller(
        &self,
        actor: &Actor,
        reseller_id: Uuid,
        command: UpdateResellerCommand,
    ) -> ApplicationResult<User> {
        self.rbac.authorize(
            actor,
            Permission::ManageResellers,
            AccessScope {
                target_tenant_id: None,
            },
        )?;
        let mut user = self
            .repository
            .get_user_by_id(reseller_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("reseller not found".into()))?;
        if user.role != UserRole::Reseller {
            return Err(ApplicationError::Validation("account is not a reseller".into()));
        }
        if let Some(existing) = self.repository.get_user_by_email(&command.email).await? {
            if existing.id != user.id {
                return Err(ApplicationError::Conflict("email already exists".into()));
            }
        }
        let tenant_id = user
            .tenant_id
            .ok_or_else(|| ApplicationError::Validation("reseller tenant is missing".into()))?;
        let mut tenant = self
            .repository
            .get_tenant_by_id(tenant_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("tenant not found".into()))?;
        tenant.name = command.tenant_name;
        tenant.updated_at = Utc::now();
        user.email = command.email;
        user.display_name = command.display_name;
        user.status = command.status;
        if let Some(password_hash) = command.password_hash {
            user.password_hash = password_hash;
        }
        user.updated_at = Utc::now();
        let updated = self.repository.update_reseller_bundle(tenant, user).await?;
        self.attach_tenant_name(updated).await
    }

    pub async fn delete_reseller(&self, actor: &Actor, reseller_id: Uuid) -> ApplicationResult<()> {
        self.rbac.authorize(
            actor,
            Permission::ManageResellers,
            AccessScope {
                target_tenant_id: None,
            },
        )?;
        let user = self
            .repository
            .get_user_by_id(reseller_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("reseller not found".into()))?;
        if user.role != UserRole::Reseller {
            return Err(ApplicationError::Validation("account is not a reseller".into()));
        }
        if let Some(tenant) = self.repository.get_tenant_by_owner_user_id(reseller_id).await? {
            return self.repository.delete_tenant(tenant.id).await;
        }
        self.repository.delete_user(reseller_id).await
    }

    async fn attach_tenant_name(&self, mut user: User) -> ApplicationResult<User> {
        user.tenant_name = match user.tenant_id {
            Some(tenant_id) => self
                .repository
                .get_tenant_by_id(tenant_id)
                .await?
                .map(|tenant| tenant.name),
            None => None,
        };
        Ok(user)
    }

    async fn attach_tenant_names(&self, users: Vec<User>) -> ApplicationResult<Vec<User>> {
        let mut enriched = Vec::with_capacity(users.len());
        for user in users {
            enriched.push(self.attach_tenant_name(user).await?);
        }
        Ok(enriched)
    }
}

#[derive(Default)]
pub struct InMemoryUserRepository {
    users: RwLock<HashMap<Uuid, User>>,
    tenants: RwLock<HashMap<Uuid, Tenant>>,
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn create_user(&self, user: User) -> ApplicationResult<User> {
        self.users
            .write()
            .expect("lock")
            .insert(user.id, user.clone());
        Ok(user)
    }

    async fn create_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User> {
        self.tenants
            .write()
            .expect("lock")
            .insert(tenant.id, tenant);
        self.users
            .write()
            .expect("lock")
            .insert(user.id, user.clone());
        Ok(user)
    }

    async fn get_user_by_email(&self, email: &str) -> ApplicationResult<Option<User>> {
        Ok(self
            .users
            .read()
            .expect("lock")
            .values()
            .find(|user| user.email.eq_ignore_ascii_case(email))
            .cloned())
    }

    async fn get_user_by_id(&self, user_id: Uuid) -> ApplicationResult<Option<User>> {
        Ok(self.users.read().expect("lock").get(&user_id).cloned())
    }

    async fn get_tenant_by_id(&self, tenant_id: Uuid) -> ApplicationResult<Option<Tenant>> {
        Ok(self.tenants.read().expect("lock").get(&tenant_id).cloned())
    }

    async fn get_tenant_by_owner_user_id(&self, user_id: Uuid) -> ApplicationResult<Option<Tenant>> {
        Ok(self
            .tenants
            .read()
            .expect("lock")
            .values()
            .find(|tenant| tenant.owner_user_id == user_id)
            .cloned())
    }

    async fn count_superadmins(&self) -> ApplicationResult<i64> {
        Ok(self
            .users
            .read()
            .expect("lock")
            .values()
            .filter(|user| user.role == UserRole::Superadmin)
            .count() as i64)
    }

    async fn list_users_by_tenant(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<User>> {
        let mut users = self
            .users
            .read()
            .expect("lock")
            .values()
            .filter(|user| tenant_id.is_none() || user.tenant_id == tenant_id)
            .cloned()
            .collect::<Vec<_>>();
        users.sort_by(|left, right| left.email.cmp(&right.email));
        Ok(users)
    }

    async fn list_resellers(&self) -> ApplicationResult<Vec<User>> {
        Ok(self
            .users
            .read()
            .expect("lock")
            .values()
            .filter(|user| user.role == UserRole::Reseller)
            .cloned()
            .collect())
    }

    async fn update_user(&self, user: User) -> ApplicationResult<User> {
        self.users
            .write()
            .expect("lock")
            .insert(user.id, user.clone());
        Ok(user)
    }

    async fn update_reseller_bundle(&self, tenant: Tenant, user: User) -> ApplicationResult<User> {
        self.tenants
            .write()
            .expect("lock")
            .insert(tenant.id, tenant);
        self.users
            .write()
            .expect("lock")
            .insert(user.id, user.clone());
        Ok(user)
    }

    async fn delete_user(&self, user_id: Uuid) -> ApplicationResult<()> {
        self.users.write().expect("lock").remove(&user_id);
        Ok(())
    }

    async fn delete_tenant(&self, tenant_id: Uuid) -> ApplicationResult<()> {
        self.tenants.write().expect("lock").remove(&tenant_id);
        self.users
            .write()
            .expect("lock")
            .retain(|_, user| user.tenant_id != Some(tenant_id));
        Ok(())
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> ApplicationResult<()> {
        let mut users = self.users.write().expect("lock");
        let user = users
            .get_mut(&user_id)
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        user.password_hash = password_hash.into();
        user.updated_at = Utc::now();
        Ok(())
    }

    async fn save_totp_secret(&self, user_id: Uuid, secret: &str) -> ApplicationResult<()> {
        let mut users = self.users.write().expect("lock");
        let user = users
            .get_mut(&user_id)
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        user.totp_secret = Some(secret.into());
        user.updated_at = Utc::now();
        Ok(())
    }

    async fn confirm_totp(&self, user_id: Uuid) -> ApplicationResult<()> {
        let mut users = self.users.write().expect("lock");
        let user = users
            .get_mut(&user_id)
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        user.totp_confirmed = true;
        user.updated_at = Utc::now();
        Ok(())
    }

    async fn clear_totp(&self, user_id: Uuid) -> ApplicationResult<()> {
        let mut users = self.users.write().expect("lock");
        let user = users
            .get_mut(&user_id)
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        user.totp_secret = None;
        user.totp_confirmed = false;
        user.updated_at = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anneal_core::{Actor, UserRole};
    use anneal_rbac::RbacService;
    use uuid::Uuid;

    use crate::application::{InMemoryUserRepository, UserService};
    use crate::domain::{CreateResellerCommand, CreateUserCommand};

    #[tokio::test]
    async fn reseller_is_scoped_to_own_tenant() {
        let repository = InMemoryUserRepository::default();
        let service = UserService::new(repository, RbacService);
        let tenant_id = Uuid::new_v4();
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(tenant_id),
            role: UserRole::Reseller,
        };

        let created = service
            .create_user(
                &actor,
                CreateUserCommand {
                    target_tenant_id: Some(Uuid::new_v4()),
                    email: "user@test.local".into(),
                    display_name: "User".into(),
                    role: UserRole::User,
                    password_hash: "hash".into(),
                },
            )
            .await
            .expect("create user");

        assert_eq!(created.tenant_id, Some(tenant_id));
    }

    #[tokio::test]
    async fn admin_can_create_reseller() {
        let repository = InMemoryUserRepository::default();
        let service = UserService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: None,
            role: UserRole::Admin,
        };

        let created = service
            .create_reseller(
                &actor,
                CreateResellerCommand {
                    tenant_name: "North".into(),
                    email: "reseller@test.local".into(),
                    display_name: "Reseller".into(),
                    password_hash: "hash".into(),
                },
            )
            .await
            .expect("create reseller");

        assert_eq!(created.role, UserRole::Reseller);
        assert!(created.tenant_id.is_some());
        assert_eq!(created.tenant_name.as_deref(), Some("North"));
    }
}
