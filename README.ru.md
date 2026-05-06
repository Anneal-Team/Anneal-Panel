# Anneal

Anneal - это Rust control-plane для пользователей, подписок, лимитов трафика, аудита и выдачи Mihomo-совместимых клиентских конфигов.

Текущая сборка намеренно оставлена только в native-режиме. Установщик ставит API, worker, web-панель, Caddy, PostgreSQL и bundled Mihomo runtime. Legacy-оркестрация серверных узлов, контейнерный deploy и старые runtime-ядра удалены.

## Структура

| Путь | Назначение |
| --- | --- |
| `apps/api` | HTTP API, auth, OpenAPI, transport layer |
| `apps/annealctl` | Native installer, update и управление сервисами |
| `apps/worker` | Фоновый worker уведомлений |
| `crates/config-engine` | Рендер Mihomo/share-link конфигов |
| `crates/subscriptions` | Подписки, delivery links, устройства |
| `crates/users` | Пользователи, реселлеры, tenant-ы |
| `crates/usage` | Usage samples, rollups, quota state |
| `web` | React/Vite панель |
| `deploy/systemd` | Native systemd units |
| `migrations` | PostgreSQL схема |

## Установка

Собрать release bundle:

```bash
scripts/package-release.sh
```

Установить bundle на сервере:

```bash
sudo ./install.sh --bundle-root /path/to/anneal-bundle
```

Установщик пишет `/etc/anneal/install.toml`, `/etc/anneal/anneal.env`, `/etc/anneal/admin-summary.env`, `/var/lib/anneal/install-state.json` и запускает:

- `postgresql`
- `anneal-api.service`
- `anneal-worker.service`
- `anneal-caddy.service`
- `anneal-mihomo.service`

## Операции

```bash
annealctl status
annealctl doctor
annealctl restart
annealctl update --bundle-root /path/to/new-bundle
annealctl uninstall
```

## Runtime

Подписки отдаются как raw links, base64 link list или Mihomo YAML в зависимости от режима запроса и client headers. Установщик создает минимальный native-конфиг Mihomo, а данные delivery endpoint-ов берутся из настроек API.
