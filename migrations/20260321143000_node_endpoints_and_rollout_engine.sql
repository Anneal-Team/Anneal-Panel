do $$
begin
    if not exists (select 1 from pg_type where typname = 'transport_kind') then
        create type transport_kind as enum ('tcp', 'ws', 'grpc', 'http_upgrade');
    end if;
    if not exists (select 1 from pg_type where typname = 'security_kind') then
        create type security_kind as enum ('none', 'tls', 'reality');
    end if;
end $$;

alter table deployment_rollouts
    add column if not exists engine proxy_engine not null default 'xray';

update deployment_rollouts r
set engine = n.engine
from nodes n
where r.node_id = n.id;

create table if not exists node_endpoints (
    id uuid primary key,
    node_id uuid not null references nodes(id) on delete cascade,
    protocol protocol_kind not null,
    listen_host text not null,
    listen_port integer not null,
    public_host text not null,
    public_port integer not null,
    transport transport_kind not null,
    security security_kind not null,
    server_name text null,
    host_header text null,
    path text null,
    service_name text null,
    flow text null,
    reality_public_key text null,
    reality_private_key text null,
    reality_short_id text null,
    fingerprint text null,
    alpn text[] not null default '{}'::text[],
    cipher text null,
    enabled boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create index if not exists idx_node_endpoints_node_id on node_endpoints(node_id);
