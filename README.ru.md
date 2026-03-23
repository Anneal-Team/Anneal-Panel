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
  <b>Управление серверными группами &nbsp;·&nbsp; Доменные правила &nbsp;·&nbsp; Доставка подписок &nbsp;·&nbsp; Клиентские конфиги</b>
</p>

---

## 🔥 Что такое Anneal

Anneal — это панель управления, которая объединяет всё необходимое для эксплуатации прокси-инфраструктуры в масштабе: runtime-агенты, правила маршрутизации по доменам, автоматическая генерация точек входа, управление подписками и выдача клиентских конфигов.

Разработана для мультитенантных сред, где разные команды, реселлеры и пользователи нуждаются в изолированном контроле над своей частью инфраструктуры.

---

## ⚡ Возможности

```
🏢  Мультиарендность     —  роли superadmin / admin / reseller / user
🖥️  Web-интерфейс        —  ноды, пользователи, подписки, доменные правила
🦀  API на Rust          —  миграции, аудит, TOTP, usage, уведомления
🤖  Агент сервера        —  регистрация runtime, heartbeat, rollout-задачи
🌐  Генерация endpoint   —  direct / legacy_direct / cdn / auto_cdn / relay / worker / reality / fake
📦  Подписки             —  конфиги xray и sing-box, ссылки, лимиты на устройство
```

---

## 📁 Состав репозитория

| Путь | Назначение |
|------|-----------|
| `apps/api` | 🔌 HTTP API, авторизация, Swagger UI, transport-слой |
| `apps/node-agent` | 🤖 Агент сервера — регистрация, heartbeat, rollout |
| `apps/worker` | ⚙️ Фоновые задачи и обработка очередей |
| `crates/nodes` | 🗂️ Серверные группы, домены, endpoint-ы, rollout orchestration |
| `crates/subscriptions` | 📋 Подписки, ссылки выдачи, устройства |
| `crates/users` | 👥 Пользователи, реселлеры, tenant-ы |
| `crates/config-engine` | 🔧 Генерация клиентских конфигов и bundle-форматов |
| `web` | 🎨 Фронтенд — React / Vite |
| `deploy/docker` | 🐳 Docker-образы и конфиги окружения |
| `migrations` | 🗄️ SQL-миграции PostgreSQL |

---

## 📦 Установка

В Anneal есть интерактивный установщик, который скачивает один готовый релизный архив из GitHub Releases и проводит установку через TUI-интерфейс.

- Файл установщика: [`scripts/install.sh`](./scripts/install.sh)
- Прямая ссылка: [raw install.sh](https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh)

Быстрый запуск:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash
```

Модель релизов:
- установщик по умолчанию сам берёт последний GitHub Release
- semver-релизы публикуются из Git-тегов вроде `v0.1.0`
- для жёсткой фиксации версии можно передать `ANNEAL_RELEASE_TAG=v0.1.0`

Поддерживаемые дистрибутивы:
- Debian 10, 11, 12, 13
- Ubuntu 22.04 LTS, 24.04 LTS, 25.04, 25.10

Какие репозитории использует установщик:
- PostgreSQL 17 ставится из официального PGDG-репозитория; для Debian 10 используется официальный PGDG archive
- Caddy ставится из официального Caddy APT-репозитория
- для Docker-режима используется официальный Docker-репозиторий там, где он доступен, а на старых платформах вроде Debian 10 идёт fallback на пакеты дистрибутива

Установка конкретной версии:

```bash
curl -fsSLo /tmp/anneal-install.sh https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh
sudo ANNEAL_RELEASE_TAG=v0.1.0 bash /tmp/anneal-install.sh
```

Установщик:
- спрашивает язык интерфейса: `Русский` или `English`
- предлагает выбрать роль сервера: `Panel` или `Node`
- предлагает выбрать тип установки: `Linux` или `Docker`
- скачивает один релизный архив вида `anneal-0.1.0-linux-amd64.tar.gz` вместо сборки проекта на сервере
- автоматически генерирует пароли, токены и внутренние секреты
- после установки показывает сводку для администратора
- ставит login-menu с действиями status, update, restart и remove

Сервер панели:
- устанавливает control-plane: web-панель, API, worker, базу и edge-сервисы

Сервер ноды:
- устанавливает отдельный VPS/VDS node server
- это не ядро Xray или Sing-box, а отдельный сервер под управлением Anneal
- распаковывает runtime-бинарники из того же релизного архива и запускает их в native или Docker режиме

Примеры запуска по роли:

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- --role control-plane
```

```bash
curl -fsSL https://raw.githubusercontent.com/Anneal-Team/Anneal-Panel/master/scripts/install.sh | sudo bash -s -- --role node
```

---

## 🎯 Ключевые сценарии

- 🏗️ Создание серверной группы и подключение runtime-агентов
- 🌍 Настройка доменных правил и автоматическая генерация точек входа
- 📬 Выпуск и редактирование подписок с лимитами и сроками
- 📱 Выдача клиентских ссылок и конфигов по устройствам
- 📊 Контроль rollout-ов, состояния нод, usage и уведомлений

---

## 🤝 Участие в разработке

Контрибьюторы приветствуются! Баг-репорт, идея для фичи, правка в доках или пул-реквест — любой вклад помогает проекту двигаться вперёд.

Если планируешь контрибьютить код — сначала открой issue, чтобы обсудить направление. Для мелких фиксов и улучшений можно сразу отправлять PR.

> [!NOTE]
> Проект находится в активной разработке. Часть кодовой базы ещё формируется — самое время войти в проект на раннем этапе.

---

## 💜 Благодарности

Отдельное спасибо команде **[Hiddify](https://github.com/hiddify)** за вклад в экосистему и сильные идеи вокруг удобной настройки доменов, структуры клиентских конфигов и механизмов доставки.
