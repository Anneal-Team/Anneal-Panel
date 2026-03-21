alter table subscriptions
    add column if not exists note text null;

do $$
begin
    if exists (
        select 1
        from information_schema.columns
        where table_name = 'subscriptions'
          and column_name = 'quota_bytes'
    ) and not exists (
        select 1
        from information_schema.columns
        where table_name = 'subscriptions'
          and column_name = 'traffic_limit_bytes'
    ) then
        alter table subscriptions rename column quota_bytes to traffic_limit_bytes;
    end if;
end
$$;

alter table subscriptions
    add column if not exists traffic_limit_bytes bigint null,
    add column if not exists access_key text null,
    add column if not exists expires_at timestamptz null;

update subscriptions
set traffic_limit_bytes = coalesce(traffic_limit_bytes, 0),
    access_key = coalesce(access_key, encode(gen_random_bytes(24), 'hex')),
    expires_at = coalesce(expires_at, created_at + interval '30 days');

alter table subscriptions
    alter column traffic_limit_bytes set not null,
    alter column access_key set not null,
    alter column expires_at set not null;

create index if not exists idx_subscriptions_expires_at
    on subscriptions(expires_at);
