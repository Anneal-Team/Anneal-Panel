alter table devices
    add column if not exists device_token_hash text;

update devices
set device_token_hash = coalesce(device_token_hash, '');

create index if not exists idx_devices_device_token_hash on devices(device_token_hash);

create table if not exists node_bootstrap_sessions (
    id uuid primary key,
    tenant_id uuid not null references tenants(id) on delete cascade,
    node_group_id uuid not null references node_groups(id) on delete cascade,
    node_name text not null,
    engines proxy_engine[] not null,
    token_hash text not null unique,
    expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    used_at timestamptz null
);

create index if not exists idx_node_bootstrap_sessions_tenant_id on node_bootstrap_sessions(tenant_id);
create index if not exists idx_node_bootstrap_sessions_token_hash on node_bootstrap_sessions(token_hash);

alter table subscription_links
    add column if not exists expires_at timestamptz;

update subscription_links
set expires_at = subscriptions.expires_at
from subscriptions
where subscriptions.id = subscription_links.subscription_id
  and subscription_links.expires_at is null;

create index if not exists idx_subscription_links_subscription_id_active
    on subscription_links(subscription_id)
    where revoked_at is null;

create table if not exists pre_auth_challenges (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    purpose text not null,
    pending_totp_secret text null,
    expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    used_at timestamptz null
);

create index if not exists idx_pre_auth_challenges_user_id on pre_auth_challenges(user_id);
