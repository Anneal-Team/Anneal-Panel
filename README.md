<p align="right">
  <a href="./README.md"><img src="https://img.shields.io/badge/English-4f46e5?style=flat-square&labelColor=1e1b4b" alt="English" /></a>
  &nbsp;
  <a href="./README.ru.md"><img src="https://img.shields.io/badge/Russian-4f46e5?style=flat-square&labelColor=1e1b4b" alt="Russian" /></a>
</p>

<p align="center">
  <img src="./anneal-github-banner.svg" alt="Anneal" width="860" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-CE422B?style=for-the-badge&logo=rust&logoColor=white&labelColor=1a1a2e" alt="Rust 2024" />
  &nbsp;
  <img src="https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=0d1117&labelColor=1a1a2e" alt="React 19" />
  &nbsp;
  <img src="https://img.shields.io/badge/PostgreSQL-17-336791?style=for-the-badge&logo=postgresql&logoColor=white&labelColor=1a1a2e" alt="PostgreSQL 17" />
  &nbsp;
  <img src="https://img.shields.io/badge/Mihomo-bundled-22c55e?style=for-the-badge&labelColor=1a1a2e" alt="Mihomo bundled" />
  &nbsp;
  <img src="https://img.shields.io/badge/License-AGPL--3.0-a855f7?style=for-the-badge&labelColor=1a1a2e" alt="AGPL-3.0" />
</p>

<p align="center">
  <b>Native install &nbsp;·&nbsp; Mihomo runtime &nbsp;·&nbsp; Subscription delivery &nbsp;·&nbsp; Web control panel</b>
</p>

---

## What is Anneal

Anneal is a native control panel for operating subscription-based proxy access: user management, quotas, audit logs, subscription delivery, and Mihomo-compatible client configuration from one Rust control plane.

It is built for multi-tenant environments where admins, resellers, and users need isolated access to their own data without carrying extra runtime orchestration.

---

## Features

```text
Multi-tenancy        - superadmin / admin / reseller / user roles
Web interface        - users, subscriptions, usage, notifications
Rust API             - migrations, audit log, TOTP, quota state, auth sessions
Native installer     - PostgreSQL, Caddy, API, worker, web panel, Mihomo
Mihomo runtime       - bundled binary, systemd service, generated config
Subscriptions        - raw links, base64 bundles, Mihomo YAML
```

---

## Repository Layout

| Path | Purpose |
|------|---------|
| `apps/api` | HTTP API, auth, OpenAPI, transport layer |
| `apps/annealctl` | Native installer, updater, service management |
| `apps/worker` | Background notification worker |
| `crates/config-engine` | Mihomo and share-link rendering |
| `crates/subscriptions` | Subscriptions, delivery links, devices |
| `crates/users` | Users, resellers, tenants |
| `crates/usage` | Usage samples, rollups, quota state |
| `web` | Frontend - React / Vite |
| `deploy/systemd` | Native systemd units |
| `migrations` | PostgreSQL SQL schema |

---

## Installation

Anneal ships with a bootstrap wrapper that downloads one ready-made release bundle from GitHub Releases, verifies bundled SHA256 checksums, and runs `annealctl install --bundle-root ...`.

- Installer file: [`scripts/install.sh`](./scripts/install.sh)
- Direct link: [raw install.sh](https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh)

Quick start:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash
```

This command opens the installer prompts when required values are not passed as CLI flags.

Release model:
- the raw `master` installer prefers the rolling `rolling-master` release and falls back to the latest GitHub Release only if that rolling channel is missing
- pushes to `master` refresh the `rolling-master` bundle, while stable bundles are published from semver tags such as `0.1.0` and `v0.1.0`
- set `ANNEAL_RELEASE_TAG=0.1.0` to pin a specific release manually

Supported distributions:
- Debian 10, 11, 12, 13
- Ubuntu 22.04 LTS, 24.04 LTS, 25.04, 25.10

Package sources used by the installer:
- PostgreSQL 17 comes from the official PGDG repository; Debian 10 uses the official PGDG archive
- Caddy comes from the official Caddy APT repository
- Mihomo is shipped inside the Anneal release bundle and installed as `anneal-mihomo.service`

Pin a specific release:

```bash
curl -fsSLo /tmp/anneal-install.sh https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh
sudo ANNEAL_RELEASE_TAG=0.1.0 bash /tmp/anneal-install.sh
```

Non-interactive control-plane install:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- \
  --domain panel.example.com \
  --non-interactive
```

The installer:
- uses `annealctl` as the single source of truth for `install`, `resume`, `status`, `doctor`, `update`, `restart`, and `uninstall`
- supports interactive prompts from the one-line bootstrap command and non-interactive CLI flags for automation
- installs the native control plane only: API, worker, web panel, PostgreSQL, Caddy, and Mihomo
- downloads exactly one release archive such as `anneal-rolling-master-linux-amd64.tar.gz` or `anneal-0.1.0-linux-amd64.tar.gz`
- unpacks the bundle and launches the bundled `bin/annealctl`
- generates panel path, database URL, admin credentials, reseller defaults, starter subscription, and secrets
- writes typed install data to `/etc/anneal/install.toml`, `/var/lib/anneal/install-state.json`, `/etc/anneal/anneal.env`, and `/etc/anneal/admin-summary.env`
- starts `postgresql`, `anneal-api.service`, `anneal-worker.service`, `anneal-caddy.service`, and `anneal-mihomo.service`

After installation:
- use `annealctl status`, `annealctl doctor`, `annealctl restart`, `annealctl update --bundle-root ...`, and `annealctl uninstall`
- after a VPS/VDS reboot, Anneal services and Mihomo come back automatically through systemd

---

## Key Scenarios

- Manage users, resellers, subscriptions, limits, and expiration dates
- Deliver raw links, base64 link bundles, and Mihomo YAML configs
- Track usage rollups and quota state
- Audit security-sensitive actions
- Run the control plane and Mihomo as native systemd services

---

## Contributing

Contributions are welcome. Bug reports, feature ideas, docs fixes, and pull requests all help move the project forward.

If you are going to contribute code, open an issue first so we can discuss the direction. For small fixes and improvements, feel free to send a PR directly.

> [!NOTE]
> The project is in active development. Some areas of the codebase are still being shaped, so keep changes focused and verified.
