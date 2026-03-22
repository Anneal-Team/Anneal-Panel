alter table subscription_links
    add column if not exists token_hash text null;

create unique index if not exists subscription_links_token_hash_idx
    on subscription_links(token_hash)
    where token_hash is not null;
