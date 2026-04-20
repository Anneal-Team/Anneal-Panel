use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    sync::RwLock,
};

use anneal_config_engine::SecurityKind;
use anneal_core::{
    Actor, ApplicationError, ApplicationResult, DeploymentStatus, NodeStatus, ProtocolKind,
    ProxyEngine, TokenHasher, UserRole,
};
use anneal_rbac::{AccessScope, Permission, RbacService};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use rand::{RngExt, distr::Alphanumeric};
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::domain::{
    ConfigRevision, DeliveryNodeEndpoint, DeploymentRollout, EnrollmentGrant, NodeBootstrapGrant,
    NodeBootstrapRuntimeGrant, NodeBootstrapSession, NodeCapability, NodeDomain, NodeDomainDraft,
    NodeDomainMode, NodeEndpoint, NodeEndpointDraft, NodeEnrollmentToken, NodeRuntime,
    NodeTokenRotationGrant, RuntimeRegistration, RuntimeRegistrationGrant, ServerNode,
};

#[async_trait]
pub trait NodeRepository: Send + Sync {
    async fn create_server_node(&self, group: ServerNode) -> ApplicationResult<ServerNode>;
    async fn list_server_nodes(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<ServerNode>>;
    async fn find_server_node(&self, server_node_id: Uuid)
    -> ApplicationResult<Option<ServerNode>>;
    async fn update_server_node(&self, group: ServerNode) -> ApplicationResult<ServerNode>;
    async fn delete_server_node(&self, server_node_id: Uuid) -> ApplicationResult<()>;
    async fn list_node_runtimes_for_server(
        &self,
        server_node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeRuntime>>;
    async fn list_node_domains(&self, server_node_id: Uuid) -> ApplicationResult<Vec<NodeDomain>>;
    async fn replace_node_domains(
        &self,
        server_node_id: Uuid,
        domains: &[NodeDomain],
    ) -> ApplicationResult<Vec<NodeDomain>>;
    async fn create_enrollment_token(
        &self,
        record: NodeEnrollmentToken,
    ) -> ApplicationResult<NodeEnrollmentToken>;
    async fn consume_enrollment_token(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeEnrollmentToken>>;
    async fn create_bootstrap_session(
        &self,
        session: NodeBootstrapSession,
    ) -> ApplicationResult<NodeBootstrapSession>;
    async fn consume_bootstrap_session(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeBootstrapSession>>;
    async fn reactivate_bootstrap_session(&self, session_id: Uuid) -> ApplicationResult<()>;
    async fn create_node(
        &self,
        node: NodeRuntime,
        protocols: &[NodeCapability],
    ) -> ApplicationResult<NodeRuntime>;
    async fn delete_node(&self, node_id: Uuid) -> ApplicationResult<()>;
    async fn find_node_by_token_hash(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeRuntime>>;
    async fn find_node(&self, node_id: Uuid) -> ApplicationResult<Option<NodeRuntime>>;
    async fn update_node_token_hash(
        &self,
        node_id: Uuid,
        node_token_hash: &str,
    ) -> ApplicationResult<()>;
    async fn update_node_heartbeat(
        &self,
        node_id: Uuid,
        version: &str,
        status: NodeStatus,
    ) -> ApplicationResult<Option<NodeRuntime>>;
    async fn list_nodes(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<NodeRuntime>>;
    async fn list_node_capabilities(&self, node_id: Uuid)
    -> ApplicationResult<Vec<NodeCapability>>;
    async fn replace_node_endpoints(
        &self,
        node_id: Uuid,
        endpoints: &[NodeEndpoint],
    ) -> ApplicationResult<Vec<NodeEndpoint>>;
    async fn list_node_endpoints(&self, node_id: Uuid) -> ApplicationResult<Vec<NodeEndpoint>>;
    async fn list_delivery_endpoints(
        &self,
        tenant_id: Uuid,
    ) -> ApplicationResult<Vec<DeliveryNodeEndpoint>>;
    async fn create_config_revision(
        &self,
        revision: ConfigRevision,
    ) -> ApplicationResult<ConfigRevision>;
    async fn create_rollout(
        &self,
        rollout: DeploymentRollout,
    ) -> ApplicationResult<DeploymentRollout>;
    async fn find_rollout(&self, rollout_id: Uuid) -> ApplicationResult<Option<DeploymentRollout>>;
    async fn list_rollouts(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<DeploymentRollout>>;
    async fn list_ready_rollouts(
        &self,
        node_id: Uuid,
        limit: i64,
    ) -> ApplicationResult<Vec<DeploymentRollout>>;
    async fn update_rollout_state(
        &self,
        rollout_id: Uuid,
        status: DeploymentStatus,
        failure_reason: Option<String>,
    ) -> ApplicationResult<()>;
}

#[async_trait]
pub trait NodeEndpointCatalog: Send + Sync {
    async fn list_delivery_endpoints(
        &self,
        tenant_id: Uuid,
    ) -> ApplicationResult<Vec<DeliveryNodeEndpoint>>;
}

#[async_trait]
impl<T> NodeEndpointCatalog for T
where
    T: NodeRepository + Send + Sync,
{
    async fn list_delivery_endpoints(
        &self,
        tenant_id: Uuid,
    ) -> ApplicationResult<Vec<DeliveryNodeEndpoint>> {
        NodeRepository::list_delivery_endpoints(self, tenant_id).await
    }
}

#[async_trait]
impl<T> NodeRepository for &T
where
    T: NodeRepository + Send + Sync,
{
    async fn create_server_node(&self, group: ServerNode) -> ApplicationResult<ServerNode> {
        (*self).create_server_node(group).await
    }

    async fn list_server_nodes(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<ServerNode>> {
        (*self).list_server_nodes(tenant_id).await
    }

    async fn find_server_node(
        &self,
        server_node_id: Uuid,
    ) -> ApplicationResult<Option<ServerNode>> {
        (*self).find_server_node(server_node_id).await
    }

    async fn update_server_node(&self, group: ServerNode) -> ApplicationResult<ServerNode> {
        (*self).update_server_node(group).await
    }

    async fn delete_server_node(&self, server_node_id: Uuid) -> ApplicationResult<()> {
        (*self).delete_server_node(server_node_id).await
    }

    async fn list_node_runtimes_for_server(
        &self,
        server_node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeRuntime>> {
        (*self).list_node_runtimes_for_server(server_node_id).await
    }

    async fn list_node_domains(&self, server_node_id: Uuid) -> ApplicationResult<Vec<NodeDomain>> {
        (*self).list_node_domains(server_node_id).await
    }

    async fn replace_node_domains(
        &self,
        server_node_id: Uuid,
        domains: &[NodeDomain],
    ) -> ApplicationResult<Vec<NodeDomain>> {
        (*self).replace_node_domains(server_node_id, domains).await
    }

    async fn create_enrollment_token(
        &self,
        record: NodeEnrollmentToken,
    ) -> ApplicationResult<NodeEnrollmentToken> {
        (*self).create_enrollment_token(record).await
    }

    async fn consume_enrollment_token(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeEnrollmentToken>> {
        (*self).consume_enrollment_token(token_hash).await
    }

    async fn create_bootstrap_session(
        &self,
        session: NodeBootstrapSession,
    ) -> ApplicationResult<NodeBootstrapSession> {
        (*self).create_bootstrap_session(session).await
    }

    async fn consume_bootstrap_session(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeBootstrapSession>> {
        (*self).consume_bootstrap_session(token_hash).await
    }

    async fn reactivate_bootstrap_session(&self, session_id: Uuid) -> ApplicationResult<()> {
        (*self).reactivate_bootstrap_session(session_id).await
    }

    async fn create_node(
        &self,
        node: NodeRuntime,
        protocols: &[NodeCapability],
    ) -> ApplicationResult<NodeRuntime> {
        (*self).create_node(node, protocols).await
    }

    async fn delete_node(&self, node_id: Uuid) -> ApplicationResult<()> {
        (*self).delete_node(node_id).await
    }

    async fn find_node_by_token_hash(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeRuntime>> {
        (*self).find_node_by_token_hash(token_hash).await
    }

    async fn find_node(&self, node_id: Uuid) -> ApplicationResult<Option<NodeRuntime>> {
        (*self).find_node(node_id).await
    }

    async fn update_node_token_hash(
        &self,
        node_id: Uuid,
        node_token_hash: &str,
    ) -> ApplicationResult<()> {
        (*self)
            .update_node_token_hash(node_id, node_token_hash)
            .await
    }

    async fn update_node_heartbeat(
        &self,
        node_id: Uuid,
        version: &str,
        status: NodeStatus,
    ) -> ApplicationResult<Option<NodeRuntime>> {
        (*self)
            .update_node_heartbeat(node_id, version, status)
            .await
    }

    async fn list_nodes(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<NodeRuntime>> {
        (*self).list_nodes(tenant_id).await
    }

    async fn list_node_capabilities(
        &self,
        node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeCapability>> {
        (*self).list_node_capabilities(node_id).await
    }

    async fn replace_node_endpoints(
        &self,
        node_id: Uuid,
        endpoints: &[NodeEndpoint],
    ) -> ApplicationResult<Vec<NodeEndpoint>> {
        (*self).replace_node_endpoints(node_id, endpoints).await
    }

    async fn list_node_endpoints(&self, node_id: Uuid) -> ApplicationResult<Vec<NodeEndpoint>> {
        (*self).list_node_endpoints(node_id).await
    }

    async fn list_delivery_endpoints(
        &self,
        tenant_id: Uuid,
    ) -> ApplicationResult<Vec<DeliveryNodeEndpoint>> {
        (*self).list_delivery_endpoints(tenant_id).await
    }

    async fn create_config_revision(
        &self,
        revision: ConfigRevision,
    ) -> ApplicationResult<ConfigRevision> {
        (*self).create_config_revision(revision).await
    }

    async fn create_rollout(
        &self,
        rollout: DeploymentRollout,
    ) -> ApplicationResult<DeploymentRollout> {
        (*self).create_rollout(rollout).await
    }

    async fn find_rollout(&self, rollout_id: Uuid) -> ApplicationResult<Option<DeploymentRollout>> {
        (*self).find_rollout(rollout_id).await
    }

    async fn list_rollouts(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<DeploymentRollout>> {
        (*self).list_rollouts(tenant_id).await
    }

    async fn list_ready_rollouts(
        &self,
        node_id: Uuid,
        limit: i64,
    ) -> ApplicationResult<Vec<DeploymentRollout>> {
        (*self).list_ready_rollouts(node_id, limit).await
    }

    async fn update_rollout_state(
        &self,
        rollout_id: Uuid,
        status: DeploymentStatus,
        failure_reason: Option<String>,
    ) -> ApplicationResult<()> {
        (*self)
            .update_rollout_state(rollout_id, status, failure_reason)
            .await
    }
}

pub struct NodeService<R> {
    repository: R,
    rbac: RbacService,
    token_hasher: TokenHasher,
    default_node_domain: Option<String>,
}

impl<R> NodeService<R> {
    pub fn new(repository: R, rbac: RbacService) -> Self {
        Self::with_token_hasher(
            repository,
            rbac,
            TokenHasher::new("anneal-default-token-hash-key").expect("token hasher"),
        )
    }

    pub fn with_token_hasher(repository: R, rbac: RbacService, token_hasher: TokenHasher) -> Self {
        Self::with_default_node_domain(repository, rbac, token_hasher, None)
    }

    pub fn with_public_base_url(
        repository: R,
        rbac: RbacService,
        token_hasher: TokenHasher,
        public_base_url: &str,
    ) -> Self {
        Self::with_default_node_domain(
            repository,
            rbac,
            token_hasher,
            default_node_domain_from_public_base_url(public_base_url),
        )
    }

    fn with_default_node_domain(
        repository: R,
        rbac: RbacService,
        token_hasher: TokenHasher,
        default_node_domain: Option<String>,
    ) -> Self {
        Self {
            repository,
            rbac,
            token_hasher,
            default_node_domain,
        }
    }
}

impl<R> NodeService<R>
where
    R: NodeRepository,
{
    async fn issue_enrollment_grant(
        &self,
        tenant_id: Uuid,
        server_node_id: Uuid,
        engine: ProxyEngine,
        ttl: Duration,
    ) -> ApplicationResult<EnrollmentGrant> {
        let token = generate_token();
        let now = Utc::now();
        let record = NodeEnrollmentToken {
            id: Uuid::new_v4(),
            tenant_id,
            server_node_id,
            token_hash: self.token_hasher.hash(&token),
            engine,
            expires_at: now + ttl,
            created_at: now,
            used_at: None,
        };
        let record = self.repository.create_enrollment_token(record).await?;
        Ok(EnrollmentGrant { token, record })
    }

    pub async fn create_bootstrap_token(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        server_node_id: Uuid,
        node_name: String,
        engines: Vec<ProxyEngine>,
    ) -> ApplicationResult<NodeBootstrapGrant> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let group = self
            .repository
            .find_server_node(server_node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node group not found".into()))?;
        if group.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        let engines = deduplicate_engines(engines);
        if engines.is_empty() {
            return Err(ApplicationError::Validation(
                "at least one engine is required".into(),
            ));
        }
        let node_name = node_name.trim().to_owned();
        if node_name.is_empty() {
            return Err(ApplicationError::Validation("node name is required".into()));
        }
        let now = Utc::now();
        let expires_at = now + Duration::minutes(15);
        let bootstrap_token = generate_token();
        self.repository
            .create_bootstrap_session(NodeBootstrapSession {
                id: Uuid::new_v4(),
                tenant_id,
                server_node_id,
                node_name: node_name.clone(),
                engines: engines.clone(),
                token_hash: self.token_hasher.hash(&bootstrap_token),
                expires_at,
                created_at: now,
                used_at: None,
            })
            .await?;
        Ok(NodeBootstrapGrant {
            bootstrap_token,
            tenant_id,
            node_id: server_node_id,
            node_name,
            engines,
            expires_at,
        })
    }

    pub async fn create_server_node(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        name: String,
    ) -> ApplicationResult<ServerNode> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let now = Utc::now();
        self.repository
            .create_server_node(ServerNode {
                id: Uuid::new_v4(),
                tenant_id,
                name,
                created_at: now,
                updated_at: now,
            })
            .await
    }

    pub async fn update_server_node(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        server_node_id: Uuid,
        name: String,
    ) -> ApplicationResult<ServerNode> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let mut group = self
            .repository
            .find_server_node(server_node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node group not found".into()))?;
        if group.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        group.name = name.trim().to_owned();
        if group.name.is_empty() {
            return Err(ApplicationError::Validation(
                "node group name is required".into(),
            ));
        }
        group.updated_at = Utc::now();
        self.repository.update_server_node(group).await
    }

    pub async fn delete_server_node(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        server_node_id: Uuid,
    ) -> ApplicationResult<()> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let group = self
            .repository
            .find_server_node(server_node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node group not found".into()))?;
        if group.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        self.repository.delete_server_node(server_node_id).await
    }

    pub async fn create_enrollment_token(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        server_node_id: Uuid,
        engine: ProxyEngine,
    ) -> ApplicationResult<EnrollmentGrant> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let group = self
            .repository
            .find_server_node(server_node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node group not found".into()))?;
        if group.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        self.issue_enrollment_grant(tenant_id, server_node_id, engine, Duration::hours(12))
            .await
    }

    pub async fn register_node(
        &self,
        token: &str,
        registration: RuntimeRegistration,
    ) -> ApplicationResult<RuntimeRegistrationGrant> {
        let record = self
            .repository
            .consume_enrollment_token(&self.token_hasher.hash(token))
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if record.used_at.is_some() || record.expires_at <= Utc::now() {
            return Err(ApplicationError::Unauthorized);
        }
        if registration.engine != record.engine {
            return Err(ApplicationError::Validation(
                "registration engine does not match enrollment token engine".into(),
            ));
        }
        validate_registered_protocols(registration.engine, &registration.protocols)?;
        let now = Utc::now();
        let node_token = generate_token();
        let node = NodeRuntime {
            id: Uuid::new_v4(),
            tenant_id: record.tenant_id,
            server_node_id: record.server_node_id,
            name: registration.name,
            engine: registration.engine,
            version: registration.version,
            status: NodeStatus::Online,
            last_seen_at: Some(now),
            node_token_hash: self.token_hasher.hash(&node_token),
            created_at: now,
            updated_at: now,
        };
        let protocols = registration
            .protocols
            .into_iter()
            .map(|protocol| NodeCapability {
                node_id: node.id,
                protocol,
            })
            .collect::<Vec<_>>();
        let node = self.repository.create_node(node, &protocols).await?;
        if let Err(error) = self.sync_server_node_endpoints(record.server_node_id).await {
            self.repository.delete_node(node.id).await?;
            return Err(error);
        }
        Ok(RuntimeRegistrationGrant {
            runtime: node,
            node_token,
        })
    }

    pub async fn bootstrap_nodes(
        &self,
        token: &str,
        registrations: Vec<RuntimeRegistration>,
    ) -> ApplicationResult<Vec<NodeBootstrapRuntimeGrant>> {
        let session = self
            .repository
            .consume_bootstrap_session(&self.token_hasher.hash(token))
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if session.used_at.is_some() || session.expires_at <= Utc::now() {
            return Err(ApplicationError::Unauthorized);
        }
        let registrations = registrations
            .into_iter()
            .map(|registration| (registration.engine, registration))
            .collect::<HashMap<_, _>>();
        let mut created_node_ids = Vec::with_capacity(session.engines.len());
        let result = async {
            let mut grants = Vec::with_capacity(session.engines.len());
            for engine in &session.engines {
                let mut registration = registrations.get(engine).cloned().ok_or_else(|| {
                    ApplicationError::Validation(format!(
                        "missing bootstrap registration for {}",
                        engine_name(*engine)
                    ))
                })?;
                registration.name =
                    bootstrap_node_name(&session.node_name, *engine, session.engines.len());
                let grant = self
                    .register_bootstrap_runtime(
                        session.tenant_id,
                        session.server_node_id,
                        registration,
                    )
                    .await?;
                created_node_ids.push(grant.runtime.id);
                grants.push(NodeBootstrapRuntimeGrant {
                    engine: *engine,
                    node_id: grant.runtime.id,
                    node_token: grant.node_token,
                });
            }
            Ok::<Vec<NodeBootstrapRuntimeGrant>, ApplicationError>(grants)
        }
        .await;
        if let Err(error) = result {
            for node_id in created_node_ids.into_iter().rev() {
                self.repository.delete_node(node_id).await?;
            }
            self.repository
                .reactivate_bootstrap_session(session.id)
                .await?;
            return Err(error);
        }
        result
    }

    pub async fn heartbeat(
        &self,
        node_id: Uuid,
        node_token: &str,
        version: &str,
    ) -> ApplicationResult<NodeRuntime> {
        let node = self.authenticate_node(node_id, node_token).await?;
        self.repository
            .update_node_heartbeat(node.id, version, NodeStatus::Online)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node not found".into()))
    }

    pub async fn rotate_node_token(
        &self,
        node_id: Uuid,
        current_node_token: &str,
    ) -> ApplicationResult<NodeTokenRotationGrant> {
        let node = self.authenticate_node(node_id, current_node_token).await?;
        let node_token = generate_token();
        self.repository
            .update_node_token_hash(node.id, &self.token_hasher.hash(&node_token))
            .await?;
        Ok(NodeTokenRotationGrant {
            node_id: node.id,
            node_token,
        })
    }

    pub async fn reissue_bootstrap_for_node(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        node_id: Uuid,
    ) -> ApplicationResult<NodeBootstrapGrant> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let node = self
            .repository
            .find_node(node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node not found".into()))?;
        if node.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        self.repository
            .update_node_token_hash(node.id, &self.token_hasher.hash(&generate_token()))
            .await?;
        self.create_bootstrap_token(
            actor,
            tenant_id,
            node.server_node_id,
            node.name.clone(),
            vec![node.engine],
        )
        .await
    }

    pub async fn list_nodes(&self, actor: &Actor) -> ApplicationResult<Vec<NodeRuntime>> {
        let tenant_id = if actor.role == UserRole::Reseller {
            actor.tenant_id
        } else {
            None
        };
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        self.repository.list_nodes(tenant_id).await
    }

    pub async fn list_server_nodes(&self, actor: &Actor) -> ApplicationResult<Vec<ServerNode>> {
        let tenant_id = if actor.role == UserRole::Reseller {
            actor.tenant_id
        } else {
            None
        };
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        self.repository.list_server_nodes(tenant_id).await
    }

    pub async fn list_node_domains(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        server_node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeDomain>> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let group = self
            .repository
            .find_server_node(server_node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node group not found".into()))?;
        if group.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        self.repository.list_node_domains(server_node_id).await
    }

    pub async fn replace_node_domains(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        server_node_id: Uuid,
        drafts: Vec<NodeDomainDraft>,
    ) -> ApplicationResult<Vec<NodeDomain>> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let group = self
            .repository
            .find_server_node(server_node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node group not found".into()))?;
        if group.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        let now = Utc::now();
        let domains = normalize_node_group_domains(server_node_id, drafts, now)?;
        let domains = self
            .repository
            .replace_node_domains(server_node_id, &domains)
            .await?;
        self.sync_server_node_endpoints(server_node_id).await?;
        Ok(domains)
    }

    pub async fn replace_node_endpoints(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        node_id: Uuid,
        drafts: Vec<NodeEndpointDraft>,
    ) -> ApplicationResult<Vec<NodeEndpoint>> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let node = self
            .repository
            .find_node(node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node not found".into()))?;
        if node.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        let existing_endpoints = self.repository.list_node_endpoints(node_id).await?;
        let capabilities = self.repository.list_node_capabilities(node_id).await?;
        let drafts = normalize_endpoint_drafts(drafts);
        validate_endpoint_drafts(node.engine, &capabilities, &drafts)?;
        let now = Utc::now();
        let mut endpoints = drafts
            .into_iter()
            .map(|draft| NodeEndpoint {
                id: Uuid::new_v4(),
                node_id,
                protocol: draft.protocol,
                listen_host: draft.listen_host,
                listen_port: i32::from(draft.listen_port),
                public_host: draft.public_host,
                public_port: i32::from(draft.public_port),
                transport: draft.transport,
                security: draft.security,
                server_name: draft.server_name,
                host_header: draft.host_header,
                path: draft.path,
                service_name: draft.service_name,
                flow: draft.flow,
                reality_public_key: draft.reality_public_key,
                reality_private_key: draft.reality_private_key,
                reality_short_id: draft.reality_short_id,
                fingerprint: draft.fingerprint,
                alpn: draft.alpn,
                cipher: draft.cipher,
                tls_certificate_path: draft.tls_certificate_path,
                tls_key_path: draft.tls_key_path,
                enabled: draft.enabled,
                created_at: now,
                updated_at: now,
            })
            .collect::<Vec<_>>();
        reconcile_manual_endpoints(&existing_endpoints, &mut endpoints);
        self.repository
            .replace_node_endpoints(node_id, &endpoints)
            .await
    }

    pub async fn list_node_endpoints(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeEndpoint>> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let node = self
            .repository
            .find_node(node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node not found".into()))?;
        if node.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        self.repository.list_node_endpoints(node_id).await
    }

    pub async fn queue_rollout(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        node_id: Uuid,
        revision_name: String,
        rendered_config: String,
        target_path: String,
    ) -> ApplicationResult<DeploymentRollout> {
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let node = self
            .repository
            .find_node(node_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("node not found".into()))?;
        if node.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        let now = Utc::now();
        let revision = self
            .repository
            .create_config_revision(ConfigRevision {
                id: Uuid::new_v4(),
                tenant_id,
                node_id: Some(node_id),
                name: revision_name.clone(),
                engine: node.engine,
                rendered_config: rendered_config.clone(),
                created_by: Some(actor.user_id),
                created_at: now,
            })
            .await?;
        self.repository
            .create_rollout(DeploymentRollout {
                id: Uuid::new_v4(),
                tenant_id,
                node_id,
                config_revision_id: revision.id,
                engine: node.engine,
                revision_name,
                rendered_config,
                target_path,
                status: DeploymentStatus::Queued,
                failure_reason: None,
                created_at: now,
                updated_at: now,
                applied_at: None,
            })
            .await
    }

    pub async fn list_rollouts(&self, actor: &Actor) -> ApplicationResult<Vec<DeploymentRollout>> {
        let tenant_id = if actor.role == UserRole::Reseller {
            actor.tenant_id
        } else {
            None
        };
        self.rbac.authorize(
            actor,
            Permission::ManageNodes,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        self.repository.list_rollouts(tenant_id).await
    }

    pub async fn pull_rollouts(
        &self,
        node_id: Uuid,
        node_token: &str,
        limit: i64,
    ) -> ApplicationResult<Vec<DeploymentRollout>> {
        let node = self.authenticate_node(node_id, node_token).await?;
        self.repository.list_ready_rollouts(node.id, limit).await
    }

    pub async fn acknowledge_rollout(
        &self,
        node_id: Uuid,
        node_token: &str,
        rollout_id: Uuid,
        success: bool,
        failure_reason: Option<String>,
    ) -> ApplicationResult<DeploymentRollout> {
        let node = self.authenticate_node(node_id, node_token).await?;
        let mut rollout = self
            .repository
            .find_rollout(rollout_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("rollout not found".into()))?;
        if rollout.node_id != node.id {
            return Err(ApplicationError::Forbidden);
        }
        let rollout_failure_reason = failure_reason.clone();
        let applied_at = if success { Some(Utc::now()) } else { None };
        let status = if success {
            DeploymentStatus::Applied
        } else {
            DeploymentStatus::Failed
        };
        self.repository
            .update_rollout_state(rollout_id, status, failure_reason)
            .await?;
        rollout.status = status;
        rollout.failure_reason = if success {
            None
        } else {
            rollout_failure_reason
        };
        rollout.updated_at = Utc::now();
        rollout.applied_at = applied_at;
        Ok(rollout)
    }

    async fn register_bootstrap_runtime(
        &self,
        tenant_id: Uuid,
        server_node_id: Uuid,
        registration: RuntimeRegistration,
    ) -> ApplicationResult<RuntimeRegistrationGrant> {
        validate_registered_protocols(registration.engine, &registration.protocols)?;
        self.prepare_bootstrap_runtime_slot(tenant_id, server_node_id, &registration.name)
            .await?;
        let now = Utc::now();
        let node_token = generate_token();
        let node = NodeRuntime {
            id: Uuid::new_v4(),
            tenant_id,
            server_node_id,
            name: registration.name,
            engine: registration.engine,
            version: registration.version,
            status: NodeStatus::Online,
            last_seen_at: Some(now),
            node_token_hash: self.token_hasher.hash(&node_token),
            created_at: now,
            updated_at: now,
        };
        let protocols = registration
            .protocols
            .into_iter()
            .map(|protocol| NodeCapability {
                node_id: node.id,
                protocol,
            })
            .collect::<Vec<_>>();
        let node = self.repository.create_node(node, &protocols).await?;
        if let Err(error) = self.sync_server_node_endpoints(server_node_id).await {
            self.repository.delete_node(node.id).await?;
            return Err(error);
        }
        Ok(RuntimeRegistrationGrant {
            runtime: node,
            node_token,
        })
    }

    pub fn resolve_status(
        last_seen_at: chrono::DateTime<Utc>,
        now: chrono::DateTime<Utc>,
    ) -> NodeStatus {
        let age = now.signed_duration_since(last_seen_at);
        if age < Duration::seconds(90) {
            NodeStatus::Online
        } else {
            NodeStatus::Offline
        }
    }

    async fn authenticate_node(
        &self,
        node_id: Uuid,
        node_token: &str,
    ) -> ApplicationResult<NodeRuntime> {
        let node_token = node_token.trim();
        if node_token.is_empty() {
            return Err(ApplicationError::Unauthorized);
        }
        let node = self
            .repository
            .find_node_by_token_hash(&self.token_hasher.hash(node_token))
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if node.id != node_id {
            return Err(ApplicationError::Forbidden);
        }
        Ok(node)
    }

    async fn load_or_seed_node_domains(
        &self,
        server_node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeDomain>> {
        let existing = self.repository.list_node_domains(server_node_id).await?;
        if !existing.is_empty() {
            return Ok(existing);
        }
        let Some(default_domain) = self.default_node_domain.as_deref() else {
            return Ok(existing);
        };
        let domains = normalize_node_group_domains(
            server_node_id,
            default_node_domain_drafts(default_domain),
            Utc::now(),
        )?;
        self.repository
            .replace_node_domains(server_node_id, &domains)
            .await
    }

    async fn sync_server_node_endpoints(&self, server_node_id: Uuid) -> ApplicationResult<()> {
        let nodes = self
            .repository
            .list_node_runtimes_for_server(server_node_id)
            .await?;
        if nodes.is_empty() {
            return Ok(());
        }
        let domains = self.load_or_seed_node_domains(server_node_id).await?;
        let mut capabilities_by_node = HashMap::new();
        let mut existing_endpoints_by_node = HashMap::new();
        for node in &nodes {
            capabilities_by_node.insert(
                node.id,
                self.repository.list_node_capabilities(node.id).await?,
            );
            existing_endpoints_by_node
                .insert(node.id, self.repository.list_node_endpoints(node.id).await?);
        }
        let mut resolved_domains = Vec::with_capacity(domains.len());
        for domain in domains {
            let public_hosts = resolve_public_hosts(&domain).await;
            resolved_domains.push(ResolvedNodeDomain {
                domain,
                public_hosts,
            });
        }
        let generated =
            build_group_generated_endpoints(&nodes, &capabilities_by_node, &resolved_domains)?;
        for node in nodes {
            let mut endpoints = generated.get(&node.id).cloned().unwrap_or_default();
            reconcile_generated_endpoints(
                existing_endpoints_by_node
                    .get(&node.id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                &mut endpoints,
            );
            self.repository
                .replace_node_endpoints(node.id, &endpoints)
                .await?;
        }
        Ok(())
    }

    async fn prepare_bootstrap_runtime_slot(
        &self,
        tenant_id: Uuid,
        server_node_id: Uuid,
        name: &str,
    ) -> ApplicationResult<()> {
        let existing = self
            .repository
            .list_nodes(Some(tenant_id))
            .await?
            .into_iter()
            .find(|node| node.name == name);
        let Some(existing) = existing else {
            return Ok(());
        };
        if existing.server_node_id == server_node_id
            && existing.status == NodeStatus::Pending
            && existing.last_seen_at.is_none()
        {
            self.repository.delete_node(existing.id).await?;
            return Ok(());
        }
        Err(ApplicationError::Conflict(format!(
            "node runtime {name} already exists"
        )))
    }
}

#[derive(Default)]
pub struct InMemoryNodeRepository {
    groups: RwLock<HashMap<Uuid, ServerNode>>,
    domains: RwLock<HashMap<Uuid, Vec<NodeDomain>>>,
    tokens: RwLock<HashMap<Uuid, NodeEnrollmentToken>>,
    bootstrap_sessions: RwLock<HashMap<Uuid, NodeBootstrapSession>>,
    nodes: RwLock<HashMap<Uuid, NodeRuntime>>,
    capabilities: RwLock<HashMap<Uuid, Vec<NodeCapability>>>,
    endpoints: RwLock<HashMap<Uuid, Vec<NodeEndpoint>>>,
    revisions: RwLock<HashMap<Uuid, ConfigRevision>>,
    rollouts: RwLock<HashMap<Uuid, DeploymentRollout>>,
    create_node_attempts: RwLock<usize>,
    fail_create_node_on_attempt: RwLock<Option<usize>>,
    replace_node_endpoints_attempts: RwLock<usize>,
    fail_replace_node_endpoints_on_attempt: RwLock<Option<usize>>,
}

impl InMemoryNodeRepository {
    #[cfg(test)]
    fn fail_create_node_on_attempt(&self, attempt: usize) {
        *self.fail_create_node_on_attempt.write().expect("lock") = Some(attempt);
    }

    #[cfg(test)]
    fn clear_create_node_failure(&self) {
        *self.fail_create_node_on_attempt.write().expect("lock") = None;
    }

    #[cfg(test)]
    fn fail_replace_node_endpoints_on_attempt(&self, attempt: usize) {
        *self
            .fail_replace_node_endpoints_on_attempt
            .write()
            .expect("lock") = Some(attempt);
    }

    #[cfg(test)]
    fn clear_replace_node_endpoints_failure(&self) {
        *self
            .fail_replace_node_endpoints_on_attempt
            .write()
            .expect("lock") = None;
    }
}

#[async_trait]
impl NodeRepository for InMemoryNodeRepository {
    async fn create_server_node(&self, group: ServerNode) -> ApplicationResult<ServerNode> {
        self.groups
            .write()
            .expect("lock")
            .insert(group.id, group.clone());
        Ok(group)
    }

    async fn list_server_nodes(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<ServerNode>> {
        Ok(self
            .groups
            .read()
            .expect("lock")
            .values()
            .filter(|group| tenant_id.is_none() || Some(group.tenant_id) == tenant_id)
            .cloned()
            .collect())
    }

    async fn find_server_node(
        &self,
        server_node_id: Uuid,
    ) -> ApplicationResult<Option<ServerNode>> {
        Ok(self
            .groups
            .read()
            .expect("lock")
            .get(&server_node_id)
            .cloned())
    }

    async fn update_server_node(&self, group: ServerNode) -> ApplicationResult<ServerNode> {
        self.groups
            .write()
            .expect("lock")
            .insert(group.id, group.clone());
        Ok(group)
    }

    async fn delete_server_node(&self, server_node_id: Uuid) -> ApplicationResult<()> {
        self.groups.write().expect("lock").remove(&server_node_id);
        self.domains.write().expect("lock").remove(&server_node_id);
        let node_ids = self
            .nodes
            .read()
            .expect("lock")
            .values()
            .filter(|node| node.server_node_id == server_node_id)
            .map(|node| node.id)
            .collect::<Vec<_>>();
        self.nodes
            .write()
            .expect("lock")
            .retain(|_, node| node.server_node_id != server_node_id);
        self.capabilities
            .write()
            .expect("lock")
            .retain(|node_id, _| !node_ids.contains(node_id));
        self.endpoints
            .write()
            .expect("lock")
            .retain(|node_id, _| !node_ids.contains(node_id));
        self.tokens
            .write()
            .expect("lock")
            .retain(|_, token| token.server_node_id != server_node_id);
        self.rollouts
            .write()
            .expect("lock")
            .retain(|_, rollout| !node_ids.contains(&rollout.node_id));
        Ok(())
    }

    async fn list_node_runtimes_for_server(
        &self,
        server_node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeRuntime>> {
        Ok(self
            .nodes
            .read()
            .expect("lock")
            .values()
            .filter(|node| node.server_node_id == server_node_id)
            .cloned()
            .collect())
    }

    async fn list_node_domains(&self, server_node_id: Uuid) -> ApplicationResult<Vec<NodeDomain>> {
        Ok(self
            .domains
            .read()
            .expect("lock")
            .get(&server_node_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn replace_node_domains(
        &self,
        server_node_id: Uuid,
        domains: &[NodeDomain],
    ) -> ApplicationResult<Vec<NodeDomain>> {
        self.domains
            .write()
            .expect("lock")
            .insert(server_node_id, domains.to_vec());
        Ok(domains.to_vec())
    }

    async fn create_enrollment_token(
        &self,
        record: NodeEnrollmentToken,
    ) -> ApplicationResult<NodeEnrollmentToken> {
        self.tokens
            .write()
            .expect("lock")
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn consume_enrollment_token(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeEnrollmentToken>> {
        let mut tokens = self.tokens.write().expect("lock");
        let found = tokens
            .values_mut()
            .find(|record| record.token_hash == token_hash && record.used_at.is_none())
            .map(|record| {
                let snapshot = record.clone();
                record.used_at = Some(Utc::now());
                snapshot
            });
        Ok(found)
    }

    async fn create_bootstrap_session(
        &self,
        session: NodeBootstrapSession,
    ) -> ApplicationResult<NodeBootstrapSession> {
        self.bootstrap_sessions
            .write()
            .expect("lock")
            .insert(session.id, session.clone());
        Ok(session)
    }

    async fn consume_bootstrap_session(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeBootstrapSession>> {
        let mut sessions = self.bootstrap_sessions.write().expect("lock");
        let found = sessions
            .values_mut()
            .find(|session| session.token_hash == token_hash && session.used_at.is_none())
            .map(|session| {
                let snapshot = session.clone();
                session.used_at = Some(Utc::now());
                snapshot
            });
        Ok(found)
    }

    async fn reactivate_bootstrap_session(&self, session_id: Uuid) -> ApplicationResult<()> {
        if let Some(session) = self
            .bootstrap_sessions
            .write()
            .expect("lock")
            .get_mut(&session_id)
        {
            session.used_at = None;
        }
        Ok(())
    }

    async fn create_node(
        &self,
        node: NodeRuntime,
        protocols: &[NodeCapability],
    ) -> ApplicationResult<NodeRuntime> {
        let attempt = {
            let mut attempts = self.create_node_attempts.write().expect("lock");
            *attempts += 1;
            *attempts
        };
        if self
            .fail_create_node_on_attempt
            .read()
            .expect("lock")
            .is_some_and(|configured| configured == attempt)
        {
            return Err(ApplicationError::Infrastructure(
                "simulated create_node failure".into(),
            ));
        }
        if self
            .nodes
            .read()
            .expect("lock")
            .values()
            .any(|existing| existing.tenant_id == node.tenant_id && existing.name == node.name)
        {
            return Err(ApplicationError::Conflict(format!(
                "node runtime {} already exists",
                node.name
            )));
        }
        self.nodes
            .write()
            .expect("lock")
            .insert(node.id, node.clone());
        self.capabilities
            .write()
            .expect("lock")
            .insert(node.id, protocols.to_vec());
        Ok(node)
    }

    async fn delete_node(&self, node_id: Uuid) -> ApplicationResult<()> {
        self.nodes.write().expect("lock").remove(&node_id);
        self.capabilities.write().expect("lock").remove(&node_id);
        self.endpoints.write().expect("lock").remove(&node_id);
        self.rollouts
            .write()
            .expect("lock")
            .retain(|_, rollout| rollout.node_id != node_id);
        self.revisions
            .write()
            .expect("lock")
            .retain(|_, revision| revision.node_id != Some(node_id));
        Ok(())
    }

    async fn find_node_by_token_hash(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeRuntime>> {
        Ok(self
            .nodes
            .read()
            .expect("lock")
            .values()
            .find(|node| node.node_token_hash == token_hash)
            .cloned())
    }

    async fn find_node(&self, node_id: Uuid) -> ApplicationResult<Option<NodeRuntime>> {
        Ok(self.nodes.read().expect("lock").get(&node_id).cloned())
    }

    async fn update_node_token_hash(
        &self,
        node_id: Uuid,
        node_token_hash: &str,
    ) -> ApplicationResult<()> {
        let mut nodes = self.nodes.write().expect("lock");
        let node = nodes
            .get_mut(&node_id)
            .ok_or_else(|| ApplicationError::NotFound("node not found".into()))?;
        node.node_token_hash = node_token_hash.into();
        node.updated_at = Utc::now();
        Ok(())
    }

    async fn update_node_heartbeat(
        &self,
        node_id: Uuid,
        version: &str,
        status: NodeStatus,
    ) -> ApplicationResult<Option<NodeRuntime>> {
        let mut nodes = self.nodes.write().expect("lock");
        let updated = nodes.get_mut(&node_id).map(|node| {
            node.version = version.into();
            node.status = status;
            node.last_seen_at = Some(Utc::now());
            node.updated_at = Utc::now();
            node.clone()
        });
        Ok(updated)
    }

    async fn list_nodes(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<NodeRuntime>> {
        Ok(self
            .nodes
            .read()
            .expect("lock")
            .values()
            .filter(|node| tenant_id.is_none() || Some(node.tenant_id) == tenant_id)
            .cloned()
            .collect())
    }

    async fn list_node_capabilities(
        &self,
        node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeCapability>> {
        Ok(self
            .capabilities
            .read()
            .expect("lock")
            .get(&node_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn replace_node_endpoints(
        &self,
        node_id: Uuid,
        endpoints: &[NodeEndpoint],
    ) -> ApplicationResult<Vec<NodeEndpoint>> {
        let attempt = {
            let mut attempts = self.replace_node_endpoints_attempts.write().expect("lock");
            *attempts += 1;
            *attempts
        };
        if self
            .fail_replace_node_endpoints_on_attempt
            .read()
            .expect("lock")
            .is_some_and(|configured| configured == attempt)
        {
            return Err(ApplicationError::Infrastructure(
                "simulated replace_node_endpoints failure".into(),
            ));
        }
        let mut endpoint_ids = BTreeSet::new();
        if endpoints
            .iter()
            .any(|endpoint| !endpoint_ids.insert(endpoint.id))
        {
            return Err(ApplicationError::Infrastructure(
                "duplicate endpoint id in replace_node_endpoints".into(),
            ));
        }
        self.endpoints
            .write()
            .expect("lock")
            .insert(node_id, endpoints.to_vec());
        Ok(endpoints.to_vec())
    }

    async fn list_node_endpoints(&self, node_id: Uuid) -> ApplicationResult<Vec<NodeEndpoint>> {
        Ok(self
            .endpoints
            .read()
            .expect("lock")
            .get(&node_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn list_delivery_endpoints(
        &self,
        tenant_id: Uuid,
    ) -> ApplicationResult<Vec<DeliveryNodeEndpoint>> {
        let nodes = self.nodes.read().expect("lock");
        let endpoints = self.endpoints.read().expect("lock");
        let mut result = Vec::new();
        for node in nodes
            .values()
            .filter(|node| node.tenant_id == tenant_id && node.status == NodeStatus::Online)
        {
            if let Some(items) = endpoints.get(&node.id) {
                result.extend(items.iter().filter(|item| item.enabled).map(|item| {
                    DeliveryNodeEndpoint {
                        node_id: node.id,
                        node_name: node.name.clone(),
                        engine: node.engine,
                        protocol: item.protocol,
                        listen_host: item.listen_host.clone(),
                        listen_port: item.listen_port,
                        public_host: item.public_host.clone(),
                        public_port: item.public_port,
                        transport: item.transport,
                        security: item.security,
                        server_name: item.server_name.clone(),
                        host_header: item.host_header.clone(),
                        path: item.path.clone(),
                        service_name: item.service_name.clone(),
                        flow: item.flow.clone(),
                        reality_public_key: item.reality_public_key.clone(),
                        reality_private_key: item.reality_private_key.clone(),
                        reality_short_id: item.reality_short_id.clone(),
                        fingerprint: item.fingerprint.clone(),
                        alpn: item.alpn.clone(),
                        cipher: item.cipher.clone(),
                        tls_certificate_path: item.tls_certificate_path.clone(),
                        tls_key_path: item.tls_key_path.clone(),
                    }
                }));
            }
        }
        Ok(result)
    }

    async fn create_config_revision(
        &self,
        revision: ConfigRevision,
    ) -> ApplicationResult<ConfigRevision> {
        self.revisions
            .write()
            .expect("lock")
            .insert(revision.id, revision.clone());
        Ok(revision)
    }

    async fn create_rollout(
        &self,
        rollout: DeploymentRollout,
    ) -> ApplicationResult<DeploymentRollout> {
        self.rollouts
            .write()
            .expect("lock")
            .insert(rollout.id, rollout.clone());
        Ok(rollout)
    }

    async fn find_rollout(&self, rollout_id: Uuid) -> ApplicationResult<Option<DeploymentRollout>> {
        Ok(self
            .rollouts
            .read()
            .expect("lock")
            .get(&rollout_id)
            .cloned())
    }

    async fn list_rollouts(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<DeploymentRollout>> {
        let mut items = self
            .rollouts
            .read()
            .expect("lock")
            .values()
            .filter(|rollout| tenant_id.is_none() || Some(rollout.tenant_id) == tenant_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|right| std::cmp::Reverse(right.created_at));
        Ok(items)
    }

    async fn list_ready_rollouts(
        &self,
        node_id: Uuid,
        limit: i64,
    ) -> ApplicationResult<Vec<DeploymentRollout>> {
        let mut rollouts = self
            .rollouts
            .read()
            .expect("lock")
            .values()
            .filter(|rollout| {
                rollout.node_id == node_id
                    && matches!(
                        rollout.status,
                        DeploymentStatus::Ready
                            | DeploymentStatus::Queued
                            | DeploymentStatus::Rendering
                    )
            })
            .cloned()
            .collect::<Vec<_>>();
        rollouts.truncate(limit as usize);
        Ok(rollouts)
    }

    async fn update_rollout_state(
        &self,
        rollout_id: Uuid,
        status: DeploymentStatus,
        failure_reason: Option<String>,
    ) -> ApplicationResult<()> {
        let mut rollouts = self.rollouts.write().expect("lock");
        let rollout = rollouts
            .get_mut(&rollout_id)
            .ok_or_else(|| ApplicationError::NotFound("rollout not found".into()))?;
        rollout.status = status;
        rollout.failure_reason = failure_reason;
        rollout.updated_at = Utc::now();
        if status == DeploymentStatus::Applied {
            rollout.applied_at = Some(Utc::now());
        }
        Ok(())
    }
}

pub fn generate_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

fn deduplicate_engines(engines: Vec<ProxyEngine>) -> Vec<ProxyEngine> {
    let mut deduplicated = Vec::new();
    for engine in engines {
        if !deduplicated.contains(&engine) {
            deduplicated.push(engine);
        }
    }
    deduplicated
}

fn validate_registered_protocols(
    engine: ProxyEngine,
    protocols: &[ProtocolKind],
) -> ApplicationResult<()> {
    for protocol in protocols {
        if !engine_supports_protocol(engine, *protocol) {
            return Err(ApplicationError::Validation(format!(
                "{} does not support {}",
                engine_name(engine),
                protocol_name(*protocol)
            )));
        }
    }
    Ok(())
}

fn validate_endpoint_drafts(
    engine: ProxyEngine,
    capabilities: &[NodeCapability],
    drafts: &[NodeEndpointDraft],
) -> ApplicationResult<()> {
    for draft in drafts {
        validate_endpoint_draft(engine, capabilities, draft)?;
    }
    Ok(())
}

fn normalize_endpoint_drafts(drafts: Vec<NodeEndpointDraft>) -> Vec<NodeEndpointDraft> {
    drafts.into_iter().map(normalize_endpoint_draft).collect()
}

fn normalize_endpoint_draft(mut draft: NodeEndpointDraft) -> NodeEndpointDraft {
    if matches!(draft.security, SecurityKind::Reality) {
        if is_blank_option(&draft.reality_public_key) || is_blank_option(&draft.reality_private_key)
        {
            let (public_key, private_key) = generate_reality_key_pair();
            draft.reality_public_key = Some(public_key);
            draft.reality_private_key = Some(private_key);
        }
        if is_blank_option(&draft.reality_short_id) {
            draft.reality_short_id = Some(generate_reality_short_id());
        }
    }
    if matches!(draft.security, SecurityKind::Tls) {
        draft.tls_certificate_path = Some(DEFAULT_TLS_CERTIFICATE_PATH.into());
        draft.tls_key_path = Some(DEFAULT_TLS_KEY_PATH.into());
    } else {
        draft.tls_certificate_path = None;
        draft.tls_key_path = None;
    }
    draft
}

fn is_blank_option(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
}

fn generate_reality_key_pair() -> (String, String) {
    let private_key = StaticSecret::from(rand::random::<[u8; 32]>());
    let public_key = PublicKey::from(&private_key);
    (
        URL_SAFE_NO_PAD.encode(public_key.as_bytes()),
        URL_SAFE_NO_PAD.encode(private_key.to_bytes()),
    )
}

fn generate_reality_short_id() -> String {
    let mut bytes = [0_u8; 8];
    rand::rng().fill(&mut bytes);
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

const DEFAULT_TLS_CERTIFICATE_PATH: &str = "/var/lib/anneal/tls/server.crt";
const DEFAULT_TLS_KEY_PATH: &str = "/var/lib/anneal/tls/server.key";

#[derive(Clone, Copy)]
struct GeneratedTemplate {
    protocol: ProtocolKind,
    transport: anneal_config_engine::TransportKind,
    security: SecurityKind,
    public_port: u16,
    path: Option<&'static str>,
    service_name: Option<&'static str>,
    flow: Option<&'static str>,
    alpn: &'static [&'static str],
    cipher: Option<&'static str>,
    fingerprint: Option<&'static str>,
    include_host_header: bool,
}

struct ResolvedNodeDomain {
    domain: NodeDomain,
    public_hosts: Vec<String>,
}

fn normalize_node_group_domains(
    server_node_id: Uuid,
    drafts: Vec<NodeDomainDraft>,
    now: chrono::DateTime<Utc>,
) -> ApplicationResult<Vec<NodeDomain>> {
    drafts
        .into_iter()
        .map(|draft| {
            let domain = draft.domain.trim().to_owned();
            if domain.is_empty() {
                return Err(ApplicationError::Validation("domain is required".into()));
            }
            Ok(NodeDomain {
                id: Uuid::new_v4(),
                server_node_id,
                mode: draft.mode,
                domain,
                alias: normalize_optional_string(draft.alias),
                server_names: normalize_string_list(draft.server_names),
                host_headers: normalize_string_list(draft.host_headers),
                created_at: now,
                updated_at: now,
            })
        })
        .collect()
}

fn default_node_domain_from_public_base_url(public_base_url: &str) -> Option<String> {
    let trimmed = public_base_url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let authority = without_scheme.split('/').next()?.trim();
    if authority.is_empty() {
        return None;
    }
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host)
        .trim();
    if host.is_empty() {
        return None;
    }
    let normalized = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest).trim()
    } else {
        host.split(':').next().unwrap_or(host).trim()
    };
    let normalized = normalized.trim_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn default_node_domain_drafts(domain: &str) -> Vec<NodeDomainDraft> {
    vec![
        NodeDomainDraft {
            mode: NodeDomainMode::Direct,
            domain: domain.to_owned(),
            alias: Some("direct".into()),
            server_names: vec![],
            host_headers: vec![],
        },
        NodeDomainDraft {
            mode: NodeDomainMode::Worker,
            domain: domain.to_owned(),
            alias: Some("worker".into()),
            server_names: vec![domain.to_owned()],
            host_headers: vec![domain.to_owned()],
        },
        NodeDomainDraft {
            mode: NodeDomainMode::Reality,
            domain: domain.to_owned(),
            alias: Some("reality".into()),
            server_names: vec![domain.to_owned()],
            host_headers: vec![],
        },
    ]
}

fn bootstrap_node_name(base_name: &str, engine: ProxyEngine, engines_count: usize) -> String {
    if engines_count == 1 {
        return base_name.to_owned();
    }
    format!("{base_name}-{}", engine_key(engine))
}

fn engine_key(engine: ProxyEngine) -> &'static str {
    match engine {
        ProxyEngine::Xray => "xray",
        ProxyEngine::Singbox => "singbox",
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let item = item.trim().to_owned();
        if item.is_empty() { None } else { Some(item) }
    })
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim().to_owned();
        if !value.is_empty() && !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    normalized
}

fn reconcile_generated_endpoints(previous: &[NodeEndpoint], generated: &mut [NodeEndpoint]) {
    reconcile_endpoints(previous, generated, true);
}

fn reconcile_manual_endpoints(previous: &[NodeEndpoint], updated: &mut [NodeEndpoint]) {
    reconcile_endpoints(previous, updated, false);
}

fn reconcile_endpoints(
    previous: &[NodeEndpoint],
    updated: &mut [NodeEndpoint],
    keep_enabled: bool,
) {
    let mut previous_by_key = previous.iter().fold(
        HashMap::<String, VecDeque<&NodeEndpoint>>::new(),
        |mut grouped, endpoint| {
            grouped
                .entry(endpoint_state_key(endpoint))
                .or_default()
                .push_back(endpoint);
            grouped
        },
    );
    for endpoint in updated {
        let key = endpoint_state_key(endpoint);
        let Some(existing) = previous_by_key.get_mut(&key).and_then(VecDeque::pop_front) else {
            continue;
        };
        endpoint.id = existing.id;
        endpoint.created_at = existing.created_at;
        if keep_enabled {
            endpoint.enabled = existing.enabled;
        }
        if endpoint.security == SecurityKind::Reality {
            endpoint.reality_public_key = existing.reality_public_key.clone();
            endpoint.reality_private_key = existing.reality_private_key.clone();
            endpoint.reality_short_id = existing.reality_short_id.clone();
        }
    }
}

fn endpoint_state_key(endpoint: &NodeEndpoint) -> String {
    format!(
        "{:?}|{}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}",
        endpoint.protocol,
        endpoint.listen_host,
        endpoint.listen_port,
        endpoint.public_host,
        endpoint.public_port,
        endpoint.transport,
        endpoint.security,
        endpoint.server_name,
        endpoint.host_header,
        endpoint.path,
        endpoint.service_name,
        endpoint.flow,
        endpoint.cipher,
        endpoint.alpn.join(","),
        endpoint.tls_certificate_path.as_deref().unwrap_or_default(),
    )
}

fn build_group_generated_endpoints(
    nodes: &[NodeRuntime],
    capabilities_by_node: &HashMap<Uuid, Vec<NodeCapability>>,
    domains: &[ResolvedNodeDomain],
) -> ApplicationResult<HashMap<Uuid, Vec<NodeEndpoint>>> {
    let mut result = nodes
        .iter()
        .map(|node| (node.id, Vec::new()))
        .collect::<HashMap<_, _>>();
    let now = Utc::now();
    for domain in domains {
        let drafts = build_domain_endpoint_drafts(nodes, capabilities_by_node, domain);
        for node in nodes {
            let node_drafts = drafts.get(&node.id).cloned().unwrap_or_default();
            let normalized = normalize_endpoint_drafts(node_drafts);
            let capabilities = capabilities_by_node
                .get(&node.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            validate_endpoint_drafts(node.engine, capabilities, &normalized)?;
            let endpoints = normalized
                .into_iter()
                .map(|draft| NodeEndpoint {
                    id: Uuid::new_v4(),
                    node_id: node.id,
                    protocol: draft.protocol,
                    listen_host: draft.listen_host,
                    listen_port: i32::from(draft.listen_port),
                    public_host: draft.public_host,
                    public_port: i32::from(draft.public_port),
                    transport: draft.transport,
                    security: draft.security,
                    server_name: draft.server_name,
                    host_header: draft.host_header,
                    path: draft.path,
                    service_name: draft.service_name,
                    flow: draft.flow,
                    reality_public_key: draft.reality_public_key,
                    reality_private_key: draft.reality_private_key,
                    reality_short_id: draft.reality_short_id,
                    fingerprint: draft.fingerprint,
                    alpn: draft.alpn,
                    cipher: draft.cipher,
                    tls_certificate_path: draft.tls_certificate_path,
                    tls_key_path: draft.tls_key_path,
                    enabled: draft.enabled,
                    created_at: now,
                    updated_at: now,
                })
                .collect::<Vec<_>>();
            result.entry(node.id).or_default().extend(endpoints);
        }
    }
    Ok(result)
}

fn build_domain_endpoint_drafts(
    nodes: &[NodeRuntime],
    capabilities_by_node: &HashMap<Uuid, Vec<NodeCapability>>,
    resolved_domain: &ResolvedNodeDomain,
) -> HashMap<Uuid, Vec<NodeEndpointDraft>> {
    let mut result = HashMap::new();
    let domain = &resolved_domain.domain;
    let server_names = if domain.server_names.is_empty() {
        vec![domain.domain.clone()]
    } else {
        domain.server_names.clone()
    };
    let host_headers = if domain.host_headers.is_empty() {
        vec![domain.domain.clone()]
    } else {
        domain.host_headers.clone()
    };
    for template in endpoint_templates_for_mode(domain.mode) {
        let Some(owner) = select_owner_node(template.protocol, nodes, capabilities_by_node) else {
            continue;
        };
        let drafts = result.entry(owner.id).or_insert_with(Vec::new);
        for public_host in &resolved_domain.public_hosts {
            for (index, server_name) in server_names.iter().enumerate() {
                let host_header = if template.include_host_header {
                    Some(
                        host_headers
                            .get(index)
                            .cloned()
                            .or_else(|| host_headers.first().cloned())
                            .unwrap_or_else(|| domain.domain.clone()),
                    )
                } else {
                    None
                };
                drafts.push(NodeEndpointDraft {
                    protocol: template.protocol,
                    listen_host: "::".into(),
                    listen_port: template.public_port,
                    public_host: public_host.clone(),
                    public_port: template.public_port,
                    transport: template.transport,
                    security: template.security,
                    server_name: if template.security == SecurityKind::None {
                        None
                    } else {
                        Some(server_name.clone())
                    },
                    host_header,
                    path: template.path.map(str::to_owned),
                    service_name: template.service_name.map(str::to_owned),
                    flow: template.flow.map(str::to_owned),
                    reality_public_key: None,
                    reality_private_key: None,
                    reality_short_id: None,
                    fingerprint: template.fingerprint.map(str::to_owned),
                    alpn: template
                        .alpn
                        .iter()
                        .map(|item| (*item).to_owned())
                        .collect(),
                    cipher: template.cipher.map(str::to_owned),
                    tls_certificate_path: if template.security == SecurityKind::Tls {
                        Some(DEFAULT_TLS_CERTIFICATE_PATH.into())
                    } else {
                        None
                    },
                    tls_key_path: if template.security == SecurityKind::Tls {
                        Some(DEFAULT_TLS_KEY_PATH.into())
                    } else {
                        None
                    },
                    enabled: true,
                });
            }
        }
    }
    result
}

async fn resolve_public_hosts(domain: &NodeDomain) -> Vec<String> {
    if domain.mode != NodeDomainMode::AutoCdn {
        return vec![domain.domain.clone()];
    }
    let Ok(addresses) = tokio::net::lookup_host((domain.domain.as_str(), 443)).await else {
        return vec![domain.domain.clone()];
    };
    let hosts = addresses
        .map(|address| address.ip().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if hosts.is_empty() {
        vec![domain.domain.clone()]
    } else {
        hosts
    }
}

fn endpoint_templates_for_mode(mode: NodeDomainMode) -> Vec<GeneratedTemplate> {
    match mode {
        NodeDomainMode::Reality => vec![GeneratedTemplate {
            protocol: ProtocolKind::VlessReality,
            transport: anneal_config_engine::TransportKind::Tcp,
            security: SecurityKind::Reality,
            public_port: 443,
            path: None,
            service_name: None,
            flow: Some("xtls-rprx-vision"),
            alpn: &["h2", "http/1.1"],
            cipher: None,
            fingerprint: Some("chrome"),
            include_host_header: false,
        }],
        NodeDomainMode::Worker => vec![
            GeneratedTemplate {
                protocol: ProtocolKind::VlessReality,
                transport: anneal_config_engine::TransportKind::Ws,
                security: SecurityKind::Tls,
                public_port: 8443,
                path: Some("/vless-ws"),
                service_name: None,
                flow: None,
                alpn: &["http/1.1"],
                cipher: None,
                fingerprint: Some("chrome"),
                include_host_header: true,
            },
            GeneratedTemplate {
                protocol: ProtocolKind::VlessReality,
                transport: anneal_config_engine::TransportKind::HttpUpgrade,
                security: SecurityKind::Tls,
                public_port: 10443,
                path: Some("/vless-upgrade"),
                service_name: None,
                flow: None,
                alpn: &["http/1.1"],
                cipher: None,
                fingerprint: Some("chrome"),
                include_host_header: true,
            },
            GeneratedTemplate {
                protocol: ProtocolKind::Trojan,
                transport: anneal_config_engine::TransportKind::Ws,
                security: SecurityKind::Tls,
                public_port: 13443,
                path: Some("/trojan-ws"),
                service_name: None,
                flow: None,
                alpn: &["http/1.1"],
                cipher: None,
                fingerprint: Some("chrome"),
                include_host_header: true,
            },
            GeneratedTemplate {
                protocol: ProtocolKind::Vmess,
                transport: anneal_config_engine::TransportKind::Ws,
                security: SecurityKind::Tls,
                public_port: 18443,
                path: Some("/vmess-ws"),
                service_name: None,
                flow: None,
                alpn: &["http/1.1"],
                cipher: None,
                fingerprint: Some("chrome"),
                include_host_header: true,
            },
        ],
        _ => {
            let include_grpc = mode != NodeDomainMode::LegacyDirect;
            let include_udp = matches!(
                mode,
                NodeDomainMode::Direct
                    | NodeDomainMode::LegacyDirect
                    | NodeDomainMode::Relay
                    | NodeDomainMode::Fake
            );
            let mut templates = vec![
                GeneratedTemplate {
                    protocol: ProtocolKind::VlessReality,
                    transport: anneal_config_engine::TransportKind::Ws,
                    security: SecurityKind::Tls,
                    public_port: 8443,
                    path: Some("/vless-ws"),
                    service_name: None,
                    flow: None,
                    alpn: &["http/1.1"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::VlessReality,
                    transport: anneal_config_engine::TransportKind::HttpUpgrade,
                    security: SecurityKind::Tls,
                    public_port: 10443,
                    path: Some("/vless-upgrade"),
                    service_name: None,
                    flow: None,
                    alpn: &["http/1.1"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::VlessReality,
                    transport: anneal_config_engine::TransportKind::Ws,
                    security: SecurityKind::Tls,
                    public_port: 11443,
                    path: Some("/vless-ws-h2"),
                    service_name: None,
                    flow: None,
                    alpn: &["h2"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::Trojan,
                    transport: anneal_config_engine::TransportKind::Ws,
                    security: SecurityKind::Tls,
                    public_port: 13443,
                    path: Some("/trojan-ws"),
                    service_name: None,
                    flow: None,
                    alpn: &["http/1.1"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::Trojan,
                    transport: anneal_config_engine::TransportKind::HttpUpgrade,
                    security: SecurityKind::Tls,
                    public_port: 15443,
                    path: Some("/trojan-upgrade"),
                    service_name: None,
                    flow: None,
                    alpn: &["http/1.1"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::Trojan,
                    transport: anneal_config_engine::TransportKind::Ws,
                    security: SecurityKind::Tls,
                    public_port: 16443,
                    path: Some("/trojan-ws-h2"),
                    service_name: None,
                    flow: None,
                    alpn: &["h2"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::Vmess,
                    transport: anneal_config_engine::TransportKind::Ws,
                    security: SecurityKind::Tls,
                    public_port: 18443,
                    path: Some("/vmess-ws"),
                    service_name: None,
                    flow: None,
                    alpn: &["http/1.1"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::Vmess,
                    transport: anneal_config_engine::TransportKind::HttpUpgrade,
                    security: SecurityKind::Tls,
                    public_port: 20443,
                    path: Some("/vmess-upgrade"),
                    service_name: None,
                    flow: None,
                    alpn: &["http/1.1"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::Vmess,
                    transport: anneal_config_engine::TransportKind::Ws,
                    security: SecurityKind::Tls,
                    public_port: 21443,
                    path: Some("/vmess-ws-h2"),
                    service_name: None,
                    flow: None,
                    alpn: &["h2"],
                    cipher: None,
                    fingerprint: Some("chrome"),
                    include_host_header: true,
                },
                GeneratedTemplate {
                    protocol: ProtocolKind::Shadowsocks2022,
                    transport: anneal_config_engine::TransportKind::Tcp,
                    security: SecurityKind::None,
                    public_port: 8388,
                    path: None,
                    service_name: None,
                    flow: None,
                    alpn: &[],
                    cipher: Some("2022-blake3-aes-128-gcm"),
                    fingerprint: None,
                    include_host_header: false,
                },
            ];
            if include_grpc {
                templates.extend([
                    GeneratedTemplate {
                        protocol: ProtocolKind::VlessReality,
                        transport: anneal_config_engine::TransportKind::Grpc,
                        security: SecurityKind::Tls,
                        public_port: 9443,
                        path: None,
                        service_name: Some("vless-grpc"),
                        flow: None,
                        alpn: &["h2", "http/1.1"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                    GeneratedTemplate {
                        protocol: ProtocolKind::VlessReality,
                        transport: anneal_config_engine::TransportKind::Grpc,
                        security: SecurityKind::Tls,
                        public_port: 12443,
                        path: None,
                        service_name: Some("vless-grpc-h2"),
                        flow: None,
                        alpn: &["h2"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                    GeneratedTemplate {
                        protocol: ProtocolKind::Trojan,
                        transport: anneal_config_engine::TransportKind::Grpc,
                        security: SecurityKind::Tls,
                        public_port: 14443,
                        path: None,
                        service_name: Some("trojan-grpc"),
                        flow: None,
                        alpn: &["h2", "http/1.1"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                    GeneratedTemplate {
                        protocol: ProtocolKind::Trojan,
                        transport: anneal_config_engine::TransportKind::Grpc,
                        security: SecurityKind::Tls,
                        public_port: 17443,
                        path: None,
                        service_name: Some("trojan-grpc-h2"),
                        flow: None,
                        alpn: &["h2"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                    GeneratedTemplate {
                        protocol: ProtocolKind::Vmess,
                        transport: anneal_config_engine::TransportKind::Grpc,
                        security: SecurityKind::Tls,
                        public_port: 19443,
                        path: None,
                        service_name: Some("vmess-grpc"),
                        flow: None,
                        alpn: &["h2", "http/1.1"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                    GeneratedTemplate {
                        protocol: ProtocolKind::Vmess,
                        transport: anneal_config_engine::TransportKind::Grpc,
                        security: SecurityKind::Tls,
                        public_port: 22443,
                        path: None,
                        service_name: Some("vmess-grpc-h2"),
                        flow: None,
                        alpn: &["h2"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                ]);
            }
            if include_udp {
                templates.extend([
                    GeneratedTemplate {
                        protocol: ProtocolKind::Tuic,
                        transport: anneal_config_engine::TransportKind::Tcp,
                        security: SecurityKind::Tls,
                        public_port: 24443,
                        path: None,
                        service_name: None,
                        flow: None,
                        alpn: &["h3"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                    GeneratedTemplate {
                        protocol: ProtocolKind::Hysteria2,
                        transport: anneal_config_engine::TransportKind::Tcp,
                        security: SecurityKind::Tls,
                        public_port: 25443,
                        path: None,
                        service_name: None,
                        flow: None,
                        alpn: &["h3"],
                        cipher: None,
                        fingerprint: Some("chrome"),
                        include_host_header: false,
                    },
                ]);
            }
            templates
        }
    }
}

fn select_owner_node<'a>(
    protocol: ProtocolKind,
    nodes: &'a [NodeRuntime],
    capabilities_by_node: &HashMap<Uuid, Vec<NodeCapability>>,
) -> Option<&'a NodeRuntime> {
    let priorities = match protocol {
        ProtocolKind::Tuic | ProtocolKind::Hysteria2 => {
            [ProxyEngine::Singbox, ProxyEngine::Singbox]
        }
        _ => [ProxyEngine::Xray, ProxyEngine::Singbox],
    };
    for engine in priorities {
        if let Some(node) = nodes.iter().find(|node| {
            node.engine == engine && node_supports_protocol(node, protocol, capabilities_by_node)
        }) {
            return Some(node);
        }
    }
    None
}

fn node_supports_protocol(
    node: &NodeRuntime,
    protocol: ProtocolKind,
    capabilities_by_node: &HashMap<Uuid, Vec<NodeCapability>>,
) -> bool {
    let capabilities = capabilities_by_node
        .get(&node.id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if capabilities.is_empty() {
        return engine_supports_protocol(node.engine, protocol);
    }
    capabilities
        .iter()
        .any(|capability| capability.protocol == protocol)
}

fn validate_endpoint_draft(
    engine: ProxyEngine,
    capabilities: &[NodeCapability],
    draft: &NodeEndpointDraft,
) -> ApplicationResult<()> {
    if !engine_supports_protocol(engine, draft.protocol) {
        return Err(ApplicationError::Validation(format!(
            "{} does not support {}",
            engine_name(engine),
            protocol_name(draft.protocol)
        )));
    }
    if !capabilities.is_empty()
        && !capabilities
            .iter()
            .any(|capability| capability.protocol == draft.protocol)
    {
        return Err(ApplicationError::Validation(format!(
            "node does not advertise {} capability",
            protocol_name(draft.protocol)
        )));
    }
    match draft.protocol {
        ProtocolKind::VlessReality => {
            if draft.security == SecurityKind::Reality
                && (draft.server_name.is_none()
                    || draft.reality_public_key.is_none()
                    || draft.reality_private_key.is_none()
                    || draft.reality_short_id.is_none())
            {
                return Err(ApplicationError::Validation(
                    "vless reality endpoints require server_name and reality keys".into(),
                ));
            }
        }
        ProtocolKind::Trojan => {
            require_tls_profile("trojan", draft)?;
        }
        ProtocolKind::Shadowsocks2022 => {
            if draft
                .cipher
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(ApplicationError::Validation(
                    "shadowsocks_2022 endpoints require cipher".into(),
                ));
            }
        }
        ProtocolKind::Tuic => {
            require_tls_profile("tuic", draft)?;
            if draft.alpn.is_empty() {
                return Err(ApplicationError::Validation(
                    "tuic endpoints require alpn".into(),
                ));
            }
        }
        ProtocolKind::Hysteria2 => {
            require_tls_profile("hysteria2", draft)?;
            if draft.alpn.is_empty() {
                return Err(ApplicationError::Validation(
                    "hysteria2 endpoints require alpn".into(),
                ));
            }
        }
        ProtocolKind::Vmess => {}
    }
    if draft.security == SecurityKind::Tls && draft.server_name.is_none() {
        return Err(ApplicationError::Validation(
            "tls endpoints require server_name".into(),
        ));
    }
    Ok(())
}

fn require_tls_profile(name: &str, draft: &NodeEndpointDraft) -> ApplicationResult<()> {
    if draft.security != SecurityKind::Tls {
        return Err(ApplicationError::Validation(format!(
            "{name} endpoints require tls security"
        )));
    }
    if draft.server_name.is_none() {
        return Err(ApplicationError::Validation(format!(
            "{name} endpoints require server_name"
        )));
    }
    Ok(())
}

fn engine_supports_protocol(engine: ProxyEngine, protocol: ProtocolKind) -> bool {
    match engine {
        ProxyEngine::Xray => !matches!(protocol, ProtocolKind::Tuic | ProtocolKind::Hysteria2),
        ProxyEngine::Singbox => true,
    }
}

fn engine_name(engine: ProxyEngine) -> &'static str {
    match engine {
        ProxyEngine::Xray => "xray",
        ProxyEngine::Singbox => "singbox",
    }
}

fn protocol_name(protocol: ProtocolKind) -> &'static str {
    match protocol {
        ProtocolKind::VlessReality => "vless_reality",
        ProtocolKind::Vmess => "vmess",
        ProtocolKind::Trojan => "trojan",
        ProtocolKind::Shadowsocks2022 => "shadowsocks_2022",
        ProtocolKind::Tuic => "tuic",
        ProtocolKind::Hysteria2 => "hysteria2",
    }
}

#[cfg(test)]
mod tests {
    use anneal_config_engine::{SecurityKind, TransportKind};
    use anneal_core::{Actor, NodeStatus, ProtocolKind, ProxyEngine, TokenHasher, UserRole};
    use anneal_rbac::RbacService;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use crate::{
        application::{
            InMemoryNodeRepository, NodeRepository, NodeService,
            default_node_domain_from_public_base_url,
        },
        domain::{NodeDomainDraft, NodeDomainMode, NodeEndpointDraft, RuntimeRegistration},
    };

    fn draft_from_endpoint(endpoint: &crate::domain::NodeEndpoint) -> NodeEndpointDraft {
        NodeEndpointDraft {
            protocol: endpoint.protocol,
            listen_host: endpoint.listen_host.clone(),
            listen_port: u16::try_from(endpoint.listen_port).expect("listen port"),
            public_host: endpoint.public_host.clone(),
            public_port: u16::try_from(endpoint.public_port).expect("public port"),
            transport: endpoint.transport,
            security: endpoint.security,
            server_name: endpoint.server_name.clone(),
            host_header: endpoint.host_header.clone(),
            path: endpoint.path.clone(),
            service_name: endpoint.service_name.clone(),
            flow: endpoint.flow.clone(),
            reality_public_key: endpoint.reality_public_key.clone(),
            reality_private_key: endpoint.reality_private_key.clone(),
            reality_short_id: endpoint.reality_short_id.clone(),
            fingerprint: endpoint.fingerprint.clone(),
            alpn: endpoint.alpn.clone(),
            cipher: endpoint.cipher.clone(),
            tls_certificate_path: endpoint.tls_certificate_path.clone(),
            tls_key_path: endpoint.tls_key_path.clone(),
            enabled: endpoint.enabled,
        }
    }

    #[test]
    fn panel_domain_is_extracted_from_public_base_url() {
        assert_eq!(
            default_node_domain_from_public_base_url("https://test.aurausa.me/hidden/panel"),
            Some("test.aurausa.me".into())
        );
        assert_eq!(
            default_node_domain_from_public_base_url("https://test.aurausa.me.:8443/hidden"),
            Some("test.aurausa.me".into())
        );
    }

    #[tokio::test]
    async fn enrollment_token_registers_node() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };

        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");

        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![],
                },
            )
            .await
            .expect("register");

        assert_eq!(node.status, NodeStatus::Online);
    }

    #[tokio::test]
    async fn runtime_registration_auto_seeds_default_domains_and_proxy_endpoints() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::with_public_base_url(
            &repository,
            RbacService,
            TokenHasher::new("test-node-seed-hash-key").expect("token hasher"),
            "https://test.aurausa.me/hidden-panel",
        );
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let xray_token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("xray token");
        let xray_node = service
            .register_node(
                &xray_token.token,
                RuntimeRegistration {
                    name: "edge-xray".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![
                        ProtocolKind::VlessReality,
                        ProtocolKind::Vmess,
                        ProtocolKind::Trojan,
                        ProtocolKind::Shadowsocks2022,
                    ],
                },
            )
            .await
            .expect("xray register");

        let domains = repository
            .list_node_domains(group.id)
            .await
            .expect("domains after seed");
        assert_eq!(domains.len(), 3);
        assert!(
            domains
                .iter()
                .any(|domain| domain.mode == NodeDomainMode::Direct)
        );
        assert!(
            domains
                .iter()
                .any(|domain| domain.mode == NodeDomainMode::Worker)
        );
        assert!(
            domains
                .iter()
                .any(|domain| domain.mode == NodeDomainMode::Reality)
        );
        assert!(
            domains
                .iter()
                .all(|domain| domain.domain == "test.aurausa.me")
        );

        let xray_endpoints = repository
            .list_node_endpoints(xray_node.id)
            .await
            .expect("xray endpoints");
        assert!(!xray_endpoints.is_empty());
        assert!(
            xray_endpoints
                .iter()
                .all(|endpoint| endpoint.public_host == "test.aurausa.me")
        );

        let singbox_token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Singbox,
            )
            .await
            .expect("singbox token");
        let singbox_node = service
            .register_node(
                &singbox_token.token,
                RuntimeRegistration {
                    name: "edge-singbox".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Singbox,
                    protocols: vec![
                        ProtocolKind::VlessReality,
                        ProtocolKind::Vmess,
                        ProtocolKind::Trojan,
                        ProtocolKind::Shadowsocks2022,
                        ProtocolKind::Tuic,
                        ProtocolKind::Hysteria2,
                    ],
                },
            )
            .await
            .expect("singbox register");

        let singbox_endpoints = repository
            .list_node_endpoints(singbox_node.id)
            .await
            .expect("singbox endpoints");
        assert!(
            singbox_endpoints
                .iter()
                .any(|endpoint| endpoint.protocol == ProtocolKind::Tuic)
        );
        assert!(
            singbox_endpoints
                .iter()
                .any(|endpoint| endpoint.protocol == ProtocolKind::Hysteria2)
        );
    }

    #[tokio::test]
    async fn replacing_endpoints_populates_delivery_catalog() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Singbox,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Singbox,
                    protocols: vec![ProtocolKind::Tuic],
                },
            )
            .await
            .expect("register");

        service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                vec![NodeEndpointDraft {
                    protocol: ProtocolKind::Tuic,
                    listen_host: "::".into(),
                    listen_port: 443,
                    public_host: "edge.example.com".into(),
                    public_port: 443,
                    transport: TransportKind::Tcp,
                    security: SecurityKind::Tls,
                    server_name: Some("edge.example.com".into()),
                    host_header: None,
                    path: None,
                    service_name: None,
                    flow: None,
                    reality_public_key: None,
                    reality_private_key: None,
                    reality_short_id: None,
                    fingerprint: Some("chrome".into()),
                    alpn: vec!["h3".into()],
                    cipher: None,
                    tls_certificate_path: Some("/var/lib/anneal/tls/server.crt".into()),
                    tls_key_path: Some("/var/lib/anneal/tls/server.key".into()),
                    enabled: true,
                }],
            )
            .await
            .expect("replace");

        let delivery = repository
            .list_delivery_endpoints(actor.tenant_id.expect("tenant"))
            .await
            .expect("delivery");
        assert_eq!(delivery.len(), 1);
        assert_eq!(delivery[0].public_host, "edge.example.com");
    }

    #[tokio::test]
    async fn xray_rejects_tuic_endpoint() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::Vmess],
                },
            )
            .await
            .expect("register");

        let error = service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                vec![NodeEndpointDraft {
                    protocol: ProtocolKind::Tuic,
                    listen_host: "::".into(),
                    listen_port: 443,
                    public_host: "edge.example.com".into(),
                    public_port: 443,
                    transport: TransportKind::Tcp,
                    security: SecurityKind::Tls,
                    server_name: Some("edge.example.com".into()),
                    host_header: None,
                    path: None,
                    service_name: None,
                    flow: None,
                    reality_public_key: None,
                    reality_private_key: None,
                    reality_short_id: None,
                    fingerprint: Some("chrome".into()),
                    alpn: vec!["h3".into()],
                    cipher: None,
                    tls_certificate_path: Some("/var/lib/anneal/tls/server.crt".into()),
                    tls_key_path: Some("/var/lib/anneal/tls/server.key".into()),
                    enabled: true,
                }],
            )
            .await
            .expect_err("must reject");

        assert_eq!(error.to_string(), "xray does not support tuic");
    }

    #[tokio::test]
    async fn reality_endpoint_generates_missing_keys() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register");

        let endpoints = service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                vec![NodeEndpointDraft {
                    protocol: ProtocolKind::VlessReality,
                    listen_host: "::".into(),
                    listen_port: 443,
                    public_host: "edge.example.com".into(),
                    public_port: 443,
                    transport: TransportKind::Tcp,
                    security: SecurityKind::Reality,
                    server_name: Some("edge.example.com".into()),
                    host_header: None,
                    path: None,
                    service_name: None,
                    flow: Some("xtls-rprx-vision".into()),
                    reality_public_key: None,
                    reality_private_key: None,
                    reality_short_id: None,
                    fingerprint: Some("chrome".into()),
                    alpn: vec!["h2".into(), "http/1.1".into()],
                    cipher: None,
                    tls_certificate_path: None,
                    tls_key_path: None,
                    enabled: true,
                }],
            )
            .await
            .expect("replace");

        assert_eq!(endpoints.len(), 1);
        assert!(endpoints[0].reality_public_key.is_some());
        assert!(endpoints[0].reality_private_key.is_some());
        assert_eq!(
            endpoints[0].reality_short_id.as_deref().map(str::len),
            Some(16)
        );
    }

    #[tokio::test]
    async fn vless_tls_endpoint_is_allowed() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register");

        let endpoints = service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                vec![NodeEndpointDraft {
                    protocol: ProtocolKind::VlessReality,
                    listen_host: "::".into(),
                    listen_port: 443,
                    public_host: "edge.example.com".into(),
                    public_port: 443,
                    transport: TransportKind::Ws,
                    security: SecurityKind::Tls,
                    server_name: Some("edge.example.com".into()),
                    host_header: Some("cdn.example.com".into()),
                    path: Some("/vless-ws".into()),
                    service_name: None,
                    flow: None,
                    reality_public_key: None,
                    reality_private_key: None,
                    reality_short_id: None,
                    fingerprint: Some("chrome".into()),
                    alpn: vec!["http/1.1".into()],
                    cipher: None,
                    tls_certificate_path: Some("/var/lib/anneal/tls/server.crt".into()),
                    tls_key_path: Some("/var/lib/anneal/tls/server.key".into()),
                    enabled: true,
                }],
            )
            .await
            .expect("replace");

        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].security, SecurityKind::Tls);
        assert!(endpoints[0].reality_public_key.is_none());
    }

    #[tokio::test]
    async fn replacing_group_domains_generates_hiddify_defaults() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let xray_token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("xray token");
        let singbox_token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Singbox,
            )
            .await
            .expect("singbox token");
        let xray = service
            .register_node(
                &xray_token.token,
                RuntimeRegistration {
                    name: "edge-xray".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![
                        ProtocolKind::VlessReality,
                        ProtocolKind::Vmess,
                        ProtocolKind::Trojan,
                        ProtocolKind::Shadowsocks2022,
                    ],
                },
            )
            .await
            .expect("register xray");
        let singbox = service
            .register_node(
                &singbox_token.token,
                RuntimeRegistration {
                    name: "edge-singbox".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Singbox,
                    protocols: vec![
                        ProtocolKind::VlessReality,
                        ProtocolKind::Vmess,
                        ProtocolKind::Trojan,
                        ProtocolKind::Shadowsocks2022,
                        ProtocolKind::Tuic,
                        ProtocolKind::Hysteria2,
                    ],
                },
            )
            .await
            .expect("register singbox");

        let domains = service
            .replace_node_domains(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                vec![NodeDomainDraft {
                    mode: NodeDomainMode::Direct,
                    domain: "edge.example.com".into(),
                    alias: Some("main".into()),
                    server_names: vec![],
                    host_headers: vec![],
                }],
            )
            .await
            .expect("replace domains");

        assert_eq!(domains.len(), 1);

        let xray_endpoints = repository
            .list_node_endpoints(xray.id)
            .await
            .expect("xray endpoints");
        let singbox_endpoints = repository
            .list_node_endpoints(singbox.id)
            .await
            .expect("singbox endpoints");

        assert!(xray_endpoints.iter().any(|endpoint| {
            endpoint.protocol == ProtocolKind::VlessReality
                && endpoint.security == SecurityKind::Tls
                && endpoint.transport == TransportKind::Ws
                && endpoint.public_host == "edge.example.com"
        }));
        assert!(xray_endpoints.iter().any(|endpoint| {
            endpoint.protocol == ProtocolKind::Trojan && endpoint.transport == TransportKind::Grpc
        }));
        assert!(xray_endpoints.iter().any(|endpoint| {
            endpoint.protocol == ProtocolKind::Shadowsocks2022 && endpoint.public_port == 8388
        }));
        assert!(singbox_endpoints.iter().any(|endpoint| {
            endpoint.protocol == ProtocolKind::Tuic
                && endpoint.public_port == 24443
                && endpoint.server_name.as_deref() == Some("edge.example.com")
        }));
        assert!(singbox_endpoints.iter().any(|endpoint| {
            endpoint.protocol == ProtocolKind::Hysteria2 && endpoint.public_port == 25443
        }));
    }

    #[tokio::test]
    async fn reality_group_domains_generate_variants_for_each_sni() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-xray".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register");

        service
            .replace_node_domains(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                vec![NodeDomainDraft {
                    mode: NodeDomainMode::Reality,
                    domain: "gateway.example.com".into(),
                    alias: None,
                    server_names: vec!["cdn-a.example.com".into(), "cdn-b.example.com".into()],
                    host_headers: vec![],
                }],
            )
            .await
            .expect("replace domains");

        let endpoints = repository
            .list_node_endpoints(node.id)
            .await
            .expect("endpoints");

        assert_eq!(endpoints.len(), 2);
        assert!(endpoints.iter().all(|endpoint| {
            endpoint.protocol == ProtocolKind::VlessReality
                && endpoint.security == SecurityKind::Reality
                && endpoint.transport == TransportKind::Tcp
                && endpoint.reality_public_key.is_some()
                && endpoint.reality_private_key.is_some()
                && endpoint.reality_short_id.is_some()
        }));
        assert!(
            endpoints
                .iter()
                .any(|endpoint| endpoint.server_name.as_deref() == Some("cdn-a.example.com"))
        );
        assert!(
            endpoints
                .iter()
                .any(|endpoint| endpoint.server_name.as_deref() == Some("cdn-b.example.com"))
        );
    }

    #[tokio::test]
    async fn generated_endpoint_toggle_survives_domain_resync() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-xray".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![
                        ProtocolKind::VlessReality,
                        ProtocolKind::Vmess,
                        ProtocolKind::Trojan,
                        ProtocolKind::Shadowsocks2022,
                    ],
                },
            )
            .await
            .expect("register");

        service
            .replace_node_domains(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                vec![NodeDomainDraft {
                    mode: NodeDomainMode::Direct,
                    domain: "edge.example.com".into(),
                    alias: None,
                    server_names: vec![],
                    host_headers: vec![],
                }],
            )
            .await
            .expect("replace domains");

        let mut endpoints = repository
            .list_node_endpoints(node.id)
            .await
            .expect("endpoints");
        let disabled_index = endpoints
            .iter()
            .position(|endpoint| {
                endpoint.public_host == "edge.example.com"
                    && endpoint.protocol == ProtocolKind::Vmess
                    && endpoint.transport == TransportKind::Ws
            })
            .expect("vmess ws endpoint");
        endpoints[disabled_index].enabled = false;

        service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                endpoints.iter().map(draft_from_endpoint).collect(),
            )
            .await
            .expect("replace endpoints");

        let disabled_id = repository
            .list_node_endpoints(node.id)
            .await
            .expect("endpoints")
            .into_iter()
            .find(|endpoint| {
                endpoint.public_host == "edge.example.com"
                    && endpoint.protocol == ProtocolKind::Vmess
                    && endpoint.transport == TransportKind::Ws
            })
            .expect("disabled endpoint after replace")
            .id;

        service
            .replace_node_domains(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                vec![NodeDomainDraft {
                    mode: NodeDomainMode::Direct,
                    domain: "edge.example.com".into(),
                    alias: None,
                    server_names: vec![],
                    host_headers: vec![],
                }],
            )
            .await
            .expect("resync domains");

        let endpoints = repository
            .list_node_endpoints(node.id)
            .await
            .expect("endpoints");
        let disabled = endpoints
            .iter()
            .find(|endpoint| endpoint.id == disabled_id)
            .expect("disabled endpoint");
        assert!(!disabled.enabled);
    }

    #[tokio::test]
    async fn generated_reality_keys_survive_domain_resync() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let token = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &token.token,
                RuntimeRegistration {
                    name: "edge-xray".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register");

        service
            .replace_node_domains(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                vec![NodeDomainDraft {
                    mode: NodeDomainMode::Reality,
                    domain: "gateway.example.com".into(),
                    alias: None,
                    server_names: vec!["cdn-a.example.com".into()],
                    host_headers: vec![],
                }],
            )
            .await
            .expect("replace domains");

        let first = repository
            .list_node_endpoints(node.id)
            .await
            .expect("first endpoints")
            .into_iter()
            .next()
            .expect("first endpoint");

        service
            .replace_node_domains(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                vec![NodeDomainDraft {
                    mode: NodeDomainMode::Reality,
                    domain: "gateway.example.com".into(),
                    alias: None,
                    server_names: vec!["cdn-a.example.com".into()],
                    host_headers: vec![],
                }],
            )
            .await
            .expect("resync domains");

        let second = repository
            .list_node_endpoints(node.id)
            .await
            .expect("second endpoints")
            .into_iter()
            .next()
            .expect("second endpoint");

        assert_eq!(first.id, second.id);
        assert_eq!(first.reality_public_key, second.reality_public_key);
        assert_eq!(first.reality_private_key, second.reality_private_key);
        assert_eq!(first.reality_short_id, second.reality_short_id);
    }

    #[test]
    fn generated_endpoint_reconcile_keeps_distinct_ids_for_duplicate_state_keys() {
        let node_id = Uuid::new_v4();
        let created_at = Utc::now();
        let first_existing = crate::domain::NodeEndpoint {
            id: Uuid::new_v4(),
            node_id,
            protocol: ProtocolKind::Vmess,
            listen_host: "::".into(),
            listen_port: 443,
            public_host: "test.aurausa.me".into(),
            public_port: 443,
            transport: TransportKind::Ws,
            security: SecurityKind::None,
            server_name: Some("test.aurausa.me".into()),
            host_header: None,
            path: Some("/vmess".into()),
            service_name: None,
            flow: None,
            reality_public_key: None,
            reality_private_key: None,
            reality_short_id: None,
            fingerprint: None,
            alpn: Vec::new(),
            cipher: None,
            tls_certificate_path: None,
            tls_key_path: None,
            enabled: false,
            created_at,
            updated_at: created_at,
        };
        let second_existing = crate::domain::NodeEndpoint {
            id: Uuid::new_v4(),
            enabled: true,
            created_at: created_at + Duration::seconds(1),
            updated_at: created_at + Duration::seconds(1),
            ..first_existing.clone()
        };
        let mut generated = vec![
            crate::domain::NodeEndpoint {
                id: Uuid::new_v4(),
                enabled: true,
                created_at: created_at + Duration::seconds(2),
                updated_at: created_at + Duration::seconds(2),
                ..first_existing.clone()
            },
            crate::domain::NodeEndpoint {
                id: Uuid::new_v4(),
                enabled: true,
                created_at: created_at + Duration::seconds(3),
                updated_at: created_at + Duration::seconds(3),
                ..first_existing.clone()
            },
        ];

        super::reconcile_generated_endpoints(
            &[first_existing.clone(), second_existing.clone()],
            &mut generated,
        );

        assert_eq!(generated[0].id, first_existing.id);
        assert_eq!(generated[0].enabled, first_existing.enabled);
        assert_eq!(generated[0].created_at, first_existing.created_at);
        assert_eq!(generated[1].id, second_existing.id);
        assert_eq!(generated[1].enabled, second_existing.enabled);
        assert_eq!(generated[1].created_at, second_existing.created_at);
    }

    #[test]
    fn stale_node_becomes_offline() {
        let now = Utc::now();
        let status = NodeService::<InMemoryNodeRepository>::resolve_status(
            now - Duration::seconds(180),
            now,
        );
        assert_eq!(status, NodeStatus::Offline);
    }

    #[tokio::test]
    async fn heartbeat_requires_matching_node_token() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let grant = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &grant.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register");

        let wrong = service
            .heartbeat(node.id, "wrong-token", "1.0.1")
            .await
            .expect_err("unauthorized heartbeat");
        assert!(matches!(wrong, anneal_core::ApplicationError::Unauthorized));

        let updated = service
            .heartbeat(node.id, &node.node_token, "1.0.1")
            .await
            .expect("heartbeat");
        assert_eq!(updated.version, "1.0.1");
    }

    #[tokio::test]
    async fn enrollment_token_requires_matching_tenant_group() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(repository, RbacService);
        let owner = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let intruder = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&owner, owner.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");

        let error = service
            .create_enrollment_token(
                &intruder,
                intruder.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect_err("foreign tenant must be rejected");
        assert!(matches!(error, anneal_core::ApplicationError::Forbidden));
    }

    #[tokio::test]
    async fn rollout_ack_requires_owner_node_token() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let first_grant = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("first token");
        let second_grant = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("second token");
        let first_node = service
            .register_node(
                &first_grant.token,
                RuntimeRegistration {
                    name: "edge-a".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register first");
        let second_node = service
            .register_node(
                &second_grant.token,
                RuntimeRegistration {
                    name: "edge-b".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register second");
        let rollout = service
            .queue_rollout(
                &actor,
                actor.tenant_id.expect("tenant"),
                first_node.id,
                "main".into(),
                "{}".into(),
                "/etc/anneal/config.json".into(),
            )
            .await
            .expect("queue rollout");

        let pull_error = service
            .pull_rollouts(first_node.id, &second_node.node_token, 10)
            .await
            .expect_err("pull must reject foreign token");
        assert!(matches!(
            pull_error,
            anneal_core::ApplicationError::Forbidden
        ));

        let ack_error = service
            .acknowledge_rollout(
                first_node.id,
                &second_node.node_token,
                rollout.id,
                true,
                None,
            )
            .await
            .expect_err("ack must reject foreign token");
        assert!(matches!(
            ack_error,
            anneal_core::ApplicationError::Forbidden
        ));

        let acknowledged = service
            .acknowledge_rollout(
                first_node.id,
                &first_node.node_token,
                rollout.id,
                true,
                None,
            )
            .await
            .expect("ack");
        assert_eq!(acknowledged.id, rollout.id);
        assert_eq!(acknowledged.node_id, first_node.id);
    }

    #[tokio::test]
    async fn bootstrap_token_registers_multiple_runtimes_once() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let bootstrap = service
            .create_bootstrap_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                "edge".into(),
                vec![ProxyEngine::Xray, ProxyEngine::Singbox],
            )
            .await
            .expect("bootstrap");

        let grants = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![
                    RuntimeRegistration {
                        name: "edge-xray".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Xray,
                        protocols: vec![ProtocolKind::VlessReality],
                    },
                    RuntimeRegistration {
                        name: "edge-singbox".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Singbox,
                        protocols: vec![ProtocolKind::VlessReality, ProtocolKind::Tuic],
                    },
                ],
            )
            .await
            .expect("bootstrap nodes");

        assert_eq!(grants.len(), 2);

        let second_attempt = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![RuntimeRegistration {
                    name: "edge-xray".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                }],
            )
            .await
            .expect_err("bootstrap token must be one-time via consumed grants");
        assert!(matches!(
            second_attempt,
            anneal_core::ApplicationError::Unauthorized
        ));
    }

    #[tokio::test]
    async fn failed_bootstrap_attempt_keeps_token_reusable() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let bootstrap = service
            .create_bootstrap_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                "edge".into(),
                vec![ProxyEngine::Xray, ProxyEngine::Singbox],
            )
            .await
            .expect("bootstrap");

        let first_attempt = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![RuntimeRegistration {
                    name: "edge-xray".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                }],
            )
            .await
            .expect_err("bootstrap must fail without all runtimes");
        assert!(matches!(
            first_attempt,
            anneal_core::ApplicationError::Validation(_)
        ));

        let second_attempt = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![
                    RuntimeRegistration {
                        name: "edge-xray".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Xray,
                        protocols: vec![ProtocolKind::VlessReality],
                    },
                    RuntimeRegistration {
                        name: "edge-singbox".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Singbox,
                        protocols: vec![ProtocolKind::VlessReality, ProtocolKind::Tuic],
                    },
                ],
            )
            .await
            .expect("bootstrap retry must succeed");

        assert_eq!(second_attempt.len(), 2);
    }

    #[tokio::test]
    async fn failed_bootstrap_after_partial_registration_rolls_back_created_nodes() {
        let repository = InMemoryNodeRepository::default();
        repository.fail_create_node_on_attempt(2);
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let tenant_id = actor.tenant_id.expect("tenant");
        let group = service
            .create_server_node(&actor, tenant_id, "main".into())
            .await
            .expect("group");
        let bootstrap = service
            .create_bootstrap_token(
                &actor,
                tenant_id,
                group.id,
                "edge".into(),
                vec![ProxyEngine::Xray, ProxyEngine::Singbox],
            )
            .await
            .expect("bootstrap");

        let first_attempt = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![
                    RuntimeRegistration {
                        name: "edge-xray".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Xray,
                        protocols: vec![ProtocolKind::VlessReality],
                    },
                    RuntimeRegistration {
                        name: "edge-singbox".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Singbox,
                        protocols: vec![ProtocolKind::VlessReality, ProtocolKind::Tuic],
                    },
                ],
            )
            .await
            .expect_err("bootstrap must fail after first runtime");
        assert!(matches!(
            first_attempt,
            anneal_core::ApplicationError::Infrastructure(_)
        ));
        assert!(
            repository
                .list_nodes(Some(tenant_id))
                .await
                .expect("nodes after rollback")
                .is_empty()
        );

        repository.clear_create_node_failure();
        let second_attempt = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![
                    RuntimeRegistration {
                        name: "edge-xray".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Xray,
                        protocols: vec![ProtocolKind::VlessReality],
                    },
                    RuntimeRegistration {
                        name: "edge-singbox".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Singbox,
                        protocols: vec![ProtocolKind::VlessReality, ProtocolKind::Tuic],
                    },
                ],
            )
            .await
            .expect("bootstrap retry");
        assert_eq!(second_attempt.len(), 2);
    }

    #[tokio::test]
    async fn bootstrap_reuses_slot_after_stale_pending_runtime_cleanup() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let tenant_id = actor.tenant_id.expect("tenant");
        let group = service
            .create_server_node(&actor, tenant_id, "main".into())
            .await
            .expect("group");
        let stale_id = Uuid::new_v4();
        let stale = repository
            .create_node(
                crate::domain::NodeRuntime {
                    id: stale_id,
                    tenant_id,
                    server_node_id: group.id,
                    name: "edge".into(),
                    engine: ProxyEngine::Xray,
                    version: "0.1.0".into(),
                    status: NodeStatus::Pending,
                    last_seen_at: None,
                    node_token_hash: "stale-token".into(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                &[crate::domain::NodeCapability {
                    node_id: stale_id,
                    protocol: ProtocolKind::VlessReality,
                }],
            )
            .await
            .expect("stale node");
        let bootstrap = service
            .create_bootstrap_token(
                &actor,
                tenant_id,
                group.id,
                "edge".into(),
                vec![ProxyEngine::Xray],
            )
            .await
            .expect("bootstrap");

        let grants = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![RuntimeRegistration {
                    name: "edge".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                }],
            )
            .await
            .expect("bootstrap");
        let nodes = repository
            .list_nodes(Some(tenant_id))
            .await
            .expect("nodes after cleanup");

        assert_eq!(grants.len(), 1);
        assert_eq!(nodes.len(), 1);
        assert_ne!(nodes[0].id, stale.id);
        assert_eq!(nodes[0].name, "edge");
    }

    #[tokio::test]
    async fn bootstrap_retry_recovers_when_endpoint_sync_fails_after_node_insert() {
        let repository = InMemoryNodeRepository::default();
        repository.fail_replace_node_endpoints_on_attempt(3);
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let tenant_id = actor.tenant_id.expect("tenant");
        let group = service
            .create_server_node(&actor, tenant_id, "main".into())
            .await
            .expect("group");
        let bootstrap = service
            .create_bootstrap_token(
                &actor,
                tenant_id,
                group.id,
                "edge".into(),
                vec![ProxyEngine::Xray, ProxyEngine::Singbox],
            )
            .await
            .expect("bootstrap");

        let first_attempt = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![
                    RuntimeRegistration {
                        name: "edge-xray".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Xray,
                        protocols: vec![ProtocolKind::VlessReality],
                    },
                    RuntimeRegistration {
                        name: "edge-singbox".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Singbox,
                        protocols: vec![ProtocolKind::VlessReality, ProtocolKind::Tuic],
                    },
                ],
            )
            .await
            .expect_err("bootstrap must fail on endpoint sync");
        assert!(matches!(
            first_attempt,
            anneal_core::ApplicationError::Infrastructure(_)
        ));
        assert!(
            repository
                .list_nodes(Some(tenant_id))
                .await
                .expect("nodes after failed sync")
                .is_empty()
        );

        repository.clear_replace_node_endpoints_failure();
        let second_attempt = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![
                    RuntimeRegistration {
                        name: "edge-xray".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Xray,
                        protocols: vec![ProtocolKind::VlessReality],
                    },
                    RuntimeRegistration {
                        name: "edge-singbox".into(),
                        version: "1.0.0".into(),
                        engine: ProxyEngine::Singbox,
                        protocols: vec![ProtocolKind::VlessReality, ProtocolKind::Tuic],
                    },
                ],
            )
            .await
            .expect("bootstrap retry");

        assert_eq!(second_attempt.len(), 2);
    }

    #[tokio::test]
    async fn bootstrap_handles_duplicate_generated_endpoint_state_keys() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::with_public_base_url(
            &repository,
            RbacService,
            TokenHasher::new("test-node-seed-hash-key").expect("token hasher"),
            "https://test.aurausa.me/hidden-panel",
        );
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let tenant_id = actor.tenant_id.expect("tenant");
        let group = service
            .create_server_node(&actor, tenant_id, "main".into())
            .await
            .expect("group");
        let bootstrap = service
            .create_bootstrap_token(
                &actor,
                tenant_id,
                group.id,
                "edge-570009".into(),
                vec![ProxyEngine::Xray, ProxyEngine::Singbox],
            )
            .await
            .expect("bootstrap");

        let runtimes = service
            .bootstrap_nodes(
                &bootstrap.bootstrap_token,
                vec![
                    RuntimeRegistration {
                        name: "edge-570009".into(),
                        version: "0.1.0".into(),
                        engine: ProxyEngine::Xray,
                        protocols: vec![
                            ProtocolKind::VlessReality,
                            ProtocolKind::Vmess,
                            ProtocolKind::Trojan,
                            ProtocolKind::Shadowsocks2022,
                        ],
                    },
                    RuntimeRegistration {
                        name: "edge-570009".into(),
                        version: "0.1.0".into(),
                        engine: ProxyEngine::Singbox,
                        protocols: vec![
                            ProtocolKind::VlessReality,
                            ProtocolKind::Vmess,
                            ProtocolKind::Trojan,
                            ProtocolKind::Shadowsocks2022,
                            ProtocolKind::Tuic,
                            ProtocolKind::Hysteria2,
                        ],
                    },
                ],
            )
            .await
            .expect("bootstrap");

        assert_eq!(runtimes.len(), 2);

        let xray = runtimes
            .iter()
            .find(|runtime| runtime.engine == ProxyEngine::Xray)
            .expect("xray runtime");
        let endpoints = repository
            .list_node_endpoints(xray.node_id)
            .await
            .expect("xray endpoints");
        let endpoint_ids = endpoints
            .iter()
            .map(|endpoint| endpoint.id)
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(endpoints.len(), endpoint_ids.len());
        assert!(endpoints.len() > 2);
    }

    #[tokio::test]
    async fn rotating_node_token_invalidates_previous_token() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let enrollment = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &enrollment.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::VlessReality],
                },
            )
            .await
            .expect("register");

        let rotated = service
            .rotate_node_token(node.id, &node.node_token)
            .await
            .expect("rotate");

        let old_error = service
            .heartbeat(node.id, &node.node_token, "1.0.1")
            .await
            .expect_err("old token must fail");
        assert!(matches!(
            old_error,
            anneal_core::ApplicationError::Unauthorized
        ));

        let updated = service
            .heartbeat(rotated.node_id, &rotated.node_token, "1.0.1")
            .await
            .expect("heartbeat");
        assert_eq!(updated.version, "1.0.1");
    }

    #[tokio::test]
    async fn manual_tls_paths_are_overwritten_with_managed_defaults() {
        let repository = InMemoryNodeRepository::default();
        let service = NodeService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        let group = service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let enrollment = service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                ProxyEngine::Xray,
            )
            .await
            .expect("token");
        let node = service
            .register_node(
                &enrollment.token,
                RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: ProxyEngine::Xray,
                    protocols: vec![ProtocolKind::Vmess],
                },
            )
            .await
            .expect("register");

        let endpoints = service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                vec![NodeEndpointDraft {
                    protocol: ProtocolKind::Vmess,
                    listen_host: "0.0.0.0".into(),
                    listen_port: 8443,
                    public_host: "edge.example.com".into(),
                    public_port: 443,
                    transport: anneal_config_engine::TransportKind::Ws,
                    security: SecurityKind::Tls,
                    server_name: Some("edge.example.com".into()),
                    host_header: None,
                    path: Some("/ws".into()),
                    service_name: None,
                    flow: None,
                    reality_public_key: None,
                    reality_private_key: None,
                    reality_short_id: None,
                    fingerprint: Some("chrome".into()),
                    alpn: vec!["http/1.1".into()],
                    cipher: None,
                    tls_certificate_path: Some("/tmp/custom.crt".into()),
                    tls_key_path: Some("/tmp/custom.key".into()),
                    enabled: true,
                }],
            )
            .await
            .expect("replace endpoints");

        assert_eq!(
            endpoints[0].tls_certificate_path.as_deref(),
            Some(super::DEFAULT_TLS_CERTIFICATE_PATH)
        );
        assert_eq!(
            endpoints[0].tls_key_path.as_deref(),
            Some(super::DEFAULT_TLS_KEY_PATH)
        );
    }
}
