create table if not exists pre_auth_challenges (
    id uuid primary key,
    user_id uuid not null references users(id) on delete cascade,
    purpose text not null,
    pending_totp_secret text null,
    expires_at timestamptz not null,
    used_at timestamptz null,
    created_at timestamptz not null default now()
);

create index if not exists idx_pre_auth_challenges_user_id
    on pre_auth_challenges(user_id);

create index if not exists idx_pre_auth_challenges_expires_at
    on pre_auth_challenges(expires_at);
