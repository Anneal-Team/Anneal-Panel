#!/usr/bin/env bash
set -euo pipefail

generate_hex() {
  openssl rand -hex "${1:-16}"
}

generate_secret() {
  openssl rand -base64 "${1:-24}" | tr -d '\n' | tr '/+=' '_-.'
}

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

ROLE="${1:-}"
if [[ "${ROLE}" == "--role" ]]; then
  ROLE="${2:-}"
fi

ANNEAL_VERSION="${ANNEAL_VERSION:-0.1.0}"
ANNEAL_INSTALLER_UI="${ANNEAL_INSTALLER_UI:-auto}"
ANNEAL_RELEASE_BASE_URL="${ANNEAL_RELEASE_BASE_URL:-https://example.com/anneal/releases/${ANNEAL_VERSION}}"
ANNEAL_XRAY_RELEASE_URL="${ANNEAL_XRAY_RELEASE_URL:-https://github.com/XTLS/Xray-core/releases/download/v26.2.6/Xray-linux-64.zip}"
ANNEAL_SINGBOX_RELEASE_URL="${ANNEAL_SINGBOX_RELEASE_URL:-https://github.com/hiddify/hiddify-core/releases/download/v4.0.4/hiddify-core-linux-amd64.tar.gz}"
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
ANNEAL_AGENT_NAME="${ANNEAL_AGENT_NAME:-node-$(generate_hex 3)}"
ANNEAL_AGENT_ENGINE="${ANNEAL_AGENT_ENGINE:-xray}"
ANNEAL_AGENT_ENGINES="${ANNEAL_AGENT_ENGINES:-}"
ANNEAL_AGENT_PROTOCOLS="${ANNEAL_AGENT_PROTOCOLS:-}"
ANNEAL_AGENT_PROTOCOLS_XRAY="${ANNEAL_AGENT_PROTOCOLS_XRAY:-vless_reality,vmess,trojan,shadowsocks_2022}"
ANNEAL_AGENT_PROTOCOLS_SINGBOX="${ANNEAL_AGENT_PROTOCOLS_SINGBOX:-vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2}"
ANNEAL_AGENT_ENROLLMENT_TOKEN="${ANNEAL_AGENT_ENROLLMENT_TOKEN:-}"
ANNEAL_AGENT_ENROLLMENT_TOKENS="${ANNEAL_AGENT_ENROLLMENT_TOKENS:-}"

finalize_node_defaults() {
  if [[ -z "${ANNEAL_AGENT_ENGINES}" ]]; then
    ANNEAL_AGENT_ENGINES="${ANNEAL_AGENT_ENGINE}"
  fi
  if [[ -z "${ANNEAL_AGENT_ENROLLMENT_TOKENS}" && -n "${ANNEAL_AGENT_ENROLLMENT_TOKEN}" ]]; then
    ANNEAL_AGENT_ENROLLMENT_TOKENS="${ANNEAL_AGENT_ENGINE}:${ANNEAL_AGENT_ENROLLMENT_TOKEN}"
  fi
}

require_root() {
  if [[ "${EUID}" -ne 0 ]]; then
    echo "run installer as root"
    exit 1
  fi
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

is_interactive_session() {
  [[ -t 0 && -t 1 ]]
}

use_tui() {
  case "${ANNEAL_INSTALLER_UI}" in
    plain|non-interactive) return 1 ;;
    tui) return 0 ;;
    auto) is_interactive_session ;;
    *) return 1 ;;
  esac
}

ensure_tui_dependencies() {
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
  whiptail --title "${title}" --inputbox "${prompt}" 12 88 "${default_value}" 3>&1 1>&2 2>&3
}

prompt_menu() {
  local title="$1"
  local prompt="$2"
  shift 2
  whiptail --title "${title}" --menu "${prompt}" 18 88 8 "$@" 3>&1 1>&2 2>&3
}

prompt_checklist() {
  local title="$1"
  local prompt="$2"
  shift 2
  local result
  result="$(whiptail --title "${title}" --checklist "${prompt}" 20 88 10 "$@" 3>&1 1>&2 2>&3)"
  echo "${result}" | tr -d '"' | xargs | tr ' ' ','
}

prompt_confirm() {
  local title="$1"
  local prompt="$2"
  whiptail --title "${title}" --yesno "${prompt}" 20 88
}

finalize_control_plane_defaults() {
  if [[ -z "${ANNEAL_PUBLIC_BASE_URL}" ]]; then
    ANNEAL_PUBLIC_BASE_URL="https://${ANNEAL_DOMAIN}"
  fi
  if [[ -z "${ANNEAL_SUPERADMIN_EMAIL}" ]]; then
    ANNEAL_SUPERADMIN_EMAIL="admin-$(generate_hex 3)@${ANNEAL_DOMAIN}"
  fi
}

control_plane_summary() {
  cat <<EOF
role: control-plane
domain: ${ANNEAL_DOMAIN}
panel_url: ${ANNEAL_PUBLIC_BASE_URL}
superadmin_email: ${ANNEAL_SUPERADMIN_EMAIL}
superadmin_password: ${ANNEAL_SUPERADMIN_PASSWORD}
database_url: ${ANNEAL_DATABASE_URL}
release_base_url: ${ANNEAL_RELEASE_BASE_URL}
otlp_endpoint: ${ANNEAL_OTLP_ENDPOINT:-disabled}
EOF
}

node_summary() {
  finalize_node_defaults
  cat <<EOF
role: node
server_url: ${ANNEAL_AGENT_SERVER_URL}
node_name: ${ANNEAL_AGENT_NAME}
runtimes: ${ANNEAL_AGENT_ENGINES}
xray_protocols: ${ANNEAL_AGENT_PROTOCOLS_XRAY}
singbox_protocols: ${ANNEAL_AGENT_PROTOCOLS_SINGBOX}
enrollment_tokens: ${ANNEAL_AGENT_ENROLLMENT_TOKENS}
release_base_url: ${ANNEAL_RELEASE_BASE_URL}
EOF
}

configure_control_plane_tui() {
  ANNEAL_DOMAIN="$(prompt_text "Anneal Control Plane" "РЈРєР°Р¶Рё РґРѕРјРµРЅ РїР°РЅРµР»Рё." "${ANNEAL_DOMAIN:-panel.example.com}")"
  finalize_control_plane_defaults
  ANNEAL_PUBLIC_BASE_URL="$(prompt_text "Anneal Control Plane" "РџСѓР±Р»РёС‡РЅС‹Р№ URL РїР°РЅРµР»Рё." "${ANNEAL_PUBLIC_BASE_URL}")"
  ANNEAL_SUPERADMIN_EMAIL="$(prompt_text "Anneal Control Plane" "Email bootstrap superadmin." "${ANNEAL_SUPERADMIN_EMAIL}")"
  ANNEAL_SUPERADMIN_DISPLAY_NAME="$(prompt_text "Anneal Control Plane" "РћС‚РѕР±СЂР°Р¶Р°РµРјРѕРµ РёРјСЏ superadmin." "${ANNEAL_SUPERADMIN_DISPLAY_NAME}")"
  ANNEAL_RELEASE_BASE_URL="$(prompt_text "Anneal Control Plane" "Р‘Р°Р·РѕРІС‹Р№ URL РіРѕС‚РѕРІС‹С… release-Р°СЂС‚РµС„Р°РєС‚РѕРІ." "${ANNEAL_RELEASE_BASE_URL}")"
  ANNEAL_OTLP_ENDPOINT="$(prompt_text "Anneal Control Plane" "OTLP endpoint. РњРѕР¶РЅРѕ РѕСЃС‚Р°РІРёС‚СЊ РїСѓСЃС‚С‹Рј." "${ANNEAL_OTLP_ENDPOINT}")"
  prompt_confirm "РџРѕРґС‚РІРµСЂР¶РґРµРЅРёРµ Control Plane" "$(control_plane_summary)"
}

configure_node_tui() {
  finalize_node_defaults
  ANNEAL_AGENT_SERVER_URL="$(prompt_text "Anneal Node" "URL control-plane API." "${ANNEAL_AGENT_SERVER_URL:-https://panel.example.com}")"
  ANNEAL_AGENT_NAME="$(prompt_text "Anneal Node" "Server name." "${ANNEAL_AGENT_NAME}")"
  ANNEAL_AGENT_ENGINES="xray,singbox"
  ANNEAL_AGENT_PROTOCOLS_XRAY="$(prompt_checklist "Anneal Node" "Xray protocols." \
    "vless_reality" "VLESS Reality" "ON" \
    "vmess" "VMess" "ON" \
    "trojan" "Trojan" "ON" \
    "shadowsocks_2022" "Shadowsocks 2022" "ON")"
  ANNEAL_AGENT_PROTOCOLS_SINGBOX="$(prompt_checklist "Anneal Node" "Sing-box protocols." \
    "vless_reality" "VLESS Reality" "ON" \
    "vmess" "VMess" "ON" \
    "trojan" "Trojan" "ON" \
    "shadowsocks_2022" "Shadowsocks 2022" "ON" \
    "tuic" "TUIC" "ON" \
    "hysteria2" "Hysteria2" "ON")"
  local xray_token
  local singbox_token
  xray_token="$(prompt_text "Anneal Node" "Xray enrollment token." "")"
  singbox_token="$(prompt_text "Anneal Node" "Sing-box enrollment token." "")"
  ANNEAL_AGENT_ENROLLMENT_TOKENS="xray:${xray_token},singbox:${singbox_token}"
  ANNEAL_RELEASE_BASE_URL="$(prompt_text "Anneal Node" "Release artifacts URL." "${ANNEAL_RELEASE_BASE_URL}")"
  prompt_confirm "Anneal Node" "$(node_summary)"
}

configure_installer_tui() {
  ensure_tui_dependencies
  if [[ -z "${ROLE:-}" ]]; then
    ROLE="$(prompt_menu "Anneal Installer" "Р’С‹Р±РµСЂРё СЂРµР¶РёРј СѓСЃС‚Р°РЅРѕРІРєРё." \
      "control-plane" "РџР°РЅРµР»СЊ, API, worker, Caddy, PostgreSQL" \
      "node" "Node-agent Рё VPN runtime")"
  fi

  case "${ROLE}" in
    control-plane) configure_control_plane_tui ;;
    node) configure_node_tui ;;
    *) echo "unknown role: ${ROLE}"; exit 1 ;;
  esac
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

install_packages() {
  export DEBIAN_FRONTEND=noninteractive
  setup_postgres_repository
  apt-get update
  apt-get install -y curl unzip tar ca-certificates gnupg lsb-release openssl jq whiptail iproute2 postgresql-17 postgresql-client-17 postgresql-contrib-17 caddy
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

download_artifact() {
  local artifact="$1"
  local destination="$2"
  curl --retry 5 --retry-all-errors --location --silent --show-error "${ANNEAL_RELEASE_BASE_URL}/${artifact}" -o "${destination}"
}

download_url() {
  local url="$1"
  local destination="$2"
  curl --retry 5 --retry-all-errors --location --silent --show-error "${url}" -o "${destination}"
}

extract_archive() {
  local archive="$1"
  local destination="$2"
  case "${archive}" in
    *.zip) unzip -oq "${archive}" -d "${destination}" ;;
    *.tar.gz) tar -xzf "${archive}" -C "${destination}" ;;
    *) echo "unsupported archive: ${archive}"; exit 1 ;;
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

install_runtime_cores() {
  rm -rf /tmp/anneal-runtime-xray /tmp/anneal-runtime-singbox
  mkdir -p /tmp/anneal-runtime-xray /tmp/anneal-runtime-singbox
  download_url "${ANNEAL_XRAY_RELEASE_URL}" /tmp/xray-runtime.zip
  download_url "${ANNEAL_SINGBOX_RELEASE_URL}" /tmp/hiddify-core.tar.gz
  extract_archive /tmp/xray-runtime.zip /tmp/anneal-runtime-xray
  extract_archive /tmp/hiddify-core.tar.gz /tmp/anneal-runtime-singbox
  install -m 0755 /tmp/anneal-runtime-xray/xray /opt/anneal/bin/xray
  install -m 0755 "$(find /tmp/anneal-runtime-singbox -type f -name 'hiddify-core' | head -n 1)" /opt/anneal/bin/hiddify-core
}

write_control_plane_env() {
  cat >/etc/anneal/anneal.env <<EOF
ANNEAL_BIND_ADDRESS=127.0.0.1:8080
ANNEAL_DATABASE_URL=${ANNEAL_DATABASE_URL}
ANNEAL_MIGRATIONS_DIR=/opt/anneal/migrations
ANNEAL_ACCESS_JWT_SECRET=${ANNEAL_ACCESS_JWT_SECRET}
ANNEAL_PRE_AUTH_JWT_SECRET=${ANNEAL_PRE_AUTH_JWT_SECRET}
ANNEAL_PUBLIC_BASE_URL=${ANNEAL_PUBLIC_BASE_URL}
ANNEAL_CADDY_DOMAIN=${ANNEAL_DOMAIN}
ANNEAL_OTLP_ENDPOINT=${ANNEAL_OTLP_ENDPOINT}
EOF
}

write_node_env() {
  finalize_node_defaults
  cat >/etc/anneal/anneal.env <<EOF
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
ANNEAL_AGENT_XRAY_SERVICE=anneal-xray.service
ANNEAL_AGENT_SINGBOX_SERVICE=anneal-singbox.service
EOF
}

print_control_plane_summary() {
  cat <<EOF
anneal control-plane installed
panel_url: ${ANNEAL_PUBLIC_BASE_URL}
superadmin_email: ${ANNEAL_SUPERADMIN_EMAIL}
superadmin_password: ${ANNEAL_SUPERADMIN_PASSWORD}
database_url: ${ANNEAL_DATABASE_URL}
env_file: /etc/anneal/anneal.env
api_service: anneal-api.service
worker_service: anneal-worker.service
caddy_service: anneal-caddy.service
EOF
}

print_node_summary() {
  finalize_node_defaults
  cat <<EOF
anneal node installed
server_url: ${ANNEAL_AGENT_SERVER_URL}
node_name: ${ANNEAL_AGENT_NAME}
runtimes: ${ANNEAL_AGENT_ENGINES}
xray_protocols: ${ANNEAL_AGENT_PROTOCOLS_XRAY}
singbox_protocols: ${ANNEAL_AGENT_PROTOCOLS_SINGBOX}
env_file: /etc/anneal/anneal.env
agent_service: anneal-node-agent.service
runtime_services: anneal-xray.service, anneal-singbox.service
EOF
}

install_control_plane() {
  finalize_control_plane_defaults
  if [[ -z "${ANNEAL_DOMAIN}" ]]; then
    echo "ANNEAL_DOMAIN is required for control-plane install"
    exit 1
  fi

  install_packages
  ensure_user
  ensure_postgres

  download_artifact "api-linux-amd64.tar.gz" /tmp/api.tar.gz
  download_artifact "worker-linux-amd64.tar.gz" /tmp/worker.tar.gz
  download_artifact "web.tar.gz" /tmp/web.tar.gz
  download_artifact "migrations.tar.gz" /tmp/migrations.tar.gz

  install_archive_contents /tmp/api.tar.gz /opt/anneal/bin
  install_archive_contents /tmp/worker.tar.gz /opt/anneal/bin
  install_archive_contents /tmp/web.tar.gz /opt/anneal/web
  install_archive_contents /tmp/migrations.tar.gz /opt/anneal/migrations

  write_control_plane_env

  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  sed "s/{{DOMAIN}}/${ANNEAL_DOMAIN}/g" "${SCRIPT_DIR}/../deploy/caddy/Caddyfile.tpl" >/etc/anneal/Caddyfile
  install -m 0644 "${SCRIPT_DIR}/../deploy/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${SCRIPT_DIR}/../deploy/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${SCRIPT_DIR}/../deploy/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service

  systemctl daemon-reload
  systemctl enable --now postgresql anneal-api anneal-worker anneal-caddy

  until curl --silent --show-error --fail http://127.0.0.1:8080/api/v1/health >/dev/null; do
    sleep 2
  done

  jq -n \
    --arg email "${ANNEAL_SUPERADMIN_EMAIL}" \
    --arg display_name "${ANNEAL_SUPERADMIN_DISPLAY_NAME}" \
    --arg password "${ANNEAL_SUPERADMIN_PASSWORD}" \
    '{email:$email, display_name:$display_name, password:$password}' |
    curl --silent --show-error --fail http://127.0.0.1:8080/api/v1/bootstrap \
      -H 'content-type: application/json' \
      --data-binary @- >/dev/null

  print_control_plane_summary
}

install_node() {
  finalize_node_defaults
  install_packages
  ensure_user
  if [[ -z "${ANNEAL_AGENT_SERVER_URL}" || -z "${ANNEAL_AGENT_NAME}" || -z "${ANNEAL_AGENT_ENROLLMENT_TOKENS}" ]]; then
    echo "set ANNEAL_AGENT_SERVER_URL, ANNEAL_AGENT_NAME and ANNEAL_AGENT_ENROLLMENT_TOKENS for node install"
    exit 1
  fi

  download_artifact "node-agent-linux-amd64.tar.gz" /tmp/node-agent.tar.gz
  install_archive_contents /tmp/node-agent.tar.gz /opt/anneal/bin
  install_runtime_cores
  install_runtime_defaults
  write_node_env

  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  install -m 0644 "${SCRIPT_DIR}/../deploy/systemd/anneal-node-agent.service" /etc/systemd/system/anneal-node-agent.service
  install -m 0644 "${SCRIPT_DIR}/../deploy/systemd/anneal-xray.service" /etc/systemd/system/anneal-xray.service
  install -m 0644 "${SCRIPT_DIR}/../deploy/systemd/anneal-singbox.service" /etc/systemd/system/anneal-singbox.service

  systemctl daemon-reload
  systemctl enable --now anneal-xray anneal-singbox
  systemctl enable --now anneal-node-agent

  print_node_summary
}

require_root
if use_tui; then
  configure_installer_tui
fi

if [[ -z "${ROLE:-}" ]]; then
  echo "usage: install.sh --role control-plane|node"
  exit 1
fi

case "${ROLE}" in
  control-plane) install_control_plane ;;
  node) install_node ;;
  *) echo "unknown role: ${ROLE}"; exit 1 ;;
esac

