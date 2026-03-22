drop index if exists subscription_links_token_hash_idx;

alter table subscription_links
    drop constraint if exists subscription_links_token_key;

alter table subscription_links
    alter column expires_at set not null;

alter table subscription_links
    drop column if exists token_hash;

alter table subscription_links
    drop column if exists token;
