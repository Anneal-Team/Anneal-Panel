create extension if not exists pgcrypto;

do $$
begin
    if not exists (select 1 from pg_type where typname = 'user_role') then
        create type user_role as enum ('superadmin', 'admin', 'reseller', 'user');
    end if;
    if not exists (select 1 from pg_type where typname = 'user_status') then
        create type user_status as enum ('active', 'suspended');
    end if;
    if not exists (select 1 from pg_type where typname = 'node_status') then
        create type node_status as enum ('pending', 'online', 'offline');
    end if;
    if not exists (select 1 from pg_type where typname = 'proxy_engine') then
        create type proxy_engine as enum ('xray', 'singbox');
    end if;
    if not exists (select 1 from pg_type where typname = 'protocol_kind') then
        create type protocol_kind as enum ('vless_reality', 'vmess', 'trojan', 'shadowsocks_2022', 'tuic', 'hysteria2');
    end if;
    if not exists (select 1 from pg_type where typname = 'deployment_status') then
        create type deployment_status as enum ('queued', 'rendering', 'validating', 'ready', 'applied', 'rolled_back', 'failed');
    end if;
    if not exists (select 1 from pg_type where typname = 'quota_state') then
        create type quota_state as enum ('normal', 'warning80', 'warning95', 'exhausted');
    end if;
    if not exists (select 1 from pg_type where typname = 'notification_kind') then
        create type notification_kind as enum ('quota80', 'quota95', 'quota100', 'node_offline');
    end if;
end $$;

create table if not exists tenants (
    id uuid primary key,
    name text not null unique,
    owner_user_id uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists users (
    id uuid primary key,
    tenant_id uuid references tenants(id) on delete cascade,
    email text not null unique,
    display_name text not null,
    role user_role not null,
    status user_status not null,
    password_hash text not null,
    totp_secret text null,
    totp_confirmed boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists totp_credentials (
    user_id uuid primary key references users(id) on delete cascade,
    secret text not null,
    confirmed boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists refresh_sessions (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    refresh_token_hash text not null unique,
    user_agent text null,
    ip_address text null,
    expires_at timestamptz not null,
    revoked_at timestamptz null,
    rotated_from_session_id uuid null references refresh_sessions(id) on delete set null,
    created_at timestamptz not null default now()
);

create table if not exists audit_logs (
    id uuid primary key default gen_random_uuid(),
    actor_user_id uuid null references users(id) on delete set null,
    tenant_id uuid null references tenants(id) on delete cascade,
    action text not null,
    resource_type text not null,
    resource_id uuid null,
    payload jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now()
);

create table if not exists node_groups (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    name text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, name)
);

create table if not exists node_enrollment_tokens (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    node_group_id uuid not null references node_groups(id) on delete cascade,
    token_hash text not null unique,
    engine proxy_engine not null,
    expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    used_at timestamptz null
);

create table if not exists nodes (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    node_group_id uuid not null references node_groups(id) on delete cascade,
    name text not null,
    engine proxy_engine not null,
    version text not null,
    status node_status not null,
    last_seen_at timestamptz null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (tenant_id, name)
);

create table if not exists node_capabilities (
    node_id uuid not null references nodes(id) on delete cascade,
    protocol protocol_kind not null,
    primary key (node_id, protocol)
);

create table if not exists devices (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    user_id uuid not null references users(id) on delete cascade,
    name text not null,
    device_token text not null unique,
    suspended boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists subscriptions (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    user_id uuid not null references users(id) on delete cascade,
    device_id uuid not null references devices(id) on delete cascade,
    name text not null,
    quota_bytes bigint not null,
    used_bytes bigint not null default 0,
    quota_state quota_state not null default 'normal',
    suspended boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists subscription_links (
    id uuid primary key,
    subscription_id uuid not null references subscriptions(id) on delete cascade,
    token text not null unique,
    revoked_at timestamptz null,
    created_at timestamptz not null default now()
);

create table if not exists config_revisions (
    id uuid primary key default gen_random_uuid(),
    tenant_id uuid not null references tenants(id) on delete cascade,
    node_id uuid null references nodes(id) on delete set null,
    name text not null,
    engine proxy_engine not null,
    rendered_config text not null,
    created_by uuid null references users(id) on delete set null,
    created_at timestamptz not null default now()
);

create table if not exists deployment_rollouts (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    node_id uuid not null references nodes(id) on delete cascade,
    revision_name text not null,
    rendered_config text not null,
    target_path text not null,
    status deployment_status not null,
    failure_reason text null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    applied_at timestamptz null
);

create table if not exists usage_samples (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    node_id uuid not null references nodes(id) on delete cascade,
    subscription_id uuid not null references subscriptions(id) on delete cascade,
    device_id uuid not null references devices(id) on delete cascade,
    bytes_in bigint not null,
    bytes_out bigint not null,
    measured_at timestamptz not null,
    created_at timestamptz not null default now()
);

create table if not exists usage_rollups_hourly (
    id uuid primary key default gen_random_uuid(),
    subscription_id uuid not null references subscriptions(id) on delete cascade,
    bucket_start timestamptz not null,
    total_bytes bigint not null,
    unique (subscription_id, bucket_start)
);

create table if not exists usage_rollups_daily (
    id uuid primary key default gen_random_uuid(),
    subscription_id uuid not null references subscriptions(id) on delete cascade,
    bucket_start date not null,
    total_bytes bigint not null,
    unique (subscription_id, bucket_start)
);

create table if not exists notification_events (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    kind notification_kind not null,
    title text not null,
    body text not null,
    delivered_at timestamptz null,
    created_at timestamptz not null default now()
);

create index if not exists idx_users_tenant_id on users(tenant_id);
create index if not exists idx_nodes_tenant_id on nodes(tenant_id);
create index if not exists idx_subscriptions_tenant_id on subscriptions(tenant_id);
create index if not exists idx_usage_samples_subscription_id on usage_samples(subscription_id);
create index if not exists idx_notification_events_tenant_id on notification_events(tenant_id);
