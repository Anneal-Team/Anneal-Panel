use std::{
    collections::{HashMap, HashSet},
    sync::RwLock,
};

use anneal_config_engine::{
    ClientCredential, InboundProfile, RenderedShareLink, ShareLinkRenderer, ShareLinkStrategy,
    SubscriptionDocumentFormat, SubscriptionDocumentRenderer,
};
use anneal_core::{Actor, ApplicationError, ApplicationResult, QuotaState, TokenHasher, UserRole};
use anneal_nodes::{DeliveryNodeEndpoint, NodeEndpointCatalog};
use anneal_rbac::{AccessScope, Permission, RbacService};
use async_trait::async_trait;
use chrono::Utc;
use rand::{Rng, distr::Alphanumeric};
use uuid::Uuid;

use crate::domain::{
    CreateDeviceCommand, CreateSubscriptionCommand, Device, RenderedSubscriptionBundle,
    ResolvedSubscriptionContext, Subscription, SubscriptionLink, UpdateSubscriptionCommand,
};

#[async_trait]
pub trait SubscriptionRepository: Send + Sync {
    async fn tenant_owns_user(&self, tenant_id: Uuid, user_id: Uuid) -> ApplicationResult<bool>;
    async fn tenant_owns_device(&self, tenant_id: Uuid, device_id: Uuid)
    -> ApplicationResult<bool>;
    async fn create_device(&self, device: Device) -> ApplicationResult<Device>;
    async fn create_subscription(
        &self,
        device: Device,
        subscription: Subscription,
        link: SubscriptionLink,
    ) -> ApplicationResult<(Subscription, SubscriptionLink)>;
    async fn list_devices(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<Device>>;
    async fn list_subscriptions(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<Subscription>>;
    async fn get_subscription(
        &self,
        subscription_id: Uuid,
    ) -> ApplicationResult<Option<Subscription>>;
    async fn update_subscription(
        &self,
        subscription: Subscription,
    ) -> ApplicationResult<Subscription>;
    async fn delete_subscription(&self, subscription_id: Uuid) -> ApplicationResult<()>;
    async fn rotate_device_token(
        &self,
        device_id: Uuid,
        device_token: &str,
        device_token_hash: &str,
    ) -> ApplicationResult<()>;
    async fn rotate_subscription_token(
        &self,
        subscription_id: Uuid,
        expires_at: chrono::DateTime<Utc>,
    ) -> ApplicationResult<SubscriptionLink>;
    async fn find_by_delivery_token(
        &self,
        link_id: Uuid,
    ) -> ApplicationResult<Option<ResolvedSubscriptionContext>>;
}

#[async_trait]
impl<T> SubscriptionRepository for &T
where
    T: SubscriptionRepository + Send + Sync,
{
    async fn tenant_owns_user(&self, tenant_id: Uuid, user_id: Uuid) -> ApplicationResult<bool> {
        (*self).tenant_owns_user(tenant_id, user_id).await
    }

    async fn tenant_owns_device(
        &self,
        tenant_id: Uuid,
        device_id: Uuid,
    ) -> ApplicationResult<bool> {
        (*self).tenant_owns_device(tenant_id, device_id).await
    }

    async fn create_device(&self, device: Device) -> ApplicationResult<Device> {
        (*self).create_device(device).await
    }

    async fn create_subscription(
        &self,
        device: Device,
        subscription: Subscription,
        link: SubscriptionLink,
    ) -> ApplicationResult<(Subscription, SubscriptionLink)> {
        (*self)
            .create_subscription(device, subscription, link)
            .await
    }

    async fn list_devices(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<Device>> {
        (*self).list_devices(tenant_id).await
    }

    async fn list_subscriptions(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<Subscription>> {
        (*self).list_subscriptions(tenant_id).await
    }

    async fn get_subscription(
        &self,
        subscription_id: Uuid,
    ) -> ApplicationResult<Option<Subscription>> {
        (*self).get_subscription(subscription_id).await
    }

    async fn update_subscription(
        &self,
        subscription: Subscription,
    ) -> ApplicationResult<Subscription> {
        (*self).update_subscription(subscription).await
    }

    async fn delete_subscription(&self, subscription_id: Uuid) -> ApplicationResult<()> {
        (*self).delete_subscription(subscription_id).await
    }

    async fn rotate_device_token(
        &self,
        device_id: Uuid,
        device_token: &str,
        device_token_hash: &str,
    ) -> ApplicationResult<()> {
        (*self)
            .rotate_device_token(device_id, device_token, device_token_hash)
            .await
    }

    async fn rotate_subscription_token(
        &self,
        subscription_id: Uuid,
        expires_at: chrono::DateTime<Utc>,
    ) -> ApplicationResult<SubscriptionLink> {
        (*self)
            .rotate_subscription_token(subscription_id, expires_at)
            .await
    }

    async fn find_by_delivery_token(
        &self,
        link_id: Uuid,
    ) -> ApplicationResult<Option<ResolvedSubscriptionContext>> {
        (*self).find_by_delivery_token(link_id).await
    }
}

pub struct SubscriptionService<R> {
    repository: R,
    rbac: RbacService,
    token_hasher: TokenHasher,
}

impl<R> SubscriptionService<R> {
    pub fn new(repository: R, rbac: RbacService) -> Self {
        Self::with_token_hasher(
            repository,
            rbac,
            TokenHasher::new("anneal-subscription-default-token-hash-key").expect("token hasher"),
        )
    }

    pub fn with_token_hasher(repository: R, rbac: RbacService, token_hasher: TokenHasher) -> Self {
        Self {
            repository,
            rbac,
            token_hasher,
        }
    }
}

impl<R> SubscriptionService<R>
where
    R: SubscriptionRepository,
{
    pub async fn create_device(
        &self,
        actor: &Actor,
        command: CreateDeviceCommand,
    ) -> ApplicationResult<Device> {
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: Some(command.tenant_id),
            },
        )?;
        if !self
            .repository
            .tenant_owns_user(command.tenant_id, command.user_id)
            .await?
        {
            return Err(ApplicationError::Forbidden);
        }
        let now = Utc::now();
        let device_token = generate_token();
        self.repository
            .create_device(Device {
                id: Uuid::new_v4(),
                tenant_id: command.tenant_id,
                user_id: command.user_id,
                name: command.name,
                device_token_hash: self.token_hasher.hash(&device_token),
                device_token,
                suspended: false,
                created_at: now,
                updated_at: now,
            })
            .await
    }

    pub async fn create_subscription(
        &self,
        actor: &Actor,
        command: CreateSubscriptionCommand,
    ) -> ApplicationResult<(Subscription, SubscriptionLink)> {
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: Some(command.tenant_id),
            },
        )?;
        if !self
            .repository
            .tenant_owns_user(command.tenant_id, command.user_id)
            .await?
        {
            return Err(ApplicationError::Forbidden);
        }
        let now = Utc::now();
        let device_token = generate_token();
        let device = Device {
            id: Uuid::new_v4(),
            tenant_id: command.tenant_id,
            user_id: command.user_id,
            name: format!("{} access", command.name),
            device_token_hash: self.token_hasher.hash(&device_token),
            device_token,
            suspended: false,
            created_at: now,
            updated_at: now,
        };
        let subscription = Subscription {
            id: Uuid::new_v4(),
            tenant_id: command.tenant_id,
            user_id: command.user_id,
            device_id: device.id,
            name: command.name,
            note: command.note,
            access_key: generate_token(),
            traffic_limit_bytes: command.traffic_limit_bytes,
            used_bytes: 0,
            quota_state: QuotaState::Normal,
            suspended: false,
            expires_at: command.expires_at,
            created_at: now,
            updated_at: now,
            current_token: None,
        };
        let link = SubscriptionLink {
            id: Uuid::new_v4(),
            subscription_id: subscription.id,
            token: String::new(),
            token_hash: String::new(),
            expires_at: subscription.expires_at,
            revoked_at: None,
            created_at: now,
        };
        let mut subscription = subscription;
        subscription.current_token = Some(link.id.to_string());
        self.repository
            .create_subscription(device, subscription, link)
            .await
    }

    pub async fn list_subscriptions(&self, actor: &Actor) -> ApplicationResult<Vec<Subscription>> {
        let tenant_id = if actor.role == UserRole::Reseller {
            actor.tenant_id
        } else {
            None
        };
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        self.repository.list_subscriptions(tenant_id).await
    }

    pub async fn list_devices(&self, actor: &Actor) -> ApplicationResult<Vec<Device>> {
        let tenant_id = if actor.role == UserRole::Reseller {
            actor.tenant_id
        } else {
            None
        };
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: tenant_id,
            },
        )?;
        self.repository.list_devices(tenant_id).await
    }

    pub async fn update_subscription(
        &self,
        actor: &Actor,
        subscription_id: Uuid,
        command: UpdateSubscriptionCommand,
    ) -> ApplicationResult<Subscription> {
        let mut subscription = self
            .repository
            .get_subscription(subscription_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("subscription not found".into()))?;
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: Some(subscription.tenant_id),
            },
        )?;
        subscription.name = command.name;
        subscription.note = command.note;
        subscription.traffic_limit_bytes = command.traffic_limit_bytes;
        subscription.expires_at = command.expires_at;
        subscription.suspended = command.suspended;
        subscription.quota_state =
            decide_quota_state(subscription.traffic_limit_bytes, subscription.used_bytes);
        subscription.updated_at = Utc::now();
        self.repository.update_subscription(subscription).await
    }

    pub async fn delete_subscription(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        subscription_id: Uuid,
    ) -> ApplicationResult<()> {
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let subscription = self
            .repository
            .get_subscription(subscription_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("subscription not found".into()))?;
        if subscription.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        self.repository.delete_subscription(subscription_id).await
    }

    pub async fn rotate_device_token(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        device_id: Uuid,
    ) -> ApplicationResult<String> {
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        if !self
            .repository
            .tenant_owns_device(tenant_id, device_id)
            .await?
        {
            return Err(ApplicationError::Forbidden);
        }
        let token = generate_token();
        let token_hash = self.token_hasher.hash(&token);
        self.repository
            .rotate_device_token(device_id, &token, &token_hash)
            .await?;
        Ok(token)
    }

    pub async fn rotate_subscription_token(
        &self,
        actor: &Actor,
        tenant_id: Uuid,
        subscription_id: Uuid,
    ) -> ApplicationResult<SubscriptionLink> {
        self.rbac.authorize(
            actor,
            Permission::ManageSubscriptions,
            AccessScope {
                target_tenant_id: Some(tenant_id),
            },
        )?;
        let subscription = self
            .repository
            .get_subscription(subscription_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("subscription not found".into()))?;
        if subscription.tenant_id != tenant_id {
            return Err(ApplicationError::Forbidden);
        }
        self.repository
            .rotate_subscription_token(subscription_id, subscription.expires_at)
            .await
    }

    pub async fn resolve_subscription(
        &self,
        link_token: &str,
    ) -> ApplicationResult<ResolvedSubscriptionContext> {
        let link_id = parse_link_token(link_token)?;
        let context = self
            .repository
            .find_by_delivery_token(link_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("subscription not found".into()))?;
        if context.subscription.suspended || context.subscription.expires_at <= Utc::now() {
            return Err(ApplicationError::Forbidden);
        }
        Ok(context)
    }
}

pub struct UnifiedSubscriptionService<R, C> {
    subscriptions: R,
    catalog: C,
}

impl<R, C> UnifiedSubscriptionService<R, C> {
    pub fn new(subscriptions: R, catalog: C) -> Self {
        Self::with_token_hasher(
            subscriptions,
            catalog,
            TokenHasher::new("anneal-subscription-default-token-hash-key").expect("token hasher"),
        )
    }

    pub fn with_token_hasher(subscriptions: R, catalog: C, token_hasher: TokenHasher) -> Self {
        let _ = token_hasher;
        Self {
            subscriptions,
            catalog,
        }
    }
}

impl<R, C> UnifiedSubscriptionService<R, C>
where
    R: SubscriptionRepository,
    C: NodeEndpointCatalog,
{
    pub async fn render_bundle(
        &self,
        link_token: &str,
        format: SubscriptionDocumentFormat,
    ) -> ApplicationResult<RenderedSubscriptionBundle> {
        let link_id = parse_link_token(link_token)?;
        let context = self
            .subscriptions
            .find_by_delivery_token(link_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("subscription not found".into()))?;
        if context.subscription.suspended || context.subscription.expires_at <= Utc::now() {
            return Err(ApplicationError::Forbidden);
        }
        let endpoints = self
            .catalog
            .list_delivery_endpoints(context.subscription.tenant_id)
            .await?;
        if endpoints.is_empty() {
            return Err(ApplicationError::NotFound(
                "no online node endpoints available".into(),
            ));
        }
        let mut endpoints = endpoints;
        endpoints.sort_by(|left, right| {
            protocol_order(left.protocol)
                .cmp(&protocol_order(right.protocol))
                .then_with(|| left.public_host.cmp(&right.public_host))
                .then_with(|| left.public_port.cmp(&right.public_port))
        });
        let credential = ClientCredential {
            email: context.subscription.name.clone(),
            uuid: context.subscription.id.to_string(),
            password: Some(context.subscription.access_key.clone()),
        };
        let rendered_links = endpoints
            .iter()
            .map(|endpoint| {
                let profile = map_delivery_endpoint(endpoint)?;
                let label = format!(
                    "{} {} {} {}",
                    context.subscription.name,
                    endpoint.node_name,
                    protocol_name(endpoint.protocol),
                    endpoint.public_port
                );
                let uri = ShareLinkRenderer.render(&profile, &credential, &label)?;
                Ok(RenderedShareLink {
                    label,
                    uri,
                    profile,
                    credential: credential.clone(),
                })
            })
            .collect::<ApplicationResult<Vec<_>>>()?;
        let rendered = SubscriptionDocumentRenderer.render(&rendered_links, format)?;
        Ok(RenderedSubscriptionBundle {
            content: rendered.content,
            links_count: rendered_links.len(),
            content_type: rendered.content_type.to_string(),
        })
    }
}

#[derive(Default)]
pub struct InMemorySubscriptionRepository {
    devices: RwLock<HashMap<Uuid, Device>>,
    subscriptions: RwLock<HashMap<Uuid, Subscription>>,
    links: RwLock<HashMap<Uuid, SubscriptionLink>>,
    tenant_users: RwLock<HashSet<(Uuid, Uuid)>>,
}

impl InMemorySubscriptionRepository {
    pub fn allow_user(&self, tenant_id: Uuid, user_id: Uuid) {
        self.tenant_users
            .write()
            .expect("lock")
            .insert((tenant_id, user_id));
    }
}

#[async_trait]
impl SubscriptionRepository for InMemorySubscriptionRepository {
    async fn tenant_owns_user(&self, tenant_id: Uuid, user_id: Uuid) -> ApplicationResult<bool> {
        Ok(self
            .tenant_users
            .read()
            .expect("lock")
            .contains(&(tenant_id, user_id)))
    }

    async fn tenant_owns_device(
        &self,
        tenant_id: Uuid,
        device_id: Uuid,
    ) -> ApplicationResult<bool> {
        Ok(self
            .devices
            .read()
            .expect("lock")
            .get(&device_id)
            .is_some_and(|device| device.tenant_id == tenant_id))
    }

    async fn create_device(&self, device: Device) -> ApplicationResult<Device> {
        self.tenant_users
            .write()
            .expect("lock")
            .insert((device.tenant_id, device.user_id));
        self.devices
            .write()
            .expect("lock")
            .insert(device.id, device.clone());
        Ok(device)
    }

    async fn create_subscription(
        &self,
        device: Device,
        subscription: Subscription,
        link: SubscriptionLink,
    ) -> ApplicationResult<(Subscription, SubscriptionLink)> {
        self.tenant_users
            .write()
            .expect("lock")
            .insert((device.tenant_id, device.user_id));
        self.devices
            .write()
            .expect("lock")
            .insert(device.id, device);
        self.subscriptions
            .write()
            .expect("lock")
            .insert(subscription.id, subscription.clone());
        self.links
            .write()
            .expect("lock")
            .insert(link.id, link.clone());
        Ok((subscription, link))
    }

    async fn list_devices(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<Device>> {
        Ok(self
            .devices
            .read()
            .expect("lock")
            .values()
            .filter(|device| tenant_id.is_none() || Some(device.tenant_id) == tenant_id)
            .cloned()
            .collect())
    }

    async fn list_subscriptions(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<Subscription>> {
        let links = self.links.read().expect("lock");
        Ok(self
            .subscriptions
            .read()
            .expect("lock")
            .values()
            .filter(|subscription| tenant_id.is_none() || Some(subscription.tenant_id) == tenant_id)
            .map(|subscription| {
                let mut subscription = subscription.clone();
                subscription.current_token = links
                    .values()
                    .filter(|link| {
                        link.subscription_id == subscription.id
                            && link.revoked_at.is_none()
                            && link.expires_at > Utc::now()
                    })
                    .max_by(|left, right| left.created_at.cmp(&right.created_at))
                    .map(|link| link.id.to_string());
                subscription
            })
            .collect())
    }

    async fn get_subscription(
        &self,
        subscription_id: Uuid,
    ) -> ApplicationResult<Option<Subscription>> {
        Ok(self
            .subscriptions
            .read()
            .expect("lock")
            .get(&subscription_id)
            .cloned())
    }

    async fn update_subscription(
        &self,
        subscription: Subscription,
    ) -> ApplicationResult<Subscription> {
        let mut links = self.links.write().expect("lock");
        for link in links
            .values_mut()
            .filter(|link| link.subscription_id == subscription.id && link.revoked_at.is_none())
        {
            link.expires_at = subscription.expires_at;
        }
        self.subscriptions
            .write()
            .expect("lock")
            .insert(subscription.id, subscription.clone());
        Ok(subscription)
    }

    async fn delete_subscription(&self, subscription_id: Uuid) -> ApplicationResult<()> {
        let device_id = self
            .subscriptions
            .write()
            .expect("lock")
            .remove(&subscription_id)
            .map(|subscription| subscription.device_id);
        self.links
            .write()
            .expect("lock")
            .retain(|_, link| link.subscription_id != subscription_id);
        if let Some(device_id) = device_id {
            let still_used = self
                .subscriptions
                .read()
                .expect("lock")
                .values()
                .any(|subscription| subscription.device_id == device_id);
            if !still_used {
                self.devices.write().expect("lock").remove(&device_id);
            }
        }
        Ok(())
    }

    async fn rotate_device_token(
        &self,
        device_id: Uuid,
        device_token: &str,
        device_token_hash: &str,
    ) -> ApplicationResult<()> {
        let mut devices = self.devices.write().expect("lock");
        let device = devices
            .get_mut(&device_id)
            .ok_or_else(|| ApplicationError::NotFound("device not found".into()))?;
        device.device_token = device_token.into();
        device.device_token_hash = device_token_hash.into();
        device.updated_at = Utc::now();
        Ok(())
    }

    async fn rotate_subscription_token(
        &self,
        subscription_id: Uuid,
        expires_at: chrono::DateTime<Utc>,
    ) -> ApplicationResult<SubscriptionLink> {
        let mut links = self.links.write().expect("lock");
        if let Some(current) = links
            .values_mut()
            .find(|link| link.subscription_id == subscription_id && link.revoked_at.is_none())
        {
            current.revoked_at = Some(Utc::now());
        }
        let link = SubscriptionLink {
            id: Uuid::new_v4(),
            subscription_id,
            token: String::new(),
            token_hash: String::new(),
            expires_at,
            revoked_at: None,
            created_at: Utc::now(),
        };
        links.insert(link.id, link.clone());
        if let Some(subscription) = self
            .subscriptions
            .write()
            .expect("lock")
            .get_mut(&subscription_id)
        {
            subscription.current_token = Some(link.id.to_string());
            subscription.updated_at = Utc::now();
        }
        Ok(link)
    }

    async fn find_by_delivery_token(
        &self,
        link_id: Uuid,
    ) -> ApplicationResult<Option<ResolvedSubscriptionContext>> {
        let links = self.links.read().expect("lock");
        let subscriptions = self.subscriptions.read().expect("lock");
        let devices = self.devices.read().expect("lock");
        let found = links.get(&link_id).and_then(|link| {
            if link.revoked_at.is_some() || link.expires_at <= Utc::now() {
                return None;
            }
            subscriptions
                .get(&link.subscription_id)
                .cloned()
                .filter(|subscription| {
                    devices
                        .get(&subscription.device_id)
                        .is_some_and(|device| !device.suspended)
                })
                .map(|subscription| ResolvedSubscriptionContext {
                    subscription,
                    link: link.clone(),
                })
        });
        Ok(found)
    }
}

pub fn generate_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

fn parse_link_token(link_token: &str) -> ApplicationResult<Uuid> {
    Uuid::parse_str(link_token)
        .map_err(|_| ApplicationError::NotFound("subscription not found".into()))
}

fn decide_quota_state(traffic_limit_bytes: i64, used_bytes: i64) -> QuotaState {
    let ratio = if traffic_limit_bytes > 0 {
        used_bytes as f64 / traffic_limit_bytes as f64
    } else {
        1.0
    };
    if ratio >= 1.0 {
        QuotaState::Exhausted
    } else if ratio >= 0.95 {
        QuotaState::Warning95
    } else if ratio >= 0.80 {
        QuotaState::Warning80
    } else {
        QuotaState::Normal
    }
}

fn map_delivery_endpoint(endpoint: &DeliveryNodeEndpoint) -> ApplicationResult<InboundProfile> {
    let listen_port = u16::try_from(endpoint.listen_port)
        .map_err(|_| ApplicationError::Validation("invalid listen_port".into()))?;
    let public_port = u16::try_from(endpoint.public_port)
        .map_err(|_| ApplicationError::Validation("invalid public_port".into()))?;
    Ok(InboundProfile {
        protocol: endpoint.protocol,
        listen_host: endpoint.listen_host.clone(),
        listen_port,
        public_host: endpoint.public_host.clone(),
        public_port,
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
    })
}

fn protocol_name(protocol: anneal_core::ProtocolKind) -> &'static str {
    match protocol {
        anneal_core::ProtocolKind::VlessReality => "vless",
        anneal_core::ProtocolKind::Vmess => "vmess",
        anneal_core::ProtocolKind::Trojan => "trojan",
        anneal_core::ProtocolKind::Shadowsocks2022 => "ss2022",
        anneal_core::ProtocolKind::Tuic => "tuic",
        anneal_core::ProtocolKind::Hysteria2 => "hy2",
    }
}

fn protocol_order(protocol: anneal_core::ProtocolKind) -> usize {
    match protocol {
        anneal_core::ProtocolKind::VlessReality => 0,
        anneal_core::ProtocolKind::Vmess => 1,
        anneal_core::ProtocolKind::Trojan => 2,
        anneal_core::ProtocolKind::Shadowsocks2022 => 3,
        anneal_core::ProtocolKind::Tuic => 4,
        anneal_core::ProtocolKind::Hysteria2 => 5,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use anneal_config_engine::SubscriptionDocumentFormat;
    use anneal_core::{Actor, ApplicationError, ProtocolKind, UserRole};
    use anneal_nodes::{InMemoryNodeRepository, NodeEndpointDraft, NodeService};
    use anneal_rbac::RbacService;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use crate::{
        application::{
            InMemorySubscriptionRepository, SubscriptionService, UnifiedSubscriptionService,
        },
        domain::CreateSubscriptionCommand,
    };

    #[tokio::test]
    async fn rotating_subscription_token_revokes_previous_link() {
        let repository = InMemorySubscriptionRepository::default();
        let service = SubscriptionService::new(repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        service
            .repository
            .allow_user(actor.tenant_id.expect("tenant"), actor.user_id);
        let (subscription, first_link) = service
            .create_subscription(
                &actor,
                CreateSubscriptionCommand {
                    tenant_id: actor.tenant_id.expect("tenant"),
                    user_id: actor.user_id,
                    name: "main".into(),
                    note: None,
                    traffic_limit_bytes: 1024,
                    expires_at: Utc::now() + Duration::days(30),
                },
            )
            .await
            .expect("subscription");
        let rotated = service
            .rotate_subscription_token(&actor, actor.tenant_id.expect("tenant"), subscription.id)
            .await
            .expect("rotated");
        let first_token = first_link.id.to_string();
        let error = service
            .resolve_subscription(&first_token)
            .await
            .expect_err("revoked link must be rejected");

        assert_ne!(first_link.id, rotated.id);
        assert!(matches!(error, ApplicationError::NotFound(_)));
    }

    #[tokio::test]
    async fn unified_bundle_contains_multiple_protocols() {
        let subscriptions = InMemorySubscriptionRepository::default();
        let subscription_service = SubscriptionService::new(&subscriptions, RbacService);
        let nodes = InMemoryNodeRepository::default();
        let node_service = NodeService::new(&nodes, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        subscriptions.allow_user(actor.tenant_id.expect("tenant"), actor.user_id);

        let group = node_service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let grant = node_service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                anneal_core::ProxyEngine::Singbox,
            )
            .await
            .expect("grant");
        let node = node_service
            .register_node(
                &grant.token,
                anneal_nodes::RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: anneal_core::ProxyEngine::Singbox,
                    protocols: vec![ProtocolKind::VlessReality, ProtocolKind::Tuic],
                },
            )
            .await
            .expect("node");
        node_service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                vec![
                    NodeEndpointDraft {
                        protocol: ProtocolKind::VlessReality,
                        listen_host: "::".into(),
                        listen_port: 443,
                        public_host: "edge.example.com".into(),
                        public_port: 443,
                        transport: anneal_config_engine::TransportKind::Tcp,
                        security: anneal_config_engine::SecurityKind::Reality,
                        server_name: Some("edge.example.com".into()),
                        host_header: None,
                        path: None,
                        service_name: None,
                        flow: Some("xtls-rprx-vision".into()),
                        reality_public_key: Some("public-key".into()),
                        reality_private_key: Some("private-key".into()),
                        reality_short_id: Some("deadbeef".into()),
                        fingerprint: Some("chrome".into()),
                        alpn: vec!["h2".into()],
                        cipher: None,
                        tls_certificate_path: None,
                        tls_key_path: None,
                        enabled: true,
                    },
                    NodeEndpointDraft {
                        protocol: ProtocolKind::Tuic,
                        listen_host: "::".into(),
                        listen_port: 8443,
                        public_host: "edge.example.com".into(),
                        public_port: 8443,
                        transport: anneal_config_engine::TransportKind::Tcp,
                        security: anneal_config_engine::SecurityKind::Tls,
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
                    },
                ],
            )
            .await
            .expect("endpoints");

        let (_subscription, link) = subscription_service
            .create_subscription(
                &actor,
                CreateSubscriptionCommand {
                    tenant_id: actor.tenant_id.expect("tenant"),
                    user_id: actor.user_id,
                    name: "main".into(),
                    note: None,
                    traffic_limit_bytes: 1_000_000,
                    expires_at: Utc::now() + Duration::days(30),
                },
            )
            .await
            .expect("subscription");
        let link_token = link.id.to_string();

        let unified = UnifiedSubscriptionService::new(&subscriptions, &nodes)
            .render_bundle(&link_token, SubscriptionDocumentFormat::Raw)
            .await
            .expect("bundle");

        assert_eq!(unified.links_count, 2);
        assert!(unified.content.contains("vless://"));
        assert!(unified.content.contains("tuic://"));
    }

    #[tokio::test]
    async fn singbox_bundle_uses_unique_tags_for_same_protocol_endpoints() {
        let subscriptions = InMemorySubscriptionRepository::default();
        let subscription_service = SubscriptionService::new(&subscriptions, RbacService);
        let nodes = InMemoryNodeRepository::default();
        let node_service = NodeService::new(&nodes, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };
        subscriptions.allow_user(actor.tenant_id.expect("tenant"), actor.user_id);

        let group = node_service
            .create_server_node(&actor, actor.tenant_id.expect("tenant"), "main".into())
            .await
            .expect("group");
        let grant = node_service
            .create_enrollment_token(
                &actor,
                actor.tenant_id.expect("tenant"),
                group.id,
                anneal_core::ProxyEngine::Singbox,
            )
            .await
            .expect("grant");
        let node = node_service
            .register_node(
                &grant.token,
                anneal_nodes::RuntimeRegistration {
                    name: "edge-1".into(),
                    version: "1.0.0".into(),
                    engine: anneal_core::ProxyEngine::Singbox,
                    protocols: vec![ProtocolKind::Vmess],
                },
            )
            .await
            .expect("node");
        node_service
            .replace_node_endpoints(
                &actor,
                actor.tenant_id.expect("tenant"),
                node.id,
                vec![
                    NodeEndpointDraft {
                        protocol: ProtocolKind::Vmess,
                        listen_host: "::".into(),
                        listen_port: 24443,
                        public_host: "edge.example.com".into(),
                        public_port: 24443,
                        transport: anneal_config_engine::TransportKind::Tcp,
                        security: anneal_config_engine::SecurityKind::None,
                        server_name: None,
                        host_header: None,
                        path: None,
                        service_name: None,
                        flow: None,
                        reality_public_key: None,
                        reality_private_key: None,
                        reality_short_id: None,
                        fingerprint: None,
                        alpn: vec![],
                        cipher: None,
                        tls_certificate_path: None,
                        tls_key_path: None,
                        enabled: true,
                    },
                    NodeEndpointDraft {
                        protocol: ProtocolKind::Vmess,
                        listen_host: "::".into(),
                        listen_port: 24444,
                        public_host: "edge.example.com".into(),
                        public_port: 24444,
                        transport: anneal_config_engine::TransportKind::Tcp,
                        security: anneal_config_engine::SecurityKind::None,
                        server_name: None,
                        host_header: None,
                        path: None,
                        service_name: None,
                        flow: None,
                        reality_public_key: None,
                        reality_private_key: None,
                        reality_short_id: None,
                        fingerprint: None,
                        alpn: vec![],
                        cipher: None,
                        tls_certificate_path: None,
                        tls_key_path: None,
                        enabled: true,
                    },
                ],
            )
            .await
            .expect("endpoints");

        let (_subscription, link) = subscription_service
            .create_subscription(
                &actor,
                CreateSubscriptionCommand {
                    tenant_id: actor.tenant_id.expect("tenant"),
                    user_id: actor.user_id,
                    name: "main".into(),
                    note: None,
                    traffic_limit_bytes: 1_000_000,
                    expires_at: Utc::now() + Duration::days(30),
                },
            )
            .await
            .expect("subscription");
        let link_token = link.id.to_string();

        let unified = UnifiedSubscriptionService::new(&subscriptions, &nodes)
            .render_bundle(&link_token, SubscriptionDocumentFormat::SingBox)
            .await
            .expect("bundle");

        let json: serde_json::Value = serde_json::from_str(&unified.content).expect("json");
        let tags = json["outbounds"]
            .as_array()
            .expect("outbounds")
            .iter()
            .filter_map(|entry| entry["tag"].as_str())
            .filter(|tag| tag.starts_with("main edge-1 vmess"))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let unique = tags.iter().cloned().collect::<HashSet<_>>();

        assert_eq!(tags.len(), 2);
        assert_eq!(unique.len(), 2);
        assert!(unique.contains("main edge-1 vmess 24443"));
        assert!(unique.contains("main edge-1 vmess 24444"));
    }

    #[tokio::test]
    async fn reseller_cannot_create_subscription_for_foreign_user() {
        let repository = InMemorySubscriptionRepository::default();
        let service = SubscriptionService::new(&repository, RbacService);
        let actor = Actor {
            user_id: Uuid::new_v4(),
            tenant_id: Some(Uuid::new_v4()),
            role: UserRole::Reseller,
        };

        let error = service
            .create_subscription(
                &actor,
                CreateSubscriptionCommand {
                    tenant_id: actor.tenant_id.expect("tenant"),
                    user_id: Uuid::new_v4(),
                    name: "main".into(),
                    note: None,
                    traffic_limit_bytes: 1024,
                    expires_at: Utc::now() + Duration::days(30),
                },
            )
            .await
            .expect_err("foreign user must be rejected");

        assert!(matches!(error, ApplicationError::Forbidden));
    }
}
