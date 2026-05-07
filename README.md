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

## What is Anneal?

A self-hosted control panel for proxy subscription management. You get user and reseller management, quotas, audit logs, subscription delivery, and Mihomo config generation — all in one Rust binary, no extra orchestration needed.

---

## ✨ Features

| | |
|---|---|
| 👥 **Roles** | Superadmin / admin / reseller / user |
| 🖥️ **Web panel** | Users, subscriptions, usage, notifications |
| 📦 **Subscriptions** | Raw links, base64 bundles, Mihomo YAML |
| ⚡ **Mihomo** | Bundled binary, systemd service, auto-generated config |
| 🔌 **API** | Migrations, audit log, TOTP, quota, auth sessions |
| 🚀 **Installer** | PostgreSQL, Caddy, API, worker, web panel, Mihomo — one command |

---

## 📁 Repository Layout

| Path | Purpose |
|------|---------|
| `apps/api` | HTTP API, auth, OpenAPI |
| `apps/annealctl` | Installer, updater, service management |
| `apps/worker` | Background notification worker |
| `crates/config-engine` | Mihomo and share-link rendering |
| `crates/subscriptions` | Subscriptions, delivery links, devices |
| `crates/users` | Users, resellers, tenants |
| `crates/usage` | Usage samples, rollups, quota state |
| `web` | Frontend — React / Vite |
| `deploy/systemd` | Systemd units |
| `migrations` | PostgreSQL schema |

---

## 🛠️ Installation

### ⚡ Quick start

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash
```

Prompts for required values interactively. To skip prompts:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- \
  --domain panel.example.com \
  --non-interactive
```

### 📌 Pin a specific release

```bash
curl -fsSLo /tmp/anneal-install.sh https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh
sudo ANNEAL_RELEASE_TAG=0.1.0 bash /tmp/anneal-install.sh
```

### 📦 What gets installed

- **Services:** `anneal-api`, `anneal-worker`, `anneal-caddy`, `anneal-mihomo`, `postgresql` — all as systemd units, restart on boot automatically
- **Config files:** `/etc/anneal/install.toml`, `anneal.env`, `admin-summary.env`, `/var/lib/anneal/install-state.json`
- **Package sources:** PostgreSQL 17 from PGDG, Caddy from the official APT repo, Mihomo bundled in the release archive

### 🐧 Supported distros

Debian 10–13 · Ubuntu 22.04, 24.04, 25.04, 25.10

### 🔧 Post-install

```bash
annealctl status
annealctl doctor
annealctl restart
annealctl update --bundle-root ...
annealctl uninstall
```

### 🔖 Release channels

- `master` → rolling `rolling-master` release (default)
- Semver tags like `0.1.0` → stable releases
- Set `ANNEAL_RELEASE_TAG=0.1.0` to pin manually

---

## 🤝 Contributing

Bug reports, feature ideas, and PRs are welcome. For anything beyond a small fix, open an issue first so we can align on direction before you write the code.

> **Note:** The project is in active development — keep PRs focused and tested.
