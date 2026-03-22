use std::{collections::HashMap, sync::RwLock};

use anneal_config_engine::{
    ClientCredential, InboundProfile, RenderedShareLink, ShareLinkRenderer, ShareLinkStrategy,
    SubscriptionDocumentFormat, SubscriptionDocumentRenderer,
};
use anneal_core::{Actor, ApplicationError, ApplicationResult, QuotaState, UserRole};
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
    async fn get_subscription(&self, subscription_id: Uuid) -> ApplicationResult<Option<Subscription>>;
    async fn update_subscription(&self, subscription: Subscription) -> ApplicationResult<Subscription>;
    async fn delete_subscription(&self, subscription_id: Uuid) -> ApplicationResult<()>;
    async fn rotate_device_token(
        &self,
        device_id: Uuid,
        device_token: &str,
    ) -> ApplicationResult<()>;
    async fn rotate_subscription_token(
        &self,
        subscription_id: Uuid,
        token: &str,
    ) -> ApplicationResult<SubscriptionLink>;
    async fn find_by_subscription_token(
        &self,
        token: &str,
    ) -> ApplicationResult<Option<ResolvedSubscriptionContext>>;
}

#[async_trait]
impl<T> SubscriptionRepository for &T
where
    T: SubscriptionRepository + Send + Sync,
{
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

    async fn get_subscription(&self, subscription_id: Uuid) -> ApplicationResult<Option<Subscription>> {
        (*self).get_subscription(subscription_id).await
    }

    async fn update_subscription(&self, subscription: Subscription) -> ApplicationResult<Subscription> {
        (*self).update_subscription(subscription).await
    }

    async fn delete_subscription(&self, subscription_id: Uuid) -> ApplicationResult<()> {
        (*self).delete_subscription(subscription_id).await
    }

    async fn rotate_device_token(
        &self,
        device_id: Uuid,
        device_token: &str,
    ) -> ApplicationResult<()> {
        (*self).rotate_device_token(device_id, device_token).await
    }

    async fn rotate_subscription_token(
        &self,
        subscription_id: Uuid,
        token: &str,
    ) -> ApplicationResult<SubscriptionLink> {
        (*self)
            .rotate_subscription_token(subscription_id, token)
            .await
    }

    async fn find_by_subscription_token(
        &self,
        token: &str,
    ) -> ApplicationResult<Option<ResolvedSubscriptionContext>> {
        (*self).find_by_subscription_token(token).await
    }
}

pub struct SubscriptionService<R> {
    repository: R,
    rbac: RbacService,
}

impl<R> SubscriptionService<R> {
    pub fn new(repository: R, rbac: RbacService) -> Self {
        Self { repository, rbac }
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
        let now = Utc::now();
        self.repository
            .create_device(Device {
                id: Uuid::new_v4(),
                tenant_id: command.tenant_id,
                user_id: command.user_id,
                name: command.name,
                device_token: generate_token(),
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
        let now = Utc::now();
        let device = Device {
            id: Uuid::new_v4(),
            tenant_id: command.tenant_id,
            user_id: command.user_id,
            name: format!("{} access", command.name),
            device_token: generate_token(),
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
            token: generate_token(),
            revoked_at: None,
            created_at: now,
        };
        let mut subscription = subscription;
        subscription.current_token = Some(link.token.clone());
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
        subscription.quota_state = decide_quota_state(
            subscription.traffic_limit_bytes,
            subscription.used_bytes,
        );
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
        let token = generate_token();
        self.repository
            .rotate_device_token(device_id, &token)
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
        self.repository
            .rotate_subscription_token(subscription_id, &generate_token())
            .await
    }

    pub async fn resolve_subscription(
        &self,
        subscription_token: &str,
    ) -> ApplicationResult<ResolvedSubscriptionContext> {
        let context = self
            .repository
            .find_by_subscription_token(subscription_token)
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
        subscription_token: &str,
        _device_token: Option<&str>,
        format: SubscriptionDocumentFormat,
    ) -> ApplicationResult<RenderedSubscriptionBundle> {
        let context = self
            .subscriptions
            .find_by_subscription_token(subscription_token)
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
}

#[async_trait]
impl SubscriptionRepository for InMemorySubscriptionRepository {
    async fn create_device(&self, device: Device) -> ApplicationResult<Device> {
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
                        link.subscription_id == subscription.id && link.revoked_at.is_none()
                    })
                    .max_by(|left, right| left.created_at.cmp(&right.created_at))
                    .map(|link| link.token.clone());
                subscription
            })
            .collect())
    }

    async fn get_subscription(&self, subscription_id: Uuid) -> ApplicationResult<Option<Subscription>> {
        Ok(self
            .subscriptions
            .read()
            .expect("lock")
            .get(&subscription_id)
            .cloned())
    }

    async fn update_subscription(&self, subscription: Subscription) -> ApplicationResult<Subscription> {
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
    ) -> ApplicationResult<()> {
        let mut devices = self.devices.write().expect("lock");
        let device = devices
            .get_mut(&device_id)
            .ok_or_else(|| ApplicationError::NotFound("device not found".into()))?;
        device.device_token = device_token.into();
        device.updated_at = Utc::now();
        Ok(())
    }

    async fn rotate_subscription_token(
        &self,
        subscription_id: Uuid,
        token: &str,
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
            token: token.into(),
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
            subscription.current_token = Some(link.token.clone());
            subscription.updated_at = Utc::now();
        }
        Ok(link)
    }

    async fn find_by_subscription_token(
        &self,
        token: &str,
    ) -> ApplicationResult<Option<ResolvedSubscriptionContext>> {
        let links = self.links.read().expect("lock");
        let subscriptions = self.subscriptions.read().expect("lock");
        let found = links
            .values()
            .find(|link| link.token == token && link.revoked_at.is_none())
            .and_then(|link| {
                subscriptions
                    .get(&link.subscription_id)
                    .cloned()
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
    use anneal_core::{Actor, ProtocolKind, UserRole};
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
        let (subscription, first_link) = service
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
            .expect("subscription");
        let rotated = service
            .rotate_subscription_token(&actor, actor.tenant_id.expect("tenant"), subscription.id)
            .await
            .expect("rotated");

        assert_ne!(first_link.token, rotated.token);
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

        let group = node_service
            .create_node_group(&actor, actor.tenant_id.expect("tenant"), "main".into())
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
                anneal_nodes::NodeRegistration {
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
                    user_id: Uuid::new_v4(),
                    name: "main".into(),
                    note: None,
                    traffic_limit_bytes: 1_000_000,
                    expires_at: Utc::now() + Duration::days(30),
                },
            )
            .await
            .expect("subscription");

        let unified = UnifiedSubscriptionService::new(&subscriptions, &nodes)
            .render_bundle(&link.token, None, SubscriptionDocumentFormat::Raw)
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

        let group = node_service
            .create_node_group(&actor, actor.tenant_id.expect("tenant"), "main".into())
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
                anneal_nodes::NodeRegistration {
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
                    user_id: Uuid::new_v4(),
                    name: "main".into(),
                    note: None,
                    traffic_limit_bytes: 1_000_000,
                    expires_at: Utc::now() + Duration::days(30),
                },
            )
            .await
            .expect("subscription");

        let unified = UnifiedSubscriptionService::new(&subscriptions, &nodes)
            .render_bundle(&link.token, None, SubscriptionDocumentFormat::SingBox)
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
}
