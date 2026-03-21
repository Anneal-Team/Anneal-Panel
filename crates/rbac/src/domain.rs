use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Bootstrap,
    ManageGlobalUsers,
    ManageResellers,
    ManageTenantUsers,
    ManageNodes,
    ManageSubscriptions,
    ManageUsage,
    ManageAudit,
    SelfService,
}
