alter table deployment_rollouts
    add column if not exists config_revision_id uuid references config_revisions(id) on delete cascade;

update deployment_rollouts
set config_revision_id = coalesce(
    config_revision_id,
    (
        select id
        from config_revisions
        where config_revisions.tenant_id = deployment_rollouts.tenant_id
          and config_revisions.node_id is not distinct from deployment_rollouts.node_id
          and config_revisions.name = deployment_rollouts.revision_name
        order by created_at desc
        limit 1
    )
);

create index if not exists idx_deployment_rollouts_config_revision_id
    on deployment_rollouts(config_revision_id);
