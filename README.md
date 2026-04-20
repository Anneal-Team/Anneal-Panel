<p align="right">
  <a href="./README.md"><img src="https://img.shields.io/badge/🇬🇧-English-4f46e5?style=flat-square&labelColor=1e1b4b" alt="English" /></a>
  &nbsp;
  <a href="./README.ru.md"><img src="https://img.shields.io/badge/🇷🇺-Русский-4f46e5?style=flat-square&labelColor=1e1b4b" alt="Русский" /></a>
</p>

<p align="center">
  <img src="./anneal-github-banner.svg" alt="Anneal" width="860" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-CE422B?style=for-the-badge&logo=rust&logoColor=white&labelColor=1a1a2e" alt="Rust 2024" />
  &nbsp;
  <img src="https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=0d1117&labelColor=1a1a2e" alt="React 19" />
  &nbsp;
  <img src="https://img.shields.io/badge/PostgreSQL-16-336791?style=for-the-badge&logo=postgresql&logoColor=white&labelColor=1a1a2e" alt="PostgreSQL 16" />
  &nbsp;
  <img src="https://img.shields.io/badge/Docker-ready-2496ED?style=for-the-badge&logo=docker&logoColor=white&labelColor=1a1a2e" alt="Docker ready" />
  &nbsp;
  <img src="https://img.shields.io/badge/License-AGPL--3.0-a855f7?style=for-the-badge&labelColor=1a1a2e" alt="AGPL-3.0" />
</p>

<p align="center">
  <b>Server group management &nbsp;·&nbsp; Domain rules &nbsp;·&nbsp; Subscription delivery &nbsp;·&nbsp; Client configs</b>
</p>

---

[![Codacy Badge](https://api.codacy.com/project/badge/Grade/39dfe3d49742453c81292e8a706a7bef)](https://app.codacy.com/gh/Anneal-Team/Anneal-Panel?utm_source=github.com&utm_medium=referral&utm_content=Anneal-Team/Anneal-Panel&utm_campaign=Badge_Grade)

## 🔥 What is Anneal

Anneal is a control panel that brings together everything needed to operate proxy infrastructure at scale — runtime agents, domain routing rules, automated endpoint generation, subscription management, and client config delivery.

Designed for multi-tenant environments where different teams, resellers and users each need isolated control over their own slice of the infrastructure.

---

## ⚡ Features

```
🏢  Multi-tenancy        —  superadmin / admin / reseller / user roles
🖥️  Web interface        —  nodes, users, subscriptions, domain rules
🦀  Rust API             —  migrations, audit log, TOTP, usage tracking, notifications
🤖  Node agent           —  runtime registration, heartbeat, rollout tasks
🌐  Endpoint generation  —  direct / legacy_direct / cdn / auto_cdn / relay / worker / reality / fake
📦  Subscriptions        —  xray & sing-box configs, links, per-device limits
```

---

## 📁 Repository Layout

| Path | Purpose |
|------|---------|
| `apps/api` | 🔌 HTTP API, auth, Swagger UI, transport layer |
| `apps/node-agent` | 🤖 Server agent — registration, heartbeat, rollout |
| `apps/worker` | ⚙️ Background jobs and queue processing |
| `crates/nodes` | 🗂️ Server groups, domains, endpoints, rollout orchestration |
| `crates/subscriptions` | 📋 Subscriptions, delivery links, devices |
| `crates/users` | 👥 Users, resellers, tenants |
| `crates/config-engine` | 🔧 Client config generation and bundle formats |
| `web` | 🎨 Frontend — React / Vite |
| `deploy/docker` | 🐳 Docker images and environment configs |
| `migrations` | 🗄️ PostgreSQL SQL migrations |

---

## 📦 Installation

Anneal ships with a bootstrap wrapper that downloads exactly one ready-made release bundle archive from GitHub Releases, verifies bundled SHA256 checksums and then runs `annealctl install --bundle-root ...`.

- Installer file: [`scripts/install.sh`](./scripts/install.sh)
- Direct link: [raw install.sh](https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh)

Quick start:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash
```

This one-liner opens the interactive installer wizard when you do not pass explicit CLI flags.

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
- Docker mode uses the official Docker repository where available and falls back to distro packages on older platforms such as Debian 10

Pin a specific release:

```bash
curl -fsSLo /tmp/anneal-install.sh https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh
sudo ANNEAL_RELEASE_TAG=0.1.0 bash /tmp/anneal-install.sh
```

The installer:
- uses `annealctl` as the single source of truth for `install`, `resume`, `status`, `doctor`, `update`, `restart` and `uninstall`
- opens an interactive wizard from the one-line bootstrap command and also supports fully non-interactive CLI flags
- lets you choose server role: `all-in-one`, `control-plane` or `node`
- lets you choose deployment type: `native` or `docker`
- downloads exactly one release bundle such as `anneal-rolling-master-linux-amd64.tar.gz` or `anneal-0.1.0-linux-amd64.tar.gz` instead of building the project on the server
- unpacks the bundle and launches the bundled `bin/annealctl` automatically
- generates panel path, database URL, admin credentials, reseller defaults, node defaults and bootstrap secrets automatically
- writes typed install data to `/etc/anneal/install.toml`, `/var/lib/anneal/install-state.json` and `/etc/anneal/admin-summary.env`
- shows the final admin summary after installation

Control plane:
- installs the panel UI, API, worker, database wiring and Caddy

Node server:
- installs a separate Anneal-managed VPS/VDS node server
- unpacks Xray and Hiddify Core binaries from the same release bundle and runs them under Anneal control
- keeps runtime restart behaviour declarative: native units use `Restart=always`, docker stacks use `restart: unless-stopped`

After installation:
- use `annealctl status`, `annealctl doctor`, `annealctl restart`, `annealctl update --bundle-root ...` and `annealctl uninstall`
- after VPS/VDS reboot the control plane, node-agent and runtime cores come back automatically through systemd or docker restart policies

Role-specific examples:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- --role all-in-one --mode native
```

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- --role control-plane --mode native
```

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- --role node --mode docker
```

---

## 🎯 Key Scenarios

- 🏗️ Create a server group and connect runtime agents
- 🌍 Configure domain rules and automatically generate entry points
- 📬 Issue and manage subscriptions with limits and expiration
- 📱 Deliver client links and configs per device
- 📊 Monitor rollouts, node health, usage and notifications

---

## 🤝 Contributing

Contributions are welcome! Whether it's a bug report, a feature idea, a docs fix, or a pull request — all of it helps move the project forward.

If you're going to contribute code, open an issue first so we can discuss the direction. For small fixes and improvements, feel free to send a PR directly.

> [!NOTE]
> The project is in active development. Some areas of the codebase are still being shaped — good time to get involved early.

---

## 💜 Acknowledgements

Special thanks to the **[Hiddify](https://github.com/hiddify)** team for their contributions to the ecosystem and strong ideas around convenient domain configuration, client config structure, and delivery pipelines.
