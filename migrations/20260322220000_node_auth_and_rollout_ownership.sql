alter table nodes
    add column if not exists node_token_hash text;

update nodes
set node_token_hash = encode(gen_random_bytes(32), 'hex')
where node_token_hash is null or node_token_hash = '';

alter table nodes
    alter column node_token_hash set not null;

create index if not exists idx_nodes_node_token_hash on nodes(node_token_hash);
