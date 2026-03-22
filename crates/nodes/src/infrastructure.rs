use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use anneal_core::{ApplicationError, ApplicationResult, DeploymentStatus, NodeStatus, SecretBox};

use crate::{
    application::NodeRepository,
    domain::{
        ConfigRevision, DeliveryNodeEndpoint, DeploymentRollout, Node, NodeCapability,
        NodeEndpoint, NodeEnrollmentToken, NodeGroup, NodeGroupDomain,
    },
};

#[derive(Clone)]
pub struct PgNodeRepository {
    pool: PgPool,
    secret_box: SecretBox,
}

impl PgNodeRepository {
    pub fn new(pool: PgPool, secret_box: SecretBox) -> Self {
        Self { pool, secret_box }
    }

    fn decrypt_endpoint(&self, mut endpoint: NodeEndpoint) -> ApplicationResult<NodeEndpoint> {
        endpoint.reality_private_key = self
            .secret_box
            .decrypt_option(endpoint.reality_private_key.as_deref())?;
        Ok(endpoint)
    }

    fn decrypt_delivery_endpoint(
        &self,
        mut endpoint: DeliveryNodeEndpoint,
    ) -> ApplicationResult<DeliveryNodeEndpoint> {
        endpoint.reality_private_key = self
            .secret_box
            .decrypt_option(endpoint.reality_private_key.as_deref())?;
        Ok(endpoint)
    }

    fn decrypt_rollout(
        &self,
        mut rollout: DeploymentRollout,
    ) -> ApplicationResult<DeploymentRollout> {
        rollout.rendered_config = self.secret_box.decrypt(&rollout.rendered_config)?;
        Ok(rollout)
    }
}

#[async_trait]
impl NodeRepository for PgNodeRepository {
    async fn create_node_group(&self, group: NodeGroup) -> ApplicationResult<NodeGroup> {
        sqlx::query(
            "insert into node_groups (id, tenant_id, name, created_at, updated_at) values ($1,$2,$3,$4,$5)",
        )
        .bind(group.id)
        .bind(group.tenant_id)
        .bind(&group.name)
        .bind(group.created_at)
        .bind(group.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(group)
    }

    async fn list_node_groups(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<NodeGroup>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, NodeGroup>(
                "select * from node_groups where tenant_id = $1 order by name asc",
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, NodeGroup>("select * from node_groups order by name asc")
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }

    async fn find_node_group(&self, node_group_id: Uuid) -> ApplicationResult<Option<NodeGroup>> {
        sqlx::query_as::<_, NodeGroup>("select * from node_groups where id = $1")
            .bind(node_group_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn update_node_group(&self, group: NodeGroup) -> ApplicationResult<NodeGroup> {
        sqlx::query("update node_groups set name = $2, updated_at = $3 where id = $1")
            .bind(group.id)
            .bind(&group.name)
            .bind(group.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(group)
    }

    async fn delete_node_group(&self, node_group_id: Uuid) -> ApplicationResult<()> {
        sqlx::query("delete from node_groups where id = $1")
            .bind(node_group_id)
            .execute(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn list_nodes_in_group(&self, node_group_id: Uuid) -> ApplicationResult<Vec<Node>> {
        sqlx::query_as::<_, Node>(
            "select * from nodes where node_group_id = $1 order by engine::text asc, name asc",
        )
        .bind(node_group_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn list_node_group_domains(
        &self,
        node_group_id: Uuid,
    ) -> ApplicationResult<Vec<NodeGroupDomain>> {
        sqlx::query_as::<_, NodeGroupDomain>(
            "select * from node_group_domains where node_group_id = $1 order by created_at asc",
        )
        .bind(node_group_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn replace_node_group_domains(
        &self,
        node_group_id: Uuid,
        domains: &[NodeGroupDomain],
    ) -> ApplicationResult<Vec<NodeGroupDomain>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query("delete from node_group_domains where node_group_id = $1")
            .bind(node_group_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        for domain in domains {
            sqlx::query(
                r#"
                insert into node_group_domains (
                    id, node_group_id, mode, domain, alias, server_names, host_headers, created_at, updated_at
                ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                "#,
            )
            .bind(domain.id)
            .bind(domain.node_group_id)
            .bind(domain.mode)
            .bind(&domain.domain)
            .bind(&domain.alias)
            .bind(&domain.server_names)
            .bind(&domain.host_headers)
            .bind(domain.created_at)
            .bind(domain.updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(domains.to_vec())
    }

    async fn create_enrollment_token(
        &self,
        record: NodeEnrollmentToken,
    ) -> ApplicationResult<NodeEnrollmentToken> {
        sqlx::query(
            r#"
            insert into node_enrollment_tokens (
                id, tenant_id, node_group_id, token_hash, engine, expires_at, created_at, used_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
        )
        .bind(record.id)
        .bind(record.tenant_id)
        .bind(record.node_group_id)
        .bind(&record.token_hash)
        .bind(record.engine)
        .bind(record.expires_at)
        .bind(record.created_at)
        .bind(record.used_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(record)
    }

    async fn consume_enrollment_token(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<NodeEnrollmentToken>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let record = sqlx::query_as::<_, NodeEnrollmentToken>(
            r#"
            select * from node_enrollment_tokens
            where token_hash = $1 and used_at is null
            order by created_at desc
            limit 1
            for update
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        if let Some(record) = &record {
            sqlx::query(
                "update node_enrollment_tokens set used_at = now() at time zone 'utc' where id = $1",
            )
            .bind(record.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(record)
    }

    async fn create_node(
        &self,
        node: Node,
        protocols: &[NodeCapability],
    ) -> ApplicationResult<Node> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query(
            r#"
            insert into nodes (
                id, tenant_id, node_group_id, name, engine, version, status, last_seen_at, node_token_hash, created_at, updated_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            "#,
        )
        .bind(node.id)
        .bind(node.tenant_id)
        .bind(node.node_group_id)
        .bind(&node.name)
        .bind(node.engine)
        .bind(&node.version)
        .bind(node.status)
        .bind(node.last_seen_at)
        .bind(&node.node_token_hash)
        .bind(node.created_at)
        .bind(node.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        for capability in protocols {
            sqlx::query("insert into node_capabilities (node_id, protocol) values ($1,$2)")
                .bind(capability.node_id)
                .bind(capability.protocol)
                .execute(&mut *transaction)
                .await
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(node)
    }

    async fn find_node(&self, node_id: Uuid) -> ApplicationResult<Option<Node>> {
        sqlx::query_as::<_, Node>("select * from nodes where id = $1")
            .bind(node_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn find_node_by_token_hash(
        &self,
        token_hash: &str,
    ) -> ApplicationResult<Option<Node>> {
        sqlx::query_as::<_, Node>("select * from nodes where node_token_hash = $1")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn update_node_heartbeat(
        &self,
        node_id: Uuid,
        version: &str,
        status: NodeStatus,
    ) -> ApplicationResult<Option<Node>> {
        sqlx::query_as::<_, Node>(
            r#"
            update nodes
            set version = $2, status = $3, last_seen_at = now() at time zone 'utc', updated_at = now() at time zone 'utc'
            where id = $1
            returning *
            "#,
        )
        .bind(node_id)
        .bind(version)
        .bind(status)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn list_nodes(&self, tenant_id: Option<Uuid>) -> ApplicationResult<Vec<Node>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, Node>("select * from nodes where tenant_id = $1 order by name asc")
                .bind(tenant_id)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query_as::<_, Node>("select * from nodes order by name asc")
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rows)
    }

    async fn list_node_capabilities(
        &self,
        node_id: Uuid,
    ) -> ApplicationResult<Vec<NodeCapability>> {
        sqlx::query_as::<_, NodeCapability>(
            "select * from node_capabilities where node_id = $1 order by protocol::text asc",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn replace_node_endpoints(
        &self,
        node_id: Uuid,
        endpoints: &[NodeEndpoint],
    ) -> ApplicationResult<Vec<NodeEndpoint>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        sqlx::query("delete from node_endpoints where node_id = $1")
            .bind(node_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        for endpoint in endpoints {
            let encrypted_reality_private_key = self
                .secret_box
                .encrypt_option(endpoint.reality_private_key.as_deref())?;
            sqlx::query(
                r#"
                insert into node_endpoints (
                    id, node_id, protocol, listen_host, listen_port, public_host, public_port, transport, security,
                    server_name, host_header, path, service_name, flow, reality_public_key, reality_private_key,
                    reality_short_id, fingerprint, alpn, cipher, tls_certificate_path, tls_key_path, enabled, created_at, updated_at
                ) values (
                    $1,$2,$3,$4,$5,$6,$7,$8,$9,
                    $10,$11,$12,$13,$14,$15,$16,
                    $17,$18,$19,$20,$21,$22,$23,$24,$25
                )
                "#,
            )
            .bind(endpoint.id)
            .bind(endpoint.node_id)
            .bind(endpoint.protocol)
            .bind(&endpoint.listen_host)
            .bind(endpoint.listen_port)
            .bind(&endpoint.public_host)
            .bind(endpoint.public_port)
            .bind(endpoint.transport)
            .bind(endpoint.security)
            .bind(&endpoint.server_name)
            .bind(&endpoint.host_header)
            .bind(&endpoint.path)
            .bind(&endpoint.service_name)
            .bind(&endpoint.flow)
            .bind(&endpoint.reality_public_key)
            .bind(&encrypted_reality_private_key)
            .bind(&endpoint.reality_short_id)
            .bind(&endpoint.fingerprint)
            .bind(&endpoint.alpn)
            .bind(&endpoint.cipher)
            .bind(&endpoint.tls_certificate_path)
            .bind(&endpoint.tls_key_path)
            .bind(endpoint.enabled)
            .bind(endpoint.created_at)
            .bind(endpoint.updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(endpoints.to_vec())
    }

    async fn list_node_endpoints(&self, node_id: Uuid) -> ApplicationResult<Vec<NodeEndpoint>> {
        let rows = sqlx::query_as::<_, NodeEndpoint>(
            "select * from node_endpoints where node_id = $1 order by protocol::text asc, public_port asc",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        rows.into_iter()
            .map(|endpoint| self.decrypt_endpoint(endpoint))
            .collect()
    }

    async fn list_delivery_endpoints(
        &self,
        tenant_id: Uuid,
    ) -> ApplicationResult<Vec<DeliveryNodeEndpoint>> {
        let rows = sqlx::query_as::<_, DeliveryNodeEndpoint>(
            r#"
            select
                n.id as node_id,
                n.name as node_name,
                n.engine,
                e.protocol,
                e.listen_host,
                e.listen_port,
                e.public_host,
                e.public_port,
                e.transport,
                e.security,
                e.server_name,
                e.host_header,
                e.path,
                e.service_name,
                e.flow,
                e.reality_public_key,
                e.reality_private_key,
                e.reality_short_id,
                e.fingerprint,
                e.alpn,
                e.cipher,
                e.tls_certificate_path,
                e.tls_key_path
            from node_endpoints e
            join nodes n on n.id = e.node_id
            where n.tenant_id = $1 and n.status = 'online' and e.enabled = true
            order by n.name asc, e.public_port asc
            "#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        rows.into_iter()
            .map(|endpoint| self.decrypt_delivery_endpoint(endpoint))
            .collect()
    }

    async fn create_config_revision(
        &self,
        revision: ConfigRevision,
    ) -> ApplicationResult<ConfigRevision> {
        let encrypted_rendered_config = self.secret_box.encrypt(&revision.rendered_config)?;
        sqlx::query(
            r#"
            insert into config_revisions (
                id, tenant_id, node_id, name, engine, rendered_config, created_by, created_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
        )
        .bind(revision.id)
        .bind(revision.tenant_id)
        .bind(revision.node_id)
        .bind(&revision.name)
        .bind(revision.engine)
        .bind(&encrypted_rendered_config)
        .bind(revision.created_by)
        .bind(revision.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(revision)
    }

    async fn create_rollout(
        &self,
        rollout: DeploymentRollout,
    ) -> ApplicationResult<DeploymentRollout> {
        let encrypted_rendered_config = self.secret_box.encrypt(&rollout.rendered_config)?;
        sqlx::query(
            r#"
            insert into deployment_rollouts (
                id, tenant_id, node_id, config_revision_id, engine, revision_name, rendered_config, target_path, status, failure_reason, created_at, updated_at, applied_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            "#,
        )
        .bind(rollout.id)
        .bind(rollout.tenant_id)
        .bind(rollout.node_id)
        .bind(rollout.config_revision_id)
        .bind(rollout.engine)
        .bind(&rollout.revision_name)
        .bind(&encrypted_rendered_config)
        .bind(&rollout.target_path)
        .bind(rollout.status)
        .bind(&rollout.failure_reason)
        .bind(rollout.created_at)
        .bind(rollout.updated_at)
        .bind(rollout.applied_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(rollout)
    }

    async fn find_rollout(&self, rollout_id: Uuid) -> ApplicationResult<Option<DeploymentRollout>> {
        let rollout =
            sqlx::query_as::<_, DeploymentRollout>("select * from deployment_rollouts where id = $1")
            .bind(rollout_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        rollout
            .map(|rollout| self.decrypt_rollout(rollout))
            .transpose()
    }

    async fn list_rollouts(
        &self,
        tenant_id: Option<Uuid>,
    ) -> ApplicationResult<Vec<DeploymentRollout>> {
        let rows = if let Some(tenant_id) = tenant_id {
            sqlx::query_as::<_, DeploymentRollout>(
                "select * from deployment_rollouts where tenant_id = $1 order by created_at desc",
            )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, DeploymentRollout>(
                "select * from deployment_rollouts order by created_at desc",
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        rows.into_iter()
            .map(|rollout| self.decrypt_rollout(rollout))
            .collect()
    }

    async fn list_ready_rollouts(
        &self,
        node_id: Uuid,
        limit: i64,
    ) -> ApplicationResult<Vec<DeploymentRollout>> {
        let rows = sqlx::query_as::<_, DeploymentRollout>(
            r#"
            select * from deployment_rollouts
            where node_id = $1 and status in ('queued', 'ready')
            order by created_at asc
            limit $2
            "#,
        )
        .bind(node_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        rows.into_iter()
            .map(|rollout| self.decrypt_rollout(rollout))
            .collect()
    }

    async fn update_rollout_state(
        &self,
        rollout_id: Uuid,
        status: DeploymentStatus,
        failure_reason: Option<String>,
    ) -> ApplicationResult<()> {
        let applied_at = if status == DeploymentStatus::Applied {
            Some(chrono::Utc::now())
        } else {
            None
        };
        sqlx::query(
            "update deployment_rollouts set status = $2, failure_reason = $3, updated_at = now() at time zone 'utc', applied_at = coalesce($4, applied_at) where id = $1",
        )
        .bind(rollout_id)
        .bind(status)
        .bind(failure_reason)
        .bind(applied_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }
}
