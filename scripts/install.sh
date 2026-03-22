#!/usr/bin/env bash
set -euo pipefail

generate_hex() {
  openssl rand -hex "${1:-16}"
}

generate_secret() {
  openssl rand -base64 "${1:-24}" | tr -d '\n' | tr '/+=' '._-'
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

text() {
  local ru="$1"
  local en="$2"
  if [[ "${ANNEAL_INSTALLER_LANG}" == "ru" ]]; then
    printf '%s' "${ru}"
    return
  fi
  printf '%s' "${en}"
}

setup_locale() {
  if [[ "${LANG:-}" != *UTF-8* && "${LANG:-}" != *utf8* ]]; then
    export LANG=C.UTF-8
  fi
  if [[ "${LC_ALL:-}" != *UTF-8* && "${LC_ALL:-}" != *utf8* ]]; then
    export LC_ALL="${LANG}"
  fi
}

setup_palette() {
  export NEWT_COLORS="${NEWT_COLORS:-root=black,black window=black,black border=lightgreen,black title=white,black roottext=lightgreen,black textbox=white,black entry=black,white button=black,lightgreen actbutton=white,green compactbutton=black,lightgreen checkbox=white,black actcheckbox=black,lightgreen label=white,black listbox=white,black actlistbox=black,lightgreen shadow=black,black}"
}

installer_backtitle() {
  text "Anneal • Установка" "Anneal • Installer"
}

dialog_select_label() {
  text "Выбрать" "Select"
}

dialog_back_label() {
  text "Назад" "Back"
}

dialog_confirm_label() {
  text "Подтвердить" "Confirm"
}

dialog_close_label() {
  text "Закрыть" "Close"
}

menu_hint() {
  text "↑↓ выбрать • Enter подтвердить • Tab кнопки" "↑↓ move • Enter confirm • Tab buttons"
}

checklist_hint() {
  text "↑↓ выбрать • Space переключить • Enter подтвердить" "↑↓ move • Space toggle • Enter confirm"
}

input_hint() {
  text "Введи значение • Enter сохранить • Tab кнопки" "Enter value • Enter save • Tab buttons"
}

confirm_hint() {
  text "←→ выбор • Enter подтвердить" "←→ choose • Enter confirm"
}

logo_block() {
  printf '%s' '▁▃▆█ Anneal'
}

brand_text() {
  local body="$1"
  local hint="${2:-}"
  if [[ -n "${hint}" ]]; then
    printf '%s\n\n%s\n\n%s' "$(logo_block)" "${body}" "${hint}"
    return
  fi
  printf '%s\n\n%s' "$(logo_block)" "${body}"
}

print_banner() {
  printf '\033[38;5;150m'
  printf '      ▂\n'
  printf '    ▂▄\n'
  printf '  ▂▄▆█  '
  printf '\033[38;5;194mAnn\033[38;5;150meal\033[0m\n'
  printf '\n'
}

require_root() {
  if [[ "${EUID}" -ne 0 ]]; then
    text "Запусти установщик от root." "Run the installer as root." >&2
    printf '\n' >&2
    exit 1
  fi
}

is_interactive_session() {
  [[ -t 0 && -t 1 ]]
}

use_tui() {
  [[ "${ANNEAL_INSTALLER_UI}" == "tui" ]] && return 0
  [[ "${ANNEAL_INSTALLER_UI}" == "plain" ]] && return 1
  is_interactive_session
}

ensure_whiptail() {
  setup_palette
  if command_exists whiptail; then
    return
  fi
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y whiptail
}

prompt_text() {
  local title="$1"
  local prompt="$2"
  local default_value="${3:-}"
  whiptail \
    --backtitle "$(installer_backtitle)" \
    --title "${title}" \
    --ok-button "$(dialog_select_label)" \
    --cancel-button "$(dialog_back_label)" \
    --inputbox "$(brand_text "${prompt}" "$(input_hint)")" 18 92 "${default_value}" 3>&1 1>&2 2>&3
}

prompt_menu() {
  local title="$1"
  local prompt="$2"
  shift 2
  whiptail \
    --backtitle "$(installer_backtitle)" \
    --title "${title}" \
    --ok-button "$(dialog_select_label)" \
    --cancel-button "$(dialog_back_label)" \
    --menu "$(brand_text "${prompt}" "$(menu_hint)")" 22 92 8 "$@" 3>&1 1>&2 2>&3
}

prompt_checklist() {
  local title="$1"
  local prompt="$2"
  shift 2
  local result
  result="$(whiptail \
    --backtitle "$(installer_backtitle)" \
    --title "${title}" \
    --ok-button "$(dialog_select_label)" \
    --cancel-button "$(dialog_back_label)" \
    --checklist "$(brand_text "${prompt}" "$(checklist_hint)")" 24 92 10 "$@" 3>&1 1>&2 2>&3)"
  echo "${result}" | tr -d '"' | xargs | tr ' ' ','
}

prompt_confirm() {
  local title="$1"
  local prompt="$2"
  whiptail \
    --backtitle "$(installer_backtitle)" \
    --title "${title}" \
    --yes-button "$(dialog_confirm_label)" \
    --no-button "$(dialog_back_label)" \
    --yesno "$(brand_text "${prompt}" "$(confirm_hint)")" 20 92
}

show_info() {
  local title="$1"
  local message="$2"
  if use_tui; then
    whiptail \
      --backtitle "$(installer_backtitle)" \
      --title "${title}" \
      --ok-button "$(dialog_close_label)" \
      --msgbox "$(brand_text "${message}")" 20 92
    return
  fi
  printf '%s\n' "${message}"
}

show_error() {
  local message="$1"
  if use_tui; then
    whiptail \
      --backtitle "$(installer_backtitle)" \
      --title "$(text "Ошибка" "Error")" \
      --ok-button "$(dialog_close_label)" \
      --msgbox "$(brand_text "${message}")" 20 92
    return
  fi
  printf '%s\n' "${message}" >&2
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --action)
        ACTION="$2"
        shift 2
        ;;
      --role)
        ROLE="$2"
        shift 2
        ;;
      --mode|--deployment-mode)
        DEPLOYMENT_MODE="$2"
        shift 2
        ;;
      --lang)
        ANNEAL_INSTALLER_LANG="$2"
        shift 2
        ;;
      --login-shell)
        LOGIN_SHELL=1
        shift
        ;;
      *)
        show_error "$(text "Неизвестный аргумент: $1" "Unknown argument: $1")"
        exit 1
        ;;
    esac
  done
}

choose_language() {
  local choice
  if [[ -n "${ANNEAL_INSTALLER_LANG:-}" && ( "${ANNEAL_INSTALLER_LANG}" == "ru" || "${ANNEAL_INSTALLER_LANG}" == "en" ) ]]; then
    return
  fi
  ANNEAL_INSTALLER_LANG="ru"
  if ! use_tui; then
    return
  fi
  ensure_whiptail
  choice="$(prompt_menu "Anneal" "Language / Язык" \
    "Русский" "Интерфейс на русском" \
    "English" "English interface")"
  case "${choice}" in
    Русский) ANNEAL_INSTALLER_LANG="ru" ;;
    English) ANNEAL_INSTALLER_LANG="en" ;;
  esac
}

choose_role() {
  local choice
  if [[ -n "${ROLE}" ]]; then
    return
  fi
  choice="$(prompt_menu \
    "$(text "Anneal • Роль" "Anneal • Role")" \
    "$(text "Выбери, что устанавливается на этот сервер." "Choose what will be installed on this server.")" \
    "$(text "Панель" "Panel")" "$(text "UI, API, worker и база" "UI, API, worker and database")" \
    "$(text "Нода" "Node")" "$(text "Отдельный VPS/VDS сервер для runtime-пакетов" "Separate VPS/VDS server for runtime packages")")"
  case "${choice}" in
    Панель|Panel) ROLE="control-plane" ;;
    Нода|Node) ROLE="node" ;;
  esac
}

choose_deployment_mode() {
  local choice
  if [[ -n "${DEPLOYMENT_MODE}" ]]; then
    return
  fi
  choice="$(prompt_menu \
    "$(text "Anneal • Режим" "Anneal • Mode")" \
    "$(text "Выбери способ установки." "Choose the deployment mode.")" \
    "Linux" "$(text "Нативная установка в систему" "Native installation into the system")" \
    "Docker" "$(text "Запуск готовых пакетов в контейнере" "Run prebuilt packages in a container")")"
  case "${choice}" in
    Linux) DEPLOYMENT_MODE="native" ;;
    Docker) DEPLOYMENT_MODE="docker" ;;
  esac
}

selected_engine() {
  local engine="$1"
  [[ ",${ANNEAL_AGENT_ENGINES}," == *",${engine},"* ]]
}

finalize_control_plane_defaults() {
  if [[ -z "${ANNEAL_DOMAIN}" ]]; then
    ANNEAL_DOMAIN="panel.example.com"
  fi
  if [[ -z "${ANNEAL_PUBLIC_BASE_URL}" ]]; then
    ANNEAL_PUBLIC_BASE_URL="https://${ANNEAL_DOMAIN}"
  fi
  if [[ -z "${ANNEAL_SUPERADMIN_EMAIL}" ]]; then
    ANNEAL_SUPERADMIN_EMAIL="admin-$(generate_hex 3)@${ANNEAL_DOMAIN}"
  fi
}

finalize_node_defaults() {
  if [[ -z "${ANNEAL_AGENT_NAME}" ]]; then
    ANNEAL_AGENT_NAME="node-$(generate_hex 3)"
  fi
  if [[ -z "${ANNEAL_AGENT_ENGINES}" ]]; then
    ANNEAL_AGENT_ENGINES="xray,singbox"
  fi
  if [[ -z "${ANNEAL_AGENT_PROTOCOLS_XRAY}" ]]; then
    ANNEAL_AGENT_PROTOCOLS_XRAY="vless_reality,vmess,trojan,shadowsocks_2022"
  fi
  if [[ -z "${ANNEAL_AGENT_PROTOCOLS_SINGBOX}" ]]; then
    ANNEAL_AGENT_PROTOCOLS_SINGBOX="vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2"
  fi
}

build_node_enrollment_tokens() {
  local pairs=()
  if selected_engine xray; then
    if [[ -z "${ANNEAL_AGENT_XRAY_TOKEN}" ]]; then
      show_error "$(text "Для runtime Xray нужен enrollment token." "Xray runtime requires an enrollment token.")"
      exit 1
    fi
    pairs+=("xray:${ANNEAL_AGENT_XRAY_TOKEN}")
  fi
  if selected_engine singbox; then
    if [[ -z "${ANNEAL_AGENT_SINGBOX_TOKEN}" ]]; then
      show_error "$(text "Для runtime Sing-box нужен enrollment token." "Sing-box runtime requires an enrollment token.")"
      exit 1
    fi
    pairs+=("singbox:${ANNEAL_AGENT_SINGBOX_TOKEN}")
  fi
  if [[ "${#pairs[@]}" -eq 0 ]]; then
    show_error "$(text "Выбери хотя бы один runtime для ноды." "Select at least one runtime for the node server.")"
    exit 1
  fi
  ANNEAL_AGENT_ENROLLMENT_TOKENS="$(IFS=,; echo "${pairs[*]}")"
}

control_plane_summary() {
  cat <<EOF
$(text "Роль" "Role"): control-plane
$(text "Режим" "Mode"): ${DEPLOYMENT_MODE}
$(text "Домен" "Domain"): ${ANNEAL_DOMAIN}
panel_url: ${ANNEAL_PUBLIC_BASE_URL}
$(text "Email суперадмина" "Superadmin email"): ${ANNEAL_SUPERADMIN_EMAIL}
$(text "Версия канала" "Release channel"): ${ANNEAL_RELEASE_TAG}
EOF
}

node_summary() {
  cat <<EOF
$(text "Роль" "Role"): node-server
$(text "Режим" "Mode"): ${DEPLOYMENT_MODE}
$(text "Control Plane URL" "Control Plane URL"): ${ANNEAL_AGENT_SERVER_URL}
$(text "Имя ноды" "Node name"): ${ANNEAL_AGENT_NAME}
$(text "Runtime-пакеты" "Runtime packages"): ${ANNEAL_AGENT_ENGINES}
$(text "Версия канала" "Release channel"): ${ANNEAL_RELEASE_TAG}
EOF
}

configure_control_plane_tui() {
  finalize_control_plane_defaults
  ANNEAL_DOMAIN="$(prompt_text \
    "$(text "Anneal • Control Plane" "Anneal • Control Plane")" \
    "$(text "Укажи домен панели." "Enter the panel domain.")" \
    "${ANNEAL_DOMAIN}")"
  finalize_control_plane_defaults
  ANNEAL_PUBLIC_BASE_URL="$(prompt_text \
    "$(text "Anneal • Control Plane" "Anneal • Control Plane")" \
    "$(text "Публичный URL панели." "Enter the public panel URL.")" \
    "${ANNEAL_PUBLIC_BASE_URL}")"
  ANNEAL_SUPERADMIN_EMAIL="$(prompt_text \
    "$(text "Anneal • Control Plane" "Anneal • Control Plane")" \
    "$(text "Email bootstrap-суперадмина." "Enter the bootstrap superadmin email.")" \
    "${ANNEAL_SUPERADMIN_EMAIL}")"
  ANNEAL_SUPERADMIN_DISPLAY_NAME="$(prompt_text \
    "$(text "Anneal • Control Plane" "Anneal • Control Plane")" \
    "$(text "Отображаемое имя суперадмина." "Enter the superadmin display name.")" \
    "${ANNEAL_SUPERADMIN_DISPLAY_NAME}")"
  if ! prompt_confirm "$(text "Подтверждение" "Confirmation")" "$(control_plane_summary)"; then
    exit 1
  fi
}

configure_node_tui() {
  finalize_node_defaults
  ANNEAL_AGENT_SERVER_URL="$(prompt_text \
    "$(text "Anneal • Node Server" "Anneal • Node Server")" \
    "$(text "Укажи URL control-plane API." "Enter the control-plane API URL.")" \
    "${ANNEAL_AGENT_SERVER_URL:-https://panel.example.com}")"
  ANNEAL_AGENT_NAME="$(prompt_text \
    "$(text "Anneal • Node Server" "Anneal • Node Server")" \
    "$(text "Имя node server." "Enter the node server name.")" \
    "${ANNEAL_AGENT_NAME}")"
  ANNEAL_AGENT_ENGINES="$(prompt_checklist \
    "$(text "Anneal • Runtime-пакеты" "Anneal • Runtime packages")" \
    "$(text "Выбери runtime-пакеты для этой ноды." "Choose runtime packages for this node server.")" \
    "xray" "$(text "Xray • vless/vmess/trojan/ss2022" "Xray • vless/vmess/trojan/ss2022")" "ON" \
    "singbox" "$(text "Sing-box • tuic/hysteria2 + classic" "Sing-box • tuic/hysteria2 + classic")" "ON")"
  if selected_engine xray; then
    ANNEAL_AGENT_XRAY_TOKEN="$(prompt_text \
      "$(text "Anneal • Xray Token" "Anneal • Xray Token")" \
      "$(text "Enrollment token для Xray на этой ноде." "Enter the Xray enrollment token for this node server.")" \
      "${ANNEAL_AGENT_XRAY_TOKEN}")"
  fi
  if selected_engine singbox; then
    ANNEAL_AGENT_SINGBOX_TOKEN="$(prompt_text \
      "$(text "Anneal • Sing-box Token" "Anneal • Sing-box Token")" \
      "$(text "Enrollment token для Sing-box на этой ноде." "Enter the Sing-box enrollment token for this node server.")" \
      "${ANNEAL_AGENT_SINGBOX_TOKEN}")"
  fi
  build_node_enrollment_tokens
  if ! prompt_confirm "$(text "Подтверждение" "Confirmation")" "$(node_summary)"; then
    exit 1
  fi
}

configure_installation() {
  choose_language
  if use_tui; then
    ensure_whiptail
    choose_role
    choose_deployment_mode
    case "${ROLE}" in
      control-plane) configure_control_plane_tui ;;
      node) configure_node_tui ;;
      *)
        show_error "$(text "Неизвестная роль." "Unknown role.")"
        exit 1
        ;;
    esac
  else
    [[ -n "${ROLE}" ]] || {
      show_error "$(text "Передай --role control-plane|node." "Pass --role control-plane|node.")"
      exit 1
    }
    [[ -n "${DEPLOYMENT_MODE}" ]] || {
      show_error "$(text "Передай --mode native|docker." "Pass --mode native|docker.")"
      exit 1
    }
    if [[ "${ROLE}" == "control-plane" ]]; then
      finalize_control_plane_defaults
    else
      finalize_node_defaults
      build_node_enrollment_tokens
    fi
  fi
}

control_utility_source_url() {
  printf 'https://raw.githubusercontent.com/%s/%s/scripts/install.sh' "${ANNEAL_GITHUB_REPOSITORY}" "${ANNEAL_RELEASE_TAG}"
}

download_release_asset() {
  local asset="$1"
  local destination="$2"
  curl --retry 5 --retry-all-errors --location --silent --show-error \
    "${ANNEAL_RELEASE_BASE_URL}/${asset}" \
    -o "${destination}"
}

prepare_deploy_assets() {
  if [[ -d "${SCRIPT_DIR}/../deploy" && -f "${SCRIPT_DIR}/../deploy/systemd/anneal-api.service" ]]; then
    DEPLOY_ASSET_ROOT="${SCRIPT_DIR}/../deploy"
    return
  fi
  DEPLOY_TEMP_DIR="$(mktemp -d)"
  download_release_asset "deploy-bundle.tar.gz" "${DEPLOY_TEMP_DIR}/deploy-bundle.tar.gz"
  tar -xzf "${DEPLOY_TEMP_DIR}/deploy-bundle.tar.gz" -C "${DEPLOY_TEMP_DIR}"
  DEPLOY_ASSET_ROOT="${DEPLOY_TEMP_DIR}/deploy"
}

cleanup_temp_dir() {
  if [[ -n "${DEPLOY_TEMP_DIR:-}" && -d "${DEPLOY_TEMP_DIR}" ]]; then
    rm -rf "${DEPLOY_TEMP_DIR}"
  fi
}

setup_postgres_repository() {
  if [[ -f /etc/apt/sources.list.d/pgdg.list ]]; then
    return
  fi
  local codename
  codename="$(. /etc/os-release && echo "${VERSION_CODENAME}")"
  install -d -m 0755 /usr/share/postgresql-common/pgdg
  curl --retry 5 --retry-all-errors --location --silent --show-error \
    https://www.postgresql.org/media/keys/ACCC4CF8.asc |
    gpg --dearmor >/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc
  cat >/etc/apt/sources.list.d/pgdg.list <<EOF
deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] https://apt.postgresql.org/pub/repos/apt ${codename}-pgdg main
EOF
}

install_native_control_plane_packages() {
  export DEBIAN_FRONTEND=noninteractive
  setup_postgres_repository
  apt-get update
  apt-get install -y curl tar unzip ca-certificates gnupg lsb-release openssl jq whiptail iproute2 postgresql-17 postgresql-client-17 postgresql-contrib-17 caddy
}

install_native_node_packages() {
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y curl tar ca-certificates openssl jq whiptail iproute2
}

install_docker_packages() {
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y curl tar ca-certificates openssl jq whiptail iproute2 docker.io docker-compose-plugin
  systemctl enable --now docker
}

compose_cmd() {
  if docker compose version >/dev/null 2>&1; then
    docker compose "$@"
    return
  fi
  if command_exists docker-compose; then
    docker-compose "$@"
    return
  fi
  show_error "$(text "Docker Compose не найден." "Docker Compose was not found.")"
  exit 1
}

parse_database_components() {
  local host_and_port
  DB_NAME="${ANNEAL_DATABASE_URL##*/}"
  DB_NAME="${DB_NAME%%\?*}"
  DB_USER="$(echo "${ANNEAL_DATABASE_URL}" | sed -E 's#^postgres://([^:]+):.*#\1#')"
  DB_PASSWORD="$(echo "${ANNEAL_DATABASE_URL}" | sed -E 's#^postgres://[^:]+:([^@]+)@.*#\1#')"
  host_and_port="$(echo "${ANNEAL_DATABASE_URL}" | sed -E 's#^postgres://[^@]+@([^/]+)/.*#\1#')"
  DB_HOST="${host_and_port%%:*}"
  DB_PORT="${host_and_port##*:}"
  if [[ "${DB_HOST}" == "${DB_PORT}" ]]; then
    DB_PORT="5432"
  fi
}

ensure_postgres() {
  parse_database_components
  if [[ "${DB_HOST}" != "127.0.0.1" && "${DB_HOST}" != "localhost" ]]; then
    return
  fi
  systemctl enable --now postgresql
  runuser -u postgres -- psql -p "${DB_PORT}" -tAc "select 1 from pg_roles where rolname='${DB_USER}'" | grep -q 1 || runuser -u postgres -- psql -p "${DB_PORT}" -c "create role ${DB_USER} login password '${DB_PASSWORD}';"
  runuser -u postgres -- psql -p "${DB_PORT}" -tAc "select 1 from pg_database where datname='${DB_NAME}'" | grep -q 1 || runuser -u postgres -- createdb -p "${DB_PORT}" -O "${DB_USER}" "${DB_NAME}"
}

ensure_user() {
  getent group "${ANNEAL_GROUP}" >/dev/null 2>&1 || groupadd --system "${ANNEAL_GROUP}"
  id -u "${ANNEAL_USER}" >/dev/null 2>&1 || useradd --system --gid "${ANNEAL_GROUP}" --home /var/lib/anneal --create-home --shell /usr/sbin/nologin "${ANNEAL_USER}"
  install -d -o "${ANNEAL_USER}" -g "${ANNEAL_GROUP}" /opt/anneal/bin /opt/anneal/web /opt/anneal/migrations /etc/anneal /var/lib/anneal
}

extract_archive() {
  local archive="$1"
  local destination="$2"
  case "${archive}" in
    *.zip) unzip -oq "${archive}" -d "${destination}" ;;
    *.tar.gz) tar -xzf "${archive}" -C "${destination}" ;;
    *)
      show_error "$(text "Неподдерживаемый архив: ${archive}" "Unsupported archive: ${archive}")"
      exit 1
      ;;
  esac
}

install_archive_contents() {
  local archive="$1"
  local destination="$2"
  local temp_dir
  temp_dir="$(mktemp -d)"
  extract_archive "${archive}" "${temp_dir}"
  shopt -s dotglob nullglob
  local extracted=("${temp_dir}"/*)
  rm -rf "${destination:?}"/*
  if [[ "${#extracted[@]}" -eq 1 && -d "${extracted[0]}" ]]; then
    cp -a "${extracted[0]}"/. "${destination}/"
  else
    cp -a "${temp_dir}"/. "${destination}/"
  fi
  shopt -u dotglob nullglob
  rm -rf "${temp_dir}"
}

install_runtime_defaults() {
  install -d -o "${ANNEAL_USER}" -g "${ANNEAL_GROUP}" /var/lib/anneal/xray /var/lib/anneal/singbox /var/lib/anneal/tls
  cat >/var/lib/anneal/xray/config.json <<EOF
{"log":{"loglevel":"warning"},"inbounds":[],"outbounds":[{"protocol":"freedom","tag":"direct"}]}
EOF
  cat >/var/lib/anneal/singbox/config.json <<EOF
{"log":{"level":"warn"},"inbounds":[],"outbounds":[{"type":"direct","tag":"direct"}]}
EOF
  if [[ ! -f /var/lib/anneal/tls/server.crt || ! -f /var/lib/anneal/tls/server.key ]]; then
    openssl req -x509 -nodes -newkey rsa:2048 \
      -keyout /var/lib/anneal/tls/server.key \
      -out /var/lib/anneal/tls/server.crt \
      -subj "/CN=${ANNEAL_DOMAIN:-anneal.local}" \
      -days 3650 >/dev/null 2>&1
  fi
  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /var/lib/anneal
}

install_runtime_bundle_native() {
  download_release_asset "runtime-bundle-${ANNEAL_TARGET_TRIPLE}.tar.gz" /tmp/runtime-bundle.tar.gz
  install_archive_contents /tmp/runtime-bundle.tar.gz /opt/anneal/bin
  chmod +x /opt/anneal/bin/xray /opt/anneal/bin/hiddify-core
}

docker_stack_root() {
  case "${ROLE}" in
    control-plane) echo "/opt/anneal/docker/control-plane" ;;
    node) echo "/opt/anneal/docker/node" ;;
    *)
      show_error "$(text "Неизвестная роль." "Unknown role.")"
      exit 1
      ;;
  esac
}

sync_docker_stack_assets() {
  local stack_root="$1"
  mkdir -p "${stack_root}"
  cp -a "${DEPLOY_ASSET_ROOT}/docker/prebuilt"/. "${stack_root}/"
}

write_control_plane_docker_files() {
  local stack_root="$1"
  cp "${stack_root}/control-plane.compose.yml" "${stack_root}/compose.yml"
  sed "s#{{SITE_ADDRESS}}#${ANNEAL_DOMAIN}#g" "${stack_root}/control-plane.Caddyfile.tpl" >"${stack_root}/Caddyfile"
}

write_node_docker_files() {
  local stack_root="$1"
  cp "${stack_root}/node.compose.yml" "${stack_root}/compose.yml"
  install -d "${stack_root}/data/xray" "${stack_root}/data/singbox" "${stack_root}/data/tls"
  cat >"${stack_root}/data/xray/config.json" <<EOF
{"log":{"loglevel":"warning"},"inbounds":[],"outbounds":[{"protocol":"freedom","tag":"direct"}]}
EOF
  cat >"${stack_root}/data/singbox/config.json" <<EOF
{"log":{"level":"warn"},"inbounds":[],"outbounds":[{"type":"direct","tag":"direct"}]}
EOF
  if [[ ! -f "${stack_root}/data/tls/server.crt" || ! -f "${stack_root}/data/tls/server.key" ]]; then
    openssl req -x509 -nodes -newkey rsa:2048 \
      -keyout "${stack_root}/data/tls/server.key" \
      -out "${stack_root}/data/tls/server.crt" \
      -subj "/CN=${ANNEAL_AGENT_NAME}" \
      -days 3650 >/dev/null 2>&1
  fi
}

write_control_plane_env_native() {
  cat >"${ENV_FILE}" <<EOF
ANNEAL_BIND_ADDRESS=127.0.0.1:8080
ANNEAL_DATABASE_URL=${ANNEAL_DATABASE_URL}
ANNEAL_MIGRATIONS_DIR=/opt/anneal/migrations
ANNEAL_ACCESS_JWT_SECRET=${ANNEAL_ACCESS_JWT_SECRET}
ANNEAL_PRE_AUTH_JWT_SECRET=${ANNEAL_PRE_AUTH_JWT_SECRET}
ANNEAL_PUBLIC_BASE_URL=${ANNEAL_PUBLIC_BASE_URL}
ANNEAL_CADDY_DOMAIN=${ANNEAL_DOMAIN}
ANNEAL_OTLP_ENDPOINT=${ANNEAL_OTLP_ENDPOINT}
EOF
  chmod 600 "${ENV_FILE}"
}

write_control_plane_env_docker() {
  local stack_root="$1"
  cat >"${stack_root}/.env" <<EOF
ANNEAL_RELEASE_BASE_URL=${ANNEAL_RELEASE_BASE_URL}
ANNEAL_TARGET_TRIPLE=${ANNEAL_TARGET_TRIPLE}
ANNEAL_DB_NAME=${ANNEAL_DB_NAME}
ANNEAL_DB_USER=${ANNEAL_DB_USER}
ANNEAL_DB_PASSWORD=${ANNEAL_DB_PASSWORD}
ANNEAL_BIND_ADDRESS=0.0.0.0:8080
ANNEAL_DATABASE_URL=postgres://${ANNEAL_DB_USER}:${ANNEAL_DB_PASSWORD}@postgres:5432/${ANNEAL_DB_NAME}
ANNEAL_MIGRATIONS_DIR=/opt/anneal/migrations
ANNEAL_ACCESS_JWT_SECRET=${ANNEAL_ACCESS_JWT_SECRET}
ANNEAL_PRE_AUTH_JWT_SECRET=${ANNEAL_PRE_AUTH_JWT_SECRET}
ANNEAL_PUBLIC_BASE_URL=${ANNEAL_PUBLIC_BASE_URL}
ANNEAL_CADDY_DOMAIN=${ANNEAL_DOMAIN}
ANNEAL_OTLP_ENDPOINT=${ANNEAL_OTLP_ENDPOINT}
EOF
  chmod 600 "${stack_root}/.env"
}

write_node_env_native() {
  cat >"${ENV_FILE}" <<EOF
ANNEAL_AGENT_SERVER_URL=${ANNEAL_AGENT_SERVER_URL}
ANNEAL_AGENT_NAME=${ANNEAL_AGENT_NAME}
ANNEAL_AGENT_VERSION=${ANNEAL_VERSION}
ANNEAL_AGENT_ENGINES=${ANNEAL_AGENT_ENGINES}
ANNEAL_AGENT_PROTOCOLS_XRAY=${ANNEAL_AGENT_PROTOCOLS_XRAY}
ANNEAL_AGENT_PROTOCOLS_SINGBOX=${ANNEAL_AGENT_PROTOCOLS_SINGBOX}
ANNEAL_AGENT_ENROLLMENT_TOKENS=${ANNEAL_AGENT_ENROLLMENT_TOKENS}
ANNEAL_AGENT_CONFIG_ROOT=/var/lib/anneal
ANNEAL_AGENT_XRAY_BINARY=/opt/anneal/bin/xray
ANNEAL_AGENT_SINGBOX_BINARY=/opt/anneal/bin/hiddify-core
ANNEAL_AGENT_RUNTIME_CONTROLLER=systemctl
ANNEAL_AGENT_SYSTEMCTL_BINARY=/usr/bin/systemctl
ANNEAL_AGENT_XRAY_SERVICE=anneal-xray.service
ANNEAL_AGENT_SINGBOX_SERVICE=anneal-singbox.service
EOF
  chmod 600 "${ENV_FILE}"
}

write_node_env_docker() {
  local stack_root="$1"
  cat >"${stack_root}/.env" <<EOF
ANNEAL_RELEASE_BASE_URL=${ANNEAL_RELEASE_BASE_URL}
ANNEAL_TARGET_TRIPLE=${ANNEAL_TARGET_TRIPLE}
ANNEAL_AGENT_SERVER_URL=${ANNEAL_AGENT_SERVER_URL}
ANNEAL_AGENT_NAME=${ANNEAL_AGENT_NAME}
ANNEAL_AGENT_VERSION=${ANNEAL_VERSION}
ANNEAL_AGENT_ENGINES=${ANNEAL_AGENT_ENGINES}
ANNEAL_AGENT_PROTOCOLS_XRAY=${ANNEAL_AGENT_PROTOCOLS_XRAY}
ANNEAL_AGENT_PROTOCOLS_SINGBOX=${ANNEAL_AGENT_PROTOCOLS_SINGBOX}
ANNEAL_AGENT_ENROLLMENT_TOKENS=${ANNEAL_AGENT_ENROLLMENT_TOKENS}
ANNEAL_AGENT_CONFIG_ROOT=/var/lib/anneal
ANNEAL_AGENT_RUNTIME_CONTROLLER=supervisorctl
ANNEAL_AGENT_SYSTEMCTL_BINARY=/usr/bin/supervisorctl
ANNEAL_AGENT_XRAY_SERVICE=xray
ANNEAL_AGENT_SINGBOX_SERVICE=singbox
EOF
  chmod 600 "${stack_root}/.env"
}

render_native_caddyfile() {
  sed "s#{{DOMAIN}}#${ANNEAL_DOMAIN}#g" "${DEPLOY_ASSET_ROOT}/caddy/Caddyfile.tpl" >/etc/anneal/Caddyfile
}

wait_for_api() {
  local url="http://127.0.0.1:8080/api/v1/health"
  for _ in $(seq 1 120); do
    if curl --silent --show-error --fail "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  show_error "$(text "API не поднялся вовремя." "API did not become ready in time.")"
  exit 1
}

bootstrap_superadmin() {
  local response_file
  local status
  response_file="$(mktemp)"
  status="$(
    curl -sS -o "${response_file}" -w '%{http_code}' \
      http://127.0.0.1:8080/api/v1/bootstrap \
      -H 'content-type: application/json' \
      --data "$(jq -nc --arg email "${ANNEAL_SUPERADMIN_EMAIL}" --arg display_name "${ANNEAL_SUPERADMIN_DISPLAY_NAME}" --arg password "${ANNEAL_SUPERADMIN_PASSWORD}" '{email:$email, display_name:$display_name, password:$password}')"
  )"
  if [[ "${status}" == "200" || "${status}" == "409" ]]; then
    rm -f "${response_file}"
    return
  fi
  cat "${response_file}" >&2
  rm -f "${response_file}"
  show_error "$(text "Не удалось выполнить bootstrap суперадмина." "Failed to bootstrap the superadmin.")"
  exit 1
}

write_kv_file() {
  local file="$1"
  shift
  : >"${file}"
  while [[ $# -gt 1 ]]; do
    printf '%s=%q\n' "$1" "$2" >>"${file}"
    shift 2
  done
  chmod 600 "${file}"
}

save_install_metadata() {
  local stack_root
  stack_root="$(docker_stack_root)"
  write_kv_file "${META_FILE}" \
    ANNEAL_INSTALLER_LANG "${ANNEAL_INSTALLER_LANG}" \
    ANNEAL_INSTALL_ROLE "${ROLE}" \
    ANNEAL_DEPLOYMENT_MODE "${DEPLOYMENT_MODE}" \
    ANNEAL_GITHUB_REPOSITORY "${ANNEAL_GITHUB_REPOSITORY}" \
    ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}" \
    ANNEAL_RELEASE_BASE_URL "${ANNEAL_RELEASE_BASE_URL}" \
    ANNEAL_TARGET_TRIPLE "${ANNEAL_TARGET_TRIPLE}" \
    ANNEAL_DOMAIN "${ANNEAL_DOMAIN}" \
    ANNEAL_PUBLIC_BASE_URL "${ANNEAL_PUBLIC_BASE_URL}" \
    ANNEAL_AGENT_SERVER_URL "${ANNEAL_AGENT_SERVER_URL}" \
    ANNEAL_AGENT_NAME "${ANNEAL_AGENT_NAME}" \
    ANNEAL_AGENT_ENGINES "${ANNEAL_AGENT_ENGINES}" \
    ANNEAL_STACK_ROOT "${stack_root}" \
    ANNEAL_COMPOSE_FILE "${stack_root}/compose.yml"
}

save_admin_summary() {
  if [[ "${ROLE}" == "control-plane" ]]; then
    write_kv_file "${SUMMARY_FILE}" \
      ANNEAL_PUBLIC_BASE_URL "${ANNEAL_PUBLIC_BASE_URL}" \
      ANNEAL_SUPERADMIN_EMAIL "${ANNEAL_SUPERADMIN_EMAIL}" \
      ANNEAL_SUPERADMIN_PASSWORD "${ANNEAL_SUPERADMIN_PASSWORD}" \
      ANNEAL_DATABASE_URL "${ANNEAL_DATABASE_URL}" \
      ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}"
    return
  fi
  write_kv_file "${SUMMARY_FILE}" \
    ANNEAL_AGENT_SERVER_URL "${ANNEAL_AGENT_SERVER_URL}" \
    ANNEAL_AGENT_NAME "${ANNEAL_AGENT_NAME}" \
    ANNEAL_AGENT_ENGINES "${ANNEAL_AGENT_ENGINES}" \
    ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}"
}

load_install_state() {
  [[ -f "${META_FILE}" ]] || {
    show_error "$(text "Файл состояния установки не найден." "Install state file was not found.")"
    exit 1
  }
  source /etc/anneal/install.meta
  ROLE="${ANNEAL_INSTALL_ROLE}"
  DEPLOYMENT_MODE="${ANNEAL_DEPLOYMENT_MODE}"
  ANNEAL_INSTALLER_LANG="${ANNEAL_INSTALLER_LANG:-ru}"
}

load_admin_summary() {
  [[ -f "${SUMMARY_FILE}" ]] && source /etc/anneal/admin-summary.env
}

install_control_utility() {
  install -d /usr/local/bin
  if [[ -f "${SELF_SOURCE}" && "${SELF_SOURCE}" != /dev/stdin && "${SELF_SOURCE}" != /dev/fd/* ]]; then
    install -m 0755 "${SELF_SOURCE}" "${CONTROL_UTILITY_PATH}"
    return
  fi
  curl --retry 5 --retry-all-errors --location --silent --show-error \
    "$(control_utility_source_url)" \
    -o "${CONTROL_UTILITY_PATH}"
  chmod 0755 "${CONTROL_UTILITY_PATH}"
}

install_profile_hook() {
  cat >"${PROFILE_HOOK_PATH}" <<'EOF'
if [ -x /usr/local/bin/annealctl ] && [ -f /etc/anneal/install.meta ] && [ -t 0 ] && [ -t 1 ] && [ -z "${ANNEAL_MENU_ACTIVE:-}" ] && [ -z "${SSH_ORIGINAL_COMMAND:-}" ] && [ "$(id -u)" -eq 0 ]; then
  export ANNEAL_MENU_ACTIVE=1
  /usr/local/bin/annealctl --action manage --login-shell || true
  unset ANNEAL_MENU_ACTIVE
fi
EOF
  chmod 0644 "${PROFILE_HOOK_PATH}"
}

control_plane_install_message() {
  cat <<EOF
$(text "Установка завершена." "Installation completed.")

panel_url: ${ANNEAL_PUBLIC_BASE_URL}
admin_email: ${ANNEAL_SUPERADMIN_EMAIL}
admin_password: ${ANNEAL_SUPERADMIN_PASSWORD}
database_url: ${ANNEAL_DATABASE_URL}
release_tag: ${ANNEAL_RELEASE_TAG}
EOF
}

node_install_message() {
  cat <<EOF
$(text "Установка завершена." "Installation completed.")

control_plane_url: ${ANNEAL_AGENT_SERVER_URL}
node_name: ${ANNEAL_AGENT_NAME}
runtimes: ${ANNEAL_AGENT_ENGINES}
release_tag: ${ANNEAL_RELEASE_TAG}
EOF
}

install_native_control_plane() {
  finalize_control_plane_defaults
  if [[ -z "${ANNEAL_DOMAIN}" ]]; then
    show_error "$(text "Для control-plane нужен домен." "Control-plane requires a domain.")"
    exit 1
  fi
  prepare_deploy_assets
  install_native_control_plane_packages
  ensure_user
  ensure_postgres
  download_release_asset "api-${ANNEAL_TARGET_TRIPLE}.tar.gz" /tmp/api.tar.gz
  download_release_asset "worker-${ANNEAL_TARGET_TRIPLE}.tar.gz" /tmp/worker.tar.gz
  download_release_asset "web.tar.gz" /tmp/web.tar.gz
  download_release_asset "migrations.tar.gz" /tmp/migrations.tar.gz
  install_archive_contents /tmp/api.tar.gz /opt/anneal/bin
  install_archive_contents /tmp/worker.tar.gz /opt/anneal/bin
  install_archive_contents /tmp/web.tar.gz /opt/anneal/web
  install_archive_contents /tmp/migrations.tar.gz /opt/anneal/migrations
  write_control_plane_env_native
  render_native_caddyfile
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service
  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  systemctl daemon-reload
  systemctl enable --now postgresql anneal-api anneal-worker anneal-caddy
  wait_for_api
  bootstrap_superadmin
}

install_native_node() {
  finalize_node_defaults
  prepare_deploy_assets
  install_native_node_packages
  ensure_user
  download_release_asset "node-agent-${ANNEAL_TARGET_TRIPLE}.tar.gz" /tmp/node-agent.tar.gz
  install_archive_contents /tmp/node-agent.tar.gz /opt/anneal/bin
  install_runtime_bundle_native
  install_runtime_defaults
  write_node_env_native
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-node-agent.service" /etc/systemd/system/anneal-node-agent.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-xray.service" /etc/systemd/system/anneal-xray.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-singbox.service" /etc/systemd/system/anneal-singbox.service
  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  systemctl daemon-reload
  systemctl enable --now anneal-xray anneal-singbox anneal-node-agent
}

install_docker_control_plane() {
  finalize_control_plane_defaults
  if [[ -z "${ANNEAL_DOMAIN}" ]]; then
    show_error "$(text "Для control-plane нужен домен." "Control-plane requires a domain.")"
    exit 1
  fi
  prepare_deploy_assets
  install_docker_packages
  local stack_root
  stack_root="$(docker_stack_root)"
  sync_docker_stack_assets "${stack_root}"
  write_control_plane_docker_files "${stack_root}"
  write_control_plane_env_docker "${stack_root}"
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" build
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" up -d
  wait_for_api
  bootstrap_superadmin
}

install_docker_node() {
  finalize_node_defaults
  prepare_deploy_assets
  install_docker_packages
  local stack_root
  stack_root="$(docker_stack_root)"
  sync_docker_stack_assets "${stack_root}"
  write_node_docker_files "${stack_root}"
  write_node_env_docker "${stack_root}"
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" build
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" up -d
}

service_status_line() {
  local service="$1"
  local status
  status="$(systemctl is-active "${service}" 2>/dev/null || true)"
  if [[ "${status}" == "active" ]]; then
    printf '[ok] %s: active\n' "${service}"
    return
  fi
  printf '[..] %s: %s\n' "${service}" "${status:-unknown}"
}

status_summary() {
  if [[ "${DEPLOYMENT_MODE}" == "native" ]]; then
    if [[ "${ROLE}" == "control-plane" ]]; then
      {
        service_status_line postgresql
        service_status_line anneal-api
        service_status_line anneal-worker
        service_status_line anneal-caddy
      }
      return
    fi
    {
      service_status_line anneal-node-agent
      service_status_line anneal-xray
      service_status_line anneal-singbox
    }
    return
  fi
  compose_cmd -f "${ANNEAL_COMPOSE_FILE}" --env-file "${ANNEAL_STACK_ROOT}/.env" ps
}

restart_current_install() {
  if [[ "${DEPLOYMENT_MODE}" == "native" ]]; then
    if [[ "${ROLE}" == "control-plane" ]]; then
      systemctl restart anneal-api anneal-worker anneal-caddy
      return
    fi
    systemctl restart anneal-node-agent anneal-xray anneal-singbox
    return
  fi
  compose_cmd -f "${ANNEAL_COMPOSE_FILE}" --env-file "${ANNEAL_STACK_ROOT}/.env" restart
}

update_native_control_plane() {
  prepare_deploy_assets
  download_release_asset "api-${ANNEAL_TARGET_TRIPLE}.tar.gz" /tmp/api.tar.gz
  download_release_asset "worker-${ANNEAL_TARGET_TRIPLE}.tar.gz" /tmp/worker.tar.gz
  download_release_asset "web.tar.gz" /tmp/web.tar.gz
  download_release_asset "migrations.tar.gz" /tmp/migrations.tar.gz
  install_archive_contents /tmp/api.tar.gz /opt/anneal/bin
  install_archive_contents /tmp/worker.tar.gz /opt/anneal/bin
  install_archive_contents /tmp/web.tar.gz /opt/anneal/web
  install_archive_contents /tmp/migrations.tar.gz /opt/anneal/migrations
  render_native_caddyfile
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service
  systemctl daemon-reload
  restart_current_install
}

update_native_node() {
  prepare_deploy_assets
  download_release_asset "node-agent-${ANNEAL_TARGET_TRIPLE}.tar.gz" /tmp/node-agent.tar.gz
  install_archive_contents /tmp/node-agent.tar.gz /opt/anneal/bin
  install_runtime_bundle_native
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-node-agent.service" /etc/systemd/system/anneal-node-agent.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-xray.service" /etc/systemd/system/anneal-xray.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-singbox.service" /etc/systemd/system/anneal-singbox.service
  systemctl daemon-reload
  restart_current_install
}

update_docker_current() {
  prepare_deploy_assets
  sync_docker_stack_assets "${ANNEAL_STACK_ROOT}"
  if [[ "${ROLE}" == "control-plane" ]]; then
    write_control_plane_docker_files "${ANNEAL_STACK_ROOT}"
  else
    write_node_docker_files "${ANNEAL_STACK_ROOT}"
  fi
  compose_cmd -f "${ANNEAL_COMPOSE_FILE}" --env-file "${ANNEAL_STACK_ROOT}/.env" build
  compose_cmd -f "${ANNEAL_COMPOSE_FILE}" --env-file "${ANNEAL_STACK_ROOT}/.env" up -d
}

update_current_install() {
  if [[ "${DEPLOYMENT_MODE}" == "native" ]]; then
    if [[ "${ROLE}" == "control-plane" ]]; then
      update_native_control_plane
      return
    fi
    update_native_node
    return
  fi
  update_docker_current
}

drop_local_database_if_possible() {
  parse_database_components
  if [[ "${DB_HOST}" != "127.0.0.1" && "${DB_HOST}" != "localhost" ]]; then
    return
  fi
  runuser -u postgres -- psql -p "${DB_PORT}" -c "select pg_terminate_backend(pid) from pg_stat_activity where datname='${DB_NAME}' and pid <> pg_backend_pid();" >/dev/null 2>&1 || true
  runuser -u postgres -- dropdb -p "${DB_PORT}" --if-exists "${DB_NAME}" >/dev/null 2>&1 || true
  runuser -u postgres -- psql -p "${DB_PORT}" -c "drop role if exists ${DB_USER};" >/dev/null 2>&1 || true
}

uninstall_native_current() {
  if [[ "${ROLE}" == "control-plane" ]]; then
    systemctl disable --now anneal-api anneal-worker anneal-caddy >/dev/null 2>&1 || true
    rm -f /etc/systemd/system/anneal-api.service /etc/systemd/system/anneal-worker.service /etc/systemd/system/anneal-caddy.service
    load_admin_summary
    [[ -n "${ANNEAL_DATABASE_URL:-}" ]] && drop_local_database_if_possible
  else
    systemctl disable --now anneal-node-agent anneal-xray anneal-singbox >/dev/null 2>&1 || true
    rm -f /etc/systemd/system/anneal-node-agent.service /etc/systemd/system/anneal-xray.service /etc/systemd/system/anneal-singbox.service
  fi
  systemctl daemon-reload
}

uninstall_docker_current() {
  compose_cmd -f "${ANNEAL_COMPOSE_FILE}" --env-file "${ANNEAL_STACK_ROOT}/.env" down -v || true
  rm -rf "${ANNEAL_STACK_ROOT}"
}

cleanup_installer_state() {
  rm -f "${PROFILE_HOOK_PATH}" "${CONTROL_UTILITY_PATH}" "${META_FILE}" "${SUMMARY_FILE}" "${ENV_FILE}"
}

uninstall_current_install() {
  if [[ "${DEPLOYMENT_MODE}" == "native" ]]; then
    uninstall_native_current
  else
    uninstall_docker_current
  fi
  rm -rf /opt/anneal
  cleanup_installer_state
}

show_admin_details() {
  load_admin_summary
  if [[ "${ROLE}" == "control-plane" ]]; then
    show_info "$(text "Данные администратора" "Administrator details")" "$(control_plane_install_message)"
    return
  fi
  show_info "$(text "Данные ноды" "Node server details")" "$(node_install_message)"
}

show_status_dialog() {
  show_info "$(text "Статус" "Status")" "$(status_summary)"
}

management_menu() {
  while true; do
    clear
    print_banner
    local title prompt status_key update_key restart_key details_key remove_key shell_key exit_key choice
    title="$(text "Anneal • Управление" "Anneal • Management")"
    prompt="$(text "Выбери действие для установленного сервера." "Choose an action for the installed server.")"
    status_key="$(text "Статус" "Status")"
    update_key="$(text "Обновить" "Update")"
    restart_key="$(text "Перезапуск" "Restart")"
    details_key="$(text "Данные" "Details")"
    remove_key="$(text "Удалить" "Remove")"
    shell_key="$(text "Shell" "Shell")"
    exit_key="$(text "Выход" "Exit")"
    choice="$(prompt_menu \
      "${title}" \
      "${prompt}" \
      "${status_key}" "$(text "Сервисы, health и версия" "Services, health and version")" \
      "${update_key}" "$(text "Скачать и применить свежий релиз" "Download and apply the latest release")" \
      "${restart_key}" "$(text "Перезапустить сервисы Anneal" "Restart Anneal services")" \
      "${details_key}" "$(text "Показать доступы и ссылки" "Show access data and links")" \
      "${remove_key}" "$(text "Полностью удалить Anneal" "Completely remove Anneal")" \
      "${shell_key}" "$(text "Выйти в обычную консоль" "Leave to the regular shell")" \
      "${exit_key}" "$(text "Закрыть меню" "Close the menu")")"
    case "${choice}" in
      "${status_key}")
        show_status_dialog
        ;;
      "${update_key}")
        update_current_install
        show_info "$(text "Обновление" "Update")" "$(text "Обновление завершено." "Update completed.")"
        ;;
      "${restart_key}")
        restart_current_install
        show_info "$(text "Перезапуск" "Restart")" "$(text "Перезапуск завершён." "Restart completed.")"
        ;;
      "${details_key}")
        show_admin_details
        ;;
      "${remove_key}")
        if prompt_confirm "$(text "Подтверждение удаления" "Uninstall confirmation")" "$(text "Удалить Anneal с этого сервера?" "Remove Anneal from this server?")"; then
          uninstall_current_install
          show_info "$(text "Удаление" "Uninstall")" "$(text "Anneal удалён с сервера." "Anneal was removed from the server.")"
          exit 0
        fi
        ;;
      "${shell_key}")
        return
        ;;
      "${exit_key}")
        if [[ "${LOGIN_SHELL}" -eq 1 ]]; then
          exit 0
        fi
        return
        ;;
    esac
  done
}

run_install() {
  configure_installation
  case "${ROLE}:${DEPLOYMENT_MODE}" in
    control-plane:native) install_native_control_plane ;;
    control-plane:docker) install_docker_control_plane ;;
    node:native) install_native_node ;;
    node:docker) install_docker_node ;;
    *)
      show_error "$(text "Комбинация роли и режима не поддерживается." "Unsupported role and mode combination.")"
      exit 1
      ;;
  esac
  install_control_utility
  install_profile_hook
  save_install_metadata
  save_admin_summary
  clear
  print_banner
  if [[ "${ROLE}" == "control-plane" ]]; then
    printf '%s\n' "$(control_plane_install_message)"
  else
    printf '%s\n' "$(node_install_message)"
  fi
}

ACTION="${ACTION:-install}"
ROLE="${ROLE:-}"
DEPLOYMENT_MODE="${DEPLOYMENT_MODE:-}"
LOGIN_SHELL=0
ANNEAL_INSTALLER_LANG="${ANNEAL_INSTALLER_LANG:-}"
ANNEAL_INSTALLER_UI="${ANNEAL_INSTALLER_UI:-auto}"
ANNEAL_GITHUB_REPOSITORY="${ANNEAL_GITHUB_REPOSITORY:-Anneal-Team/Anneal-Panel}"
ANNEAL_RELEASE_TAG="${ANNEAL_RELEASE_TAG:-rolling}"
ANNEAL_RELEASE_BASE_URL="${ANNEAL_RELEASE_BASE_URL:-https://github.com/${ANNEAL_GITHUB_REPOSITORY}/releases/download/${ANNEAL_RELEASE_TAG}}"
ANNEAL_TARGET_TRIPLE="${ANNEAL_TARGET_TRIPLE:-linux-amd64}"
ANNEAL_VERSION="${ANNEAL_VERSION:-0.1.0}"
ANNEAL_USER="${ANNEAL_USER:-anneal}"
ANNEAL_GROUP="${ANNEAL_GROUP:-anneal}"
ANNEAL_DOMAIN="${ANNEAL_DOMAIN:-}"
ANNEAL_PUBLIC_BASE_URL="${ANNEAL_PUBLIC_BASE_URL:-}"
ANNEAL_DB_NAME="${ANNEAL_DB_NAME:-anneal_$(generate_hex 4)}"
ANNEAL_DB_USER="${ANNEAL_DB_USER:-anneal_$(generate_hex 4)}"
ANNEAL_DB_PASSWORD="${ANNEAL_DB_PASSWORD:-$(generate_secret 18)}"
ANNEAL_DATABASE_HOST="${ANNEAL_DATABASE_HOST:-127.0.0.1}"
ANNEAL_DATABASE_PORT="${ANNEAL_DATABASE_PORT:-5432}"
ANNEAL_DATABASE_URL="${ANNEAL_DATABASE_URL:-postgres://${ANNEAL_DB_USER}:${ANNEAL_DB_PASSWORD}@${ANNEAL_DATABASE_HOST}:${ANNEAL_DATABASE_PORT}/${ANNEAL_DB_NAME}}"
ANNEAL_ACCESS_JWT_SECRET="${ANNEAL_ACCESS_JWT_SECRET:-$(generate_hex 32)}"
ANNEAL_PRE_AUTH_JWT_SECRET="${ANNEAL_PRE_AUTH_JWT_SECRET:-$(generate_hex 32)}"
ANNEAL_SUPERADMIN_EMAIL="${ANNEAL_SUPERADMIN_EMAIL:-}"
ANNEAL_SUPERADMIN_DISPLAY_NAME="${ANNEAL_SUPERADMIN_DISPLAY_NAME:-Superadmin}"
ANNEAL_SUPERADMIN_PASSWORD="${ANNEAL_SUPERADMIN_PASSWORD:-$(generate_secret 18)}"
ANNEAL_OTLP_ENDPOINT="${ANNEAL_OTLP_ENDPOINT:-}"
ANNEAL_AGENT_SERVER_URL="${ANNEAL_AGENT_SERVER_URL:-}"
ANNEAL_AGENT_NAME="${ANNEAL_AGENT_NAME:-}"
ANNEAL_AGENT_ENGINES="${ANNEAL_AGENT_ENGINES:-}"
ANNEAL_AGENT_PROTOCOLS_XRAY="${ANNEAL_AGENT_PROTOCOLS_XRAY:-}"
ANNEAL_AGENT_PROTOCOLS_SINGBOX="${ANNEAL_AGENT_PROTOCOLS_SINGBOX:-}"
ANNEAL_AGENT_XRAY_TOKEN="${ANNEAL_AGENT_XRAY_TOKEN:-}"
ANNEAL_AGENT_SINGBOX_TOKEN="${ANNEAL_AGENT_SINGBOX_TOKEN:-}"
ANNEAL_AGENT_ENROLLMENT_TOKENS="${ANNEAL_AGENT_ENROLLMENT_TOKENS:-}"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SELF_SOURCE="${BASH_SOURCE[0]}"
ENV_FILE="/etc/anneal/anneal.env"
META_FILE="/etc/anneal/install.meta"
SUMMARY_FILE="/etc/anneal/admin-summary.env"
CONTROL_UTILITY_PATH="/usr/local/bin/annealctl"
PROFILE_HOOK_PATH="/etc/profile.d/anneal-menu.sh"
DEPLOY_ASSET_ROOT=""
DEPLOY_TEMP_DIR=""

trap cleanup_temp_dir EXIT

setup_locale
parse_args "$@"
require_root

case "${ACTION}" in
  install)
    run_install
    ;;
  manage)
    load_install_state
    if use_tui; then
      ensure_whiptail
      management_menu
    else
      printf '%s\n' "$(status_summary)"
    fi
    ;;
  update)
    load_install_state
    update_current_install
    ;;
  restart)
    load_install_state
    restart_current_install
    ;;
  uninstall)
    load_install_state
    uninstall_current_install
    ;;
  status)
    load_install_state
    printf '%s\n' "$(status_summary)"
    ;;
  *)
    show_error "$(text "Неизвестное действие." "Unknown action.")"
    exit 1
    ;;
esac
