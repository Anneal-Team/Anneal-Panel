alter table node_endpoints
    add column if not exists tls_certificate_path text null,
    add column if not exists tls_key_path text null;
