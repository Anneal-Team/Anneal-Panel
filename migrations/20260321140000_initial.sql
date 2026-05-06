create extension if not exists pgcrypto;

do $$
begin
    if not exists (select 1 from pg_type where typname = 'user_role') then
        create type user_role as enum ('superadmin', 'admin', 'reseller', 'user');
    end if;
    if not exists (select 1 from pg_type where typname = 'user_status') then
        create type user_status as enum ('active', 'suspended');
    end if;
    if not exists (select 1 from pg_type where typname = 'quota_state') then
        create type quota_state as enum ('normal', 'warning80', 'warning95', 'exhausted');
    end if;
    if not exists (select 1 from pg_type where typname = 'notification_kind') then
        create type notification_kind as enum ('quota80', 'quota95', 'quota100');
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

create table if not exists pre_auth_challenges (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    purpose text not null,
    pending_totp_secret text null,
    expires_at timestamptz not null,
    used_at timestamptz null,
    created_at timestamptz not null default now()
);

create table if not exists auth_rate_limits (
    scope text primary key,
    failures integer not null,
    first_failed_at timestamptz not null,
    last_failed_at timestamptz not null,
    locked_until timestamptz null
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

create table if not exists devices (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    user_id uuid not null references users(id) on delete cascade,
    name text not null,
    device_token text not null unique,
    device_token_hash text not null unique,
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
    note text null,
    access_key text not null,
    traffic_limit_bytes bigint not null,
    used_bytes bigint not null default 0,
    quota_state quota_state not null default 'normal',
    suspended boolean not null default false,
    expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table if not exists subscription_links (
    id uuid primary key,
    subscription_id uuid not null references subscriptions(id) on delete cascade,
    expires_at timestamptz not null,
    revoked_at timestamptz null,
    created_at timestamptz not null default now()
);

create table if not exists usage_samples (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
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

create schema if not exists apalis;

create table if not exists apalis.workers (
    id text primary key,
    worker_type text not null,
    storage_name text not null,
    layers text not null default '',
    last_seen timestamptz not null default now(),
    started_at timestamptz null
);

create table if not exists apalis.jobs (
    job bytea not null,
    id text not null,
    job_type text not null,
    status text not null default 'Pending',
    attempts integer not null default 0,
    max_attempts integer not null default 25,
    run_at timestamptz not null default now(),
    last_result jsonb null,
    lock_at timestamptz null,
    lock_by text null references apalis.workers(id),
    done_at timestamptz null,
    priority integer not null default 0,
    metadata jsonb null,
    primary key (id)
);

create or replace function apalis.get_jobs(
    worker_id text,
    v_job_type text,
    v_job_count integer default 5::integer
) returns setof apalis.jobs as $fn$
begin
    return query
    update apalis.jobs
    set status = 'Queued',
        lock_by = worker_id,
        lock_at = now()
    where id in (
        select id
        from apalis.jobs
        where (status = 'Pending' or (status = 'Failed' and attempts < max_attempts))
          and run_at < now()
          and job_type = v_job_type
        order by priority desc, run_at asc, id asc
        limit v_job_count
        for update skip locked
    )
    returning *;
end;
$fn$ language plpgsql volatile;

drop trigger if exists notify_workers on apalis.jobs;
drop function if exists apalis.notify_new_jobs;

create function apalis.notify_new_jobs() returns trigger as $fn$
begin
    if new.run_at <= now() then
        perform pg_notify(
            'apalis::job::insert',
            json_build_object(
                'job_type', new.job_type,
                'id', new.id,
                'run_at', new.run_at
            )::text
        );
    end if;
    return new;
end;
$fn$ language plpgsql;

create trigger notify_workers
after insert on apalis.jobs
for each row execute function apalis.notify_new_jobs();

create index if not exists idx_users_tenant_id on users(tenant_id);
create index if not exists idx_users_email_lower on users(lower(email));
create index if not exists idx_refresh_sessions_user_id on refresh_sessions(user_id);
create index if not exists idx_pre_auth_challenges_user_id on pre_auth_challenges(user_id);
create index if not exists idx_pre_auth_challenges_expires_at on pre_auth_challenges(expires_at);
create index if not exists idx_devices_tenant_id on devices(tenant_id);
create index if not exists idx_devices_user_id on devices(user_id);
create index if not exists idx_subscriptions_tenant_id on subscriptions(tenant_id);
create index if not exists idx_subscriptions_device_id on subscriptions(device_id);
create index if not exists idx_subscription_links_subscription_id on subscription_links(subscription_id);
create index if not exists idx_usage_samples_subscription_id on usage_samples(subscription_id);
create index if not exists idx_usage_samples_measured_at on usage_samples(measured_at);
create index if not exists idx_notification_events_tenant_id on notification_events(tenant_id);
create index if not exists idx_notification_events_created_at on notification_events(created_at);
create index if not exists apalis_workers_worker_type_idx on apalis.workers(worker_type);
create index if not exists apalis_workers_last_seen_idx on apalis.workers(last_seen);
create index if not exists apalis_jobs_status_idx on apalis.jobs(status);
create index if not exists apalis_jobs_lock_by_idx on apalis.jobs(lock_by);
create index if not exists apalis_jobs_job_type_idx on apalis.jobs(job_type);
