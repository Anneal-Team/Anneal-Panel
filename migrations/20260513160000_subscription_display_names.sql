create table if not exists app_settings (
    key text primary key,
    value jsonb not null,
    updated_at timestamptz not null default now()
);

alter table subscriptions
    add column if not exists client_name text null,
    add column if not exists proxy_name_overrides jsonb not null default '{}'::jsonb;
