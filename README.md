# Anneal

Anneal is a Rust control plane for user management, subscription delivery, quota tracking, audit logs, and Mihomo-compatible client configuration.

The current build is intentionally native-only. It installs the API, worker, web panel, Caddy, PostgreSQL, and the bundled Mihomo runtime. Legacy server-node orchestration, container deployment, and old runtime cores have been removed.

## Repository Layout

| Path | Purpose |
| --- | --- |
| `apps/api` | HTTP API, auth, OpenAPI, transport layer |
| `apps/annealctl` | Native installer, updater, service management |
| `apps/worker` | Background notification worker |
| `crates/config-engine` | Mihomo/share-link rendering |
| `crates/subscriptions` | Subscriptions, delivery links, devices |
| `crates/users` | Users, resellers, tenants |
| `crates/usage` | Usage samples, rollups, quota state |
| `web` | React/Vite control panel |
| `deploy/systemd` | Native systemd units |
| `migrations` | PostgreSQL schema |

## Install

Build a release bundle with:

```bash
scripts/package-release.sh
```

Install from a bundle on the target host:

```bash
sudo ./install.sh --bundle-root /path/to/anneal-bundle
```

The installer writes `/etc/anneal/install.toml`, `/etc/anneal/anneal.env`, `/etc/anneal/admin-summary.env`, and `/var/lib/anneal/install-state.json`, then starts:

- `postgresql`
- `anneal-api.service`
- `anneal-worker.service`
- `anneal-caddy.service`
- `anneal-mihomo.service`

## Operations

```bash
annealctl status
annealctl doctor
annealctl restart
annealctl update --bundle-root /path/to/new-bundle
annealctl uninstall
```

## Runtime

Subscriptions are served as raw links, base64 link lists, or Mihomo YAML depending on request mode and client headers. The installer provisions Mihomo with a minimal native config and exposes delivery endpoint metadata through the API settings.
