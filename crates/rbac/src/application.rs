use anneal_core::{Actor, ApplicationError, ApplicationResult, UserRole};
use uuid::Uuid;

use crate::domain::Permission;

#[derive(Debug, Clone, Copy)]
pub struct AccessScope {
    pub target_tenant_id: Option<Uuid>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RbacService;

impl RbacService {
    pub fn authorize(
        &self,
        actor: &Actor,
        permission: Permission,
        scope: AccessScope,
    ) -> ApplicationResult<()> {
        if self.is_allowed(actor, permission, scope) {
            Ok(())
        } else {
            Err(ApplicationError::Forbidden)
        }
    }

    pub fn is_allowed(&self, actor: &Actor, permission: Permission, scope: AccessScope) -> bool {
        match actor.role {
            UserRole::Superadmin => true,
            UserRole::Admin => !matches!(permission, Permission::Bootstrap),
            UserRole::Reseller => self.is_reseller_allowed(actor, permission, scope),
            UserRole::User => matches!(permission, Permission::SelfService),
        }
    }

    fn is_reseller_allowed(
        &self,
        actor: &Actor,
        permission: Permission,
        scope: AccessScope,
    ) -> bool {
        let tenant_matches = actor.tenant_id.is_some() && actor.tenant_id == scope.target_tenant_id;
        match permission {
            Permission::ManageTenantUsers
            | Permission::ManageNodes
            | Permission::ManageSubscriptions
            | Permission::ManageUsage
            | Permission::ManageNotifications
            | Permission::SelfService => tenant_matches,
            Permission::ManageResellers
            | Permission::ManageGlobalUsers
            | Permission::ManageAudit
            | Permission::Bootstrap => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use anneal_core::{Actor, UserRole};
    use uuid::Uuid;

    use crate::{
        application::{AccessScope, RbacService},
        domain::Permission,
    };

    #[test]
    fn superadmin_can_manage_any_scope() {
        let tenant_id = Uuid::new_v4();
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: None,
            role: UserRole::Superadmin,
        };

        let allowed = RbacService.is_allowed(
            &actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        );

        assert!(allowed);
    }

    #[test]
    fn reseller_cannot_manage_foreign_tenant() {
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };

        let allowed = RbacService.is_allowed(
            &actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: Some(Uuid::new_v4()),
            },
        );

        assert!(!allowed);
    }

    #[test]
    fn reseller_can_manage_notifications_in_own_tenant() {
        let tenant_id = Uuid::new_v4();
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(tenant_id),
            role: UserRole::Reseller,
        };

        let allowed = RbacService.is_allowed(
            &actor,
            Permission::ManageNotifications,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        );

        assert!(allowed);
    }

    #[test]
    fn user_cannot_manage_notifications() {
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::User,
        };

        let allowed = RbacService.is_allowed(
            &actor,
            Permission::ManageNotifications,
            AccessScope {
                target_tenant_id: actor.tenant_id,
            },
        );

        assert!(!allowed);
    }
}
