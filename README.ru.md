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
  <b>Native-установка &nbsp;·&nbsp; Mihomo runtime &nbsp;·&nbsp; Доставка подписок &nbsp;·&nbsp; Web-панель</b>
</p>

---

## Что такое Anneal

Anneal - это native control panel для подписочного прокси-доступа: пользователи, лимиты, аудит, доставка подписок и Mihomo-совместимые клиентские конфиги из одного Rust control plane.

Панель рассчитана на мультитенантные среды, где админы, реселлеры и пользователи должны видеть только свою часть данных без лишней runtime-оркестрации.

---

## Возможности

```text
Мультиарендность     - роли superadmin / admin / reseller / user
Web-интерфейс        - пользователи, подписки, usage, уведомления
API на Rust          - миграции, аудит, TOTP, quota state, auth sessions
Native installer     - PostgreSQL, Caddy, API, worker, web-панель, Mihomo
Mihomo runtime       - bundled binary, systemd service, generated config
Подписки             - raw links, base64 bundles, Mihomo YAML
```

---

## Состав репозитория

| Путь | Назначение |
|------|-----------|
| `apps/api` | HTTP API, авторизация, OpenAPI, transport-слой |
| `apps/annealctl` | Native installer, update и управление сервисами |
| `apps/worker` | Фоновый worker уведомлений |
| `crates/config-engine` | Рендер Mihomo и share-link конфигов |
| `crates/subscriptions` | Подписки, ссылки выдачи, устройства |
| `crates/users` | Пользователи, реселлеры, tenants |
| `crates/usage` | Usage samples, rollups, quota state |
| `web` | Фронтенд - React / Vite |
| `deploy/systemd` | Native systemd units |
| `migrations` | SQL-схема PostgreSQL |

---

## Установка

В Anneal есть bootstrap-обёртка, которая скачивает один готовый релизный архив из GitHub Releases, проверяет встроенные SHA256-суммы и запускает `annealctl install --bundle-root ...`.

- Файл установщика: [`scripts/install.sh`](./scripts/install.sh)
- Прямая ссылка: [raw install.sh](https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh)

Быстрый запуск:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash
```

Эта команда открывает вопросы установщика, если обязательные значения не переданы через CLI-флаги.

Модель релизов:
- raw `master` installer в первую очередь берёт rolling-release `rolling-master`, а на последний GitHub Release падает только как fallback
- push в `master` обновляет bundle `rolling-master`, а стабильные bundle публикуются по semver-тегам `0.1.0` и `v0.1.0`
- для фиксации версии можно передать `ANNEAL_RELEASE_TAG=0.1.0`

Поддерживаемые дистрибутивы:
- Debian 10, 11, 12, 13
- Ubuntu 22.04 LTS, 24.04 LTS, 25.04, 25.10

Какие репозитории использует установщик:
- PostgreSQL 17 ставится из официального PGDG-репозитория; для Debian 10 используется официальный PGDG archive
- Caddy ставится из официального Caddy APT-репозитория
- Mihomo лежит внутри Anneal release bundle и ставится как `anneal-mihomo.service`

Установка конкретной версии:

```bash
curl -fsSLo /tmp/anneal-install.sh https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh
sudo ANNEAL_RELEASE_TAG=0.1.0 bash /tmp/anneal-install.sh
```

Non-interactive установка control-plane:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- \
  --domain panel.example.com \
  --non-interactive
```

Установщик:
- использует `annealctl` как единственный источник правды для `install`, `resume`, `status`, `doctor`, `update`, `restart` и `uninstall`
- поддерживает интерактивные вопросы из one-line bootstrap-команды и non-interactive запуск через CLI-флаги
- устанавливает только native control plane: API, worker, web-панель, PostgreSQL, Caddy и Mihomo
- скачивает ровно один релизный архив вида `anneal-rolling-master-linux-amd64.tar.gz` или `anneal-0.1.0-linux-amd64.tar.gz`
- распаковывает bundle и запускает встроенный `bin/annealctl`
- генерирует panel path, database URL, admin credentials, reseller defaults, starter subscription и secrets
- пишет типизированное состояние в `/etc/anneal/install.toml`, `/var/lib/anneal/install-state.json`, `/etc/anneal/anneal.env` и `/etc/anneal/admin-summary.env`
- запускает `postgresql`, `anneal-api.service`, `anneal-worker.service`, `anneal-caddy.service` и `anneal-mihomo.service`

После установки:
- используйте `annealctl status`, `annealctl doctor`, `annealctl restart`, `annealctl update --bundle-root ...` и `annealctl uninstall`
- после рестарта VPS/VDS сервисы Anneal и Mihomo автоматически поднимутся через systemd

---

## Ключевые сценарии

- Управление пользователями, реселлерами, подписками, лимитами и сроками действия
- Выдача raw links, base64 link bundles и Mihomo YAML конфигов
- Учёт usage rollups и quota state
- Аудит security-sensitive действий
- Запуск control plane и Mihomo как native systemd services

---

## Участие в разработке

Контрибьюторы приветствуются. Баг-репорт, идея для фичи, правка в доках или pull request помогают проекту двигаться вперёд.

Если планируешь контрибьютить код - сначала открой issue, чтобы обсудить направление. Для мелких фиксов и улучшений можно сразу отправлять PR.

> [!NOTE]
> Проект находится в активной разработке. Часть кодовой базы ещё формируется, поэтому держите изменения сфокусированными и проверенными.
