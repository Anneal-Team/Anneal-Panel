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
  <b>Нативная установка &nbsp;·&nbsp; Mihomo runtime &nbsp;·&nbsp; Доставка подписок &nbsp;·&nbsp; Веб-панель управления</b>
</p>

---

## Что такое Anneal?

Self-hosted панель управления прокси-подписками. Управление пользователями и реселлерами, квоты, журнал аудита, доставка подписок и генерация конфигов Mihomo — всё в одном Rust-бинарнике, без лишней оркестрации.

---

## ✨ Возможности

| | |
|---|---|
| 👥 **Роли** | Суперадмин / админ / реселлер / пользователь |
| 🖥️ **Веб-панель** | Пользователи, подписки, потребление, уведомления |
| 📦 **Подписки** | Raw-ссылки, base64-бандлы, Mihomo YAML |
| ⚡ **Mihomo** | Встроенный бинарник, systemd-сервис, автогенерация конфига |
| 🔌 **API** | Миграции, аудит-лог, TOTP, квоты, сессии авторизации |
| 🚀 **Установщик** | PostgreSQL, Caddy, API, worker, веб-панель, Mihomo — одной командой |

---

## 📁 Структура репозитория

| Путь | Назначение |
|------|-----------|
| `apps/api` | HTTP API, авторизация, OpenAPI |
| `apps/annealctl` | Установщик, обновление, управление сервисами |
| `apps/worker` | Фоновый воркер уведомлений |
| `crates/config-engine` | Рендеринг конфигов Mihomo и share-ссылок |
| `crates/subscriptions` | Подписки, ссылки доставки, устройства |
| `crates/users` | Пользователи, реселлеры, тенанты |
| `crates/usage` | Статистика, роллапы, состояние квот |
| `web` | Фронтенд — React / Vite |
| `deploy/systemd` | Systemd-юниты |
| `migrations` | Схема PostgreSQL |

---

## 🛠️ Установка

### ⚡ Быстрый старт

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash
```

Запрашивает необходимые значения интерактивно. Для автоматизации:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- \
  --domain panel.example.com \
  --non-interactive
```

### 📌 Зафиксировать конкретный релиз

```bash
curl -fsSLo /tmp/anneal-install.sh https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh
sudo ANNEAL_RELEASE_TAG=0.1.0 bash /tmp/anneal-install.sh
```

### 📦 Что устанавливается

- **Сервисы:** `anneal-api`, `anneal-worker`, `anneal-caddy`, `anneal-mihomo`, `postgresql` — все как systemd-юниты, поднимаются автоматически после перезагрузки
- **Конфиги:** `/etc/anneal/install.toml`, `anneal.env`, `admin-summary.env`, `/var/lib/anneal/install-state.json`
- **Источники пакетов:** PostgreSQL 17 из PGDG, Caddy из официального APT-репозитория, Mihomo bundled в архиве релиза

### 🐧 Поддерживаемые дистрибутивы

Debian 10–13 · Ubuntu 22.04, 24.04, 25.04, 25.10

### 🔧 После установки

```bash
annealctl status
annealctl doctor
annealctl restart
annealctl update --bundle-root ...
annealctl uninstall
```

### 🔖 Каналы релизов

- `master` → rolling-релиз `rolling-master` (по умолчанию)
- Semver-теги вроде `0.1.0` → стабильные релизы
- `ANNEAL_RELEASE_TAG=0.1.0` — ручная фиксация версии

---

## 🤝 Участие в разработке

Баг-репорты, идеи и PR приветствуются. Для чего-то крупнее мелкого фикса — сначала откройте issue, чтобы согласовать направление.

> **Заметка:** Проект в активной разработке — держите PR сфокусированными и проверенными.
