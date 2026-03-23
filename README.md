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

Anneal ships with an interactive installer that downloads one ready-made release bundle archive from GitHub Releases and guides setup in TUI mode.

- Installer file: [`scripts/install.sh`](./scripts/install.sh)
- Direct link: [raw install.sh](https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh)

Quick start:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash
```

Release channels:
- `rolling` from the `master` branch
- semver releases from Git tags such as `v0.1.0`

Pin a specific release:

```bash
curl -fsSLo /tmp/anneal-install.sh https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh
sudo ANNEAL_RELEASE_TAG=v0.1.0 bash /tmp/anneal-install.sh
```

The installer:
- asks for installer language: `Русский` or `English`
- lets you choose server role: `Panel` or `Node`
- lets you choose deployment type: `Linux` or `Docker`
- downloads one release bundle such as `anneal-0.1.0-linux-amd64.tar.gz` instead of building the project on the server
- generates passwords, tokens and internal secrets automatically
- shows a final admin summary after installation
- installs a post-login management menu with status, update, restart and removal actions

Panel server:
- installs the control plane: panel UI, API, worker, database and edge services

Node server:
- installs a separate VPS/VDS node server
- this is not an Xray or Sing-box core itself, but a separate Anneal-managed server
- unpacks runtime binaries from the same release bundle and runs them in native or Docker mode

Role-specific examples:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- --role control-plane
```

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- --role node
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
