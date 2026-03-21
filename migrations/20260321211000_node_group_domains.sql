do $$
begin
    if not exists (select 1 from pg_type where typname = 'node_group_domain_mode') then
        create type node_group_domain_mode as enum (
            'direct',
            'legacy_direct',
            'cdn',
            'auto_cdn',
            'relay',
            'worker',
            'reality',
            'fake'
        );
    end if;
end $$;

create table if not exists node_group_domains (
    id uuid primary key,
    node_group_id uuid not null references node_groups(id) on delete cascade,
    mode node_group_domain_mode not null,
    domain text not null,
    alias text null,
    server_names text[] not null default '{}'::text[],
    host_headers text[] not null default '{}'::text[],
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (node_group_id, mode, domain)
);

create index if not exists idx_node_group_domains_node_group_id on node_group_domains(node_group_id);
