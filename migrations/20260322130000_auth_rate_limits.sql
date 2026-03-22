create table if not exists auth_rate_limits (
    scope text primary key,
    failures integer not null,
    first_failed_at timestamptz not null,
    last_failed_at timestamptz not null,
    locked_until timestamptz null
);
