#!/usr/bin/env bash
# shellcheck disable=SC1091,SC1111,SC1112
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

detect_self_source() {
  if [[ -n "${BASH_SOURCE[0]:-}" ]]; then
    printf '%s' "${BASH_SOURCE[0]}"
    return
  fi
  if [[ -n "${0:-}" && "${0}" != "bash" && "${0}" != "-bash" ]]; then
    printf '%s' "${0}"
    return
  fi
  printf '%s' "/dev/stdin"
}

detect_script_dir() {
  local source_path="$1"
  case "${source_path}" in
    /dev/stdin|/dev/fd/*)
      pwd
      ;;
    *)
      cd -- "$(dirname -- "${source_path}")" && pwd
      ;;
  esac
}

normalize_release_version() {
  printf '%s' "${1#v}"
}

github_api_get() {
  local url="$1"
  curl \
    --fail \
    --retry 5 \
    --retry-all-errors \
    --location \
    --silent \
    --show-error \
    --user-agent "Anneal-Installer/1.0" \
    -H "Accept: application/vnd.github+json" \
    "${url}"
}

resolve_latest_release_tag() {
  local response
  local release_tag
  response="$(github_api_get "https://api.github.com/repos/${ANNEAL_GITHUB_REPOSITORY}/releases/latest")" || {
    show_error "$(text "Не удалось получить latest release из GitHub. Сначала опубликуй semver release." "Failed to resolve the latest GitHub release. Publish a semver release first.")"
    exit 1
  }
  release_tag="$(
    printf '%s' "${response}" |
      tr -d '\r\n' |
      sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
      head -n 1
  )"
  if [[ -z "${release_tag}" ]]; then
    show_error "$(text "GitHub не вернул tag_name для latest release." "GitHub did not return tag_name for the latest release.")"
    exit 1
  fi
  printf '%s' "${release_tag}"
}

ensure_release_metadata() {
  if [[ -z "${ANNEAL_RELEASE_TAG}" ]]; then
    ANNEAL_RELEASE_TAG="$(resolve_latest_release_tag)"
  fi
  if [[ -z "${ANNEAL_VERSION}" ]]; then
    ANNEAL_VERSION="$(normalize_release_version "${ANNEAL_RELEASE_TAG}")"
  fi
  if [[ -z "${ANNEAL_RELEASE_BASE_URL}" ]]; then
    ANNEAL_RELEASE_BASE_URL="https://github.com/${ANNEAL_GITHUB_REPOSITORY}/releases/download/${ANNEAL_RELEASE_TAG}"
  fi
}

use_requested_release_metadata() {
  ANNEAL_RELEASE_TAG="${REQUESTED_RELEASE_TAG}"
  ANNEAL_VERSION="${REQUESTED_RELEASE_VERSION}"
  ANNEAL_RELEASE_BASE_URL="${REQUESTED_RELEASE_BASE_URL}"
}

reset_release_metadata_to_latest() {
  if [[ -n "${REQUESTED_RELEASE_TAG}" || -n "${REQUESTED_RELEASE_VERSION}" || -n "${REQUESTED_RELEASE_BASE_URL}" ]]; then
    use_requested_release_metadata
    return
  fi
  ANNEAL_RELEASE_TAG=""
  ANNEAL_VERSION=""
  ANNEAL_RELEASE_BASE_URL=""
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
  if [[ "${LC_CTYPE:-}" != *UTF-8* && "${LC_CTYPE:-}" != *utf8* ]]; then
    export LC_CTYPE="${LANG}"
  fi
  if [[ "${LC_ALL:-}" != *UTF-8* && "${LC_ALL:-}" != *utf8* ]]; then
    export LC_ALL="${LANG}"
  fi
  export LANGUAGE="${LANGUAGE:-${ANNEAL_INSTALLER_LANG:-en}}"
  export NCURSES_NO_UTF8_ACS="${NCURSES_NO_UTF8_ACS:-1}"
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

has_tui_terminal() {
  [[ -t 1 ]] || return 1
  [[ -r /dev/tty ]]
}

use_tui() {
  if [[ "${ANNEAL_INSTALLER_UI}" == "tui" ]]; then
    has_tui_terminal
    return
  fi
  [[ "${ANNEAL_INSTALLER_UI}" == "plain" ]] && return 1
  is_interactive_session || has_tui_terminal
}

run_whiptail() {
  if has_tui_terminal; then
    whiptail "$@" </dev/tty
    return
  fi
  whiptail "$@"
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
  local value=""
  if use_tui && has_tui_terminal && [[ "${ANNEAL_INSTALLER_TEXT_UI:-tty}" != "dialog" ]]; then
    printf '\n%s\n' "$(installer_backtitle)" > /dev/tty
    printf '%s\n\n' "${title}" > /dev/tty
    printf '%s\n' "${prompt}" > /dev/tty
    printf '%s\n\n' "$(text "Вставь значение и нажми Enter." "Paste the value and press Enter.")" > /dev/tty
    if [[ -n "${default_value}" ]]; then
      read -r -e -i "${default_value}" -p "> " value < /dev/tty > /dev/tty
    else
      read -r -e -p "> " value < /dev/tty > /dev/tty
    fi
    printf '%s\n' "${value}"
    return
  fi
  run_whiptail \
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
  run_whiptail \
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
  result="$(run_whiptail \
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
  run_whiptail \
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
    run_whiptail \
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
    run_whiptail \
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

role_includes_control_plane() {
  case "${ROLE}" in
    control-plane|all-in-one) return 0 ;;
  esac
  return 1
}

role_includes_node() {
  case "${ROLE}" in
    node|all-in-one) return 0 ;;
  esac
  return 1
}

normalize_domain_input() {
  local value="$1"
  value="${value#http://}"
  value="${value#https://}"
  value="${value%%/*}"
  printf '%s' "${value}"
}

normalize_panel_path() {
  local value="$1"
  value="${value#/}"
  value="${value%/}"
  printf '%s' "${value}"
}

panel_path_prefix() {
  if [[ -z "${ANNEAL_PANEL_PATH}" ]]; then
    return
  fi
  printf '/%s' "${ANNEAL_PANEL_PATH}"
}

panel_base_href() {
  local prefix
  prefix="$(panel_path_prefix)"
  if [[ -z "${prefix}" ]]; then
    printf '%s' "/"
    return
  fi
  printf '%s/' "${prefix}"
}

generate_panel_path() {
  generate_hex 24
}

hydrate_control_plane_access_from_public_url() {
  local without_scheme
  local host
  local path
  if [[ -z "${ANNEAL_PUBLIC_BASE_URL}" ]]; then
    return
  fi
  without_scheme="${ANNEAL_PUBLIC_BASE_URL#http://}"
  without_scheme="${without_scheme#https://}"
  host="${without_scheme%%/*}"
  path=""
  if [[ "${without_scheme}" == */* ]]; then
    path="/${without_scheme#*/}"
  fi
  if [[ -z "${ANNEAL_DOMAIN}" ]]; then
    ANNEAL_DOMAIN="${host}"
  fi
  if [[ -z "${ANNEAL_PANEL_PATH}" ]]; then
    ANNEAL_PANEL_PATH="$(normalize_panel_path "${path}")"
  fi
}

finalize_control_plane_defaults() {
  hydrate_control_plane_access_from_public_url
  if [[ -z "${ANNEAL_DOMAIN}" ]]; then
    ANNEAL_DOMAIN="panel.example.com"
  else
    ANNEAL_DOMAIN="$(normalize_domain_input "${ANNEAL_DOMAIN}")"
  fi
  if [[ -z "${ANNEAL_PANEL_PATH}" ]]; then
    ANNEAL_PANEL_PATH="$(generate_panel_path)"
  fi
  if [[ -z "${ANNEAL_PUBLIC_BASE_URL}" ]]; then
    ANNEAL_PUBLIC_BASE_URL="https://${ANNEAL_DOMAIN}$(panel_path_prefix)"
  fi
  if [[ -z "${ANNEAL_SUPERADMIN_EMAIL}" ]]; then
    ANNEAL_SUPERADMIN_EMAIL="admin-$(generate_hex 3)@${ANNEAL_DOMAIN}"
  fi
}

finalize_single_server_defaults() {
  finalize_control_plane_defaults
  finalize_node_defaults
  if [[ -z "${ANNEAL_RESELLER_TENANT_NAME}" ]]; then
    ANNEAL_RESELLER_TENANT_NAME="Default Tenant"
  fi
  if [[ -z "${ANNEAL_RESELLER_DISPLAY_NAME}" ]]; then
    ANNEAL_RESELLER_DISPLAY_NAME="Tenant Admin"
  fi
  if [[ -z "${ANNEAL_RESELLER_EMAIL}" ]]; then
    ANNEAL_RESELLER_EMAIL="tenant-$(generate_hex 3)@${ANNEAL_DOMAIN}"
  fi
  if [[ -z "${ANNEAL_RESELLER_PASSWORD}" ]]; then
    ANNEAL_RESELLER_PASSWORD="$(generate_secret 18)"
  fi
  if [[ -z "${ANNEAL_NODE_GROUP_NAME}" ]]; then
    ANNEAL_NODE_GROUP_NAME="edge-$(generate_hex 3)"
  fi
  ANNEAL_AGENT_NAME="${ANNEAL_NODE_GROUP_NAME}"
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
  return 0
}

validate_node_bootstrap() {
  if [[ -z "${ANNEAL_AGENT_BOOTSTRAP_TOKEN}" ]]; then
    show_error "$(text "Для node server нужен bootstrap token панели." "Node server requires a bootstrap token from the panel.")"
    exit 1
  fi
  if [[ "${ANNEAL_AGENT_SERVER_URL}" != https://* ]]; then
    show_error "$(text "URL control-plane должен начинаться с https://." "Control-plane URL must start with https://.")"
    exit 1
  fi
}

base32_secret_to_hex() {
  local secret="$1"
  local normalized
  local remainder
  normalized="$(printf '%s' "${secret}" | tr -d '[:space:]=' | tr '[:lower:]' '[:upper:]')"
  remainder=$(( ${#normalized} % 8 ))
  if [[ "${remainder}" -ne 0 ]]; then
    normalized="${normalized}$(printf '=%.0s' $(seq 1 $((8 - remainder))))"
  fi
  if command_exists base32; then
    printf '%s' "${normalized}" | base32 --decode | od -An -vtx1 | tr -d ' \n'
    return
  fi
  if command_exists basenc; then
    printf '%s' "${normalized}" | basenc --base32 -d | od -An -vtx1 | tr -d ' \n'
    return
  fi
  show_error "$(text "Не найден base32/basenc для генерации TOTP." "base32/basenc was not found for TOTP generation.")"
  exit 1
}

hex_to_binary() {
  local hex="$1"
  local escaped=""
  local index
  for ((index = 0; index < ${#hex}; index += 2)); do
    escaped="${escaped}\\x${hex:index:2}"
  done
  printf '%b' "${escaped}"
}

generate_totp_code() {
  local secret="$1"
  local key_hex
  local counter_hex
  local digest_hex
  local offset
  local code_hex
  local code
  key_hex="$(base32_secret_to_hex "${secret}")"
  counter_hex="$(printf '%016x' "$(( $(date +%s) / 30 ))")"
  digest_hex="$(
    hex_to_binary "${counter_hex}" |
      openssl dgst -sha1 -mac HMAC -macopt "hexkey:${key_hex}" -binary |
      od -An -vtx1 | tr -d ' \n'
  )"
  offset=$((16#${digest_hex:${#digest_hex}-1:1}))
  code_hex="${digest_hex:$((offset * 2)):8}"
  code=$(( (16#${code_hex} & 0x7fffffff) % 1000000 ))
  printf '%06d' "${code}"
}

local_api_base_url() {
  printf '%s' "http://127.0.0.1:8080/api/v1"
}

api_post_local_json() {
  local path="$1"
  local payload="$2"
  curl -fsS "$(local_api_base_url)${path}" -H 'content-type: application/json' --data "${payload}"
}

api_post_local_auth_json() {
  local access_token="$1"
  local path="$2"
  local payload="$3"
  curl -fsS "$(local_api_base_url)${path}" -H 'content-type: application/json' -H "authorization: Bearer ${access_token}" --data "${payload}"
}

login_local_superadmin_access_token() {
  local login_response
  local status
  local pre_auth_token
  local totp_setup
  local totp_secret
  local totp_code
  local verify_response
  login_response="$(api_post_local_json "/auth/login" "$(jq -nc --arg email "${ANNEAL_SUPERADMIN_EMAIL}" --arg password "${ANNEAL_SUPERADMIN_PASSWORD}" '{email:$email, password:$password}')" )"
  status="$(printf '%s' "${login_response}" | jq -r '.status')"
  if [[ "${status}" == "authenticated" ]]; then
    printf '%s' "${login_response}" | jq -r '.tokens.access_token'
    return
  fi
  pre_auth_token="$(printf '%s' "${login_response}" | jq -r '.pre_auth_token')"
  totp_setup="$(curl -fsS "$(local_api_base_url)/auth/totp/setup" -X POST -H "authorization: Bearer ${pre_auth_token}")"
  totp_secret="$(printf '%s' "${totp_setup}" | jq -r '.secret')"
  totp_code="$(generate_totp_code "${totp_secret}")"
  verify_response="$(curl -fsS "$(local_api_base_url)/auth/totp/verify" -H 'content-type: application/json' -H "authorization: Bearer ${pre_auth_token}" --data "$(jq -nc --arg code "${totp_code}" '{code:$code}')" )"
  printf '%s' "${verify_response}" | jq -r '.access_token'
}

wait_for_public_api() {
  local health_url
  health_url="${ANNEAL_PUBLIC_BASE_URL}/api/v1/health"
  for _ in $(seq 1 120); do
    if curl --silent --show-error --fail --resolve "${ANNEAL_DOMAIN}:443:127.0.0.1" "${health_url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  show_error "$(text "Публичный HTTPS панели не поднялся вовремя." "Public panel HTTPS did not become ready in time.")"
  exit 1
}

bootstrap_single_server_node() {
  local access_token
  local reseller_response
  local tenant_id
  local node_response
  local node_id
  local bootstrap_response
  local engines_json
  finalize_single_server_defaults
  access_token="$(login_local_superadmin_access_token)"
  reseller_response="$(api_post_local_auth_json "${access_token}" "/resellers" "$(jq -nc --arg tenant_name "${ANNEAL_RESELLER_TENANT_NAME}" --arg email "${ANNEAL_RESELLER_EMAIL}" --arg display_name "${ANNEAL_RESELLER_DISPLAY_NAME}" --arg password "${ANNEAL_RESELLER_PASSWORD}" '{tenant_name:$tenant_name, email:$email, display_name:$display_name, password:$password}')" )"
  tenant_id="$(printf '%s' "${reseller_response}" | jq -r '.tenant_id')"
  node_response="$(api_post_local_auth_json "${access_token}" "/nodes" "$(jq -nc --arg tenant_id "${tenant_id}" --arg name "${ANNEAL_NODE_GROUP_NAME}" '{tenant_id:$tenant_id, name:$name}')" )"
  node_id="$(printf '%s' "${node_response}" | jq -r '.id')"
  engines_json="$(jq -nc --arg engines "${ANNEAL_AGENT_ENGINES}" '$engines | split(",") | map(select(length > 0))')"
  bootstrap_response="$(api_post_local_auth_json "${access_token}" "/nodes/${node_id}/bootstrap-sessions" "$(jq -nc --arg tenant_id "${tenant_id}" --argjson engines "${engines_json}" '{tenant_id:$tenant_id, engines:$engines}')" )"
  ANNEAL_AGENT_NAME="${ANNEAL_NODE_GROUP_NAME}"
  ANNEAL_AGENT_SERVER_URL="${ANNEAL_PUBLIC_BASE_URL}"
  ANNEAL_AGENT_BOOTSTRAP_TOKEN="$(printf '%s' "${bootstrap_response}" | jq -r '.bootstrap_token')"
}

control_plane_summary() {
  ensure_release_metadata
  cat <<EOF
$(text "Роль" "Role"): control-plane
$(text "Режим" "Mode"): ${DEPLOYMENT_MODE}
$(text "Домен" "Domain"): ${ANNEAL_DOMAIN}
panel_url: ${ANNEAL_PUBLIC_BASE_URL}
$(text "Email суперадмина" "Superadmin email"): ${ANNEAL_SUPERADMIN_EMAIL}
$(text "Релиз" "Release"): ${ANNEAL_RELEASE_TAG}
$(text "Версия" "Version"): ${ANNEAL_VERSION}
EOF
}

node_summary() {
  ensure_release_metadata
  cat <<EOF
$(text "Роль" "Role"): node-server
$(text "Режим" "Mode"): ${DEPLOYMENT_MODE}
$(text "Control Plane URL" "Control Plane URL"): ${ANNEAL_AGENT_SERVER_URL}
$(text "Имя ноды" "Node name"): ${ANNEAL_AGENT_NAME}
$(text "Runtime-пакеты" "Runtime packages"): ${ANNEAL_AGENT_ENGINES}
$(text "Релиз" "Release"): ${ANNEAL_RELEASE_TAG}
$(text "Версия" "Version"): ${ANNEAL_VERSION}
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
  ANNEAL_AGENT_BOOTSTRAP_TOKEN="$(prompt_text \
    "$(text "Anneal • Bootstrap Token" "Anneal • Bootstrap Token")" \
    "$(text "Вставь bootstrap token панели для этой ноды." "Enter the panel bootstrap token for this node server.")" \
    "${ANNEAL_AGENT_BOOTSTRAP_TOKEN}")"
  validate_node_bootstrap
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
      validate_node_bootstrap
    fi
  fi
}

control_utility_source_url() {
  ensure_release_metadata
  printf 'https://raw.githubusercontent.com/%s/%s/scripts/install.sh' "${ANNEAL_GITHUB_REPOSITORY}" "${ANNEAL_RELEASE_TAG}"
}

choose_role() {
  local choice
  if [[ -n "${ROLE}" ]]; then
    return
  fi
  choice="$(prompt_menu \
    "Anneal • Role" \
    "$(text "Выбери, что устанавливается на этот сервер." "Choose what will be installed on this server.")" \
    "All-in-one" "$(text "Панель и runtime-ядра на одном сервере" "Panel and runtime engines on one server")" \
    "Panel" "$(text "Только control-plane: UI, API, worker и база" "Control-plane only: UI, API, worker and database")" \
    "Node" "$(text "Отдельная node с runtime-ядрами" "Separate node with runtime engines")")"
  case "${choice}" in
    All-in-one) ROLE="all-in-one" ;;
    Panel) ROLE="control-plane" ;;
    Node) ROLE="node" ;;
  esac
}

control_plane_summary() {
  ensure_release_metadata
  cat <<EOF
Role: control-plane
Mode: ${DEPLOYMENT_MODE}
Domain: ${ANNEAL_DOMAIN}
panel_url: ${ANNEAL_PUBLIC_BASE_URL}
panel_path: $(panel_path_prefix)
superadmin_email: ${ANNEAL_SUPERADMIN_EMAIL}
release_tag: ${ANNEAL_RELEASE_TAG}
version: ${ANNEAL_VERSION}
EOF
}

all_in_one_summary() {
  ensure_release_metadata
  cat <<EOF
Role: all-in-one
Mode: ${DEPLOYMENT_MODE}
Domain: ${ANNEAL_DOMAIN}
panel_url: ${ANNEAL_PUBLIC_BASE_URL}
panel_path: $(panel_path_prefix)
superadmin_email: ${ANNEAL_SUPERADMIN_EMAIL}
tenant_name: ${ANNEAL_RESELLER_TENANT_NAME}
tenant_admin_email: ${ANNEAL_RESELLER_EMAIL}
node_name: ${ANNEAL_NODE_GROUP_NAME}
runtimes: ${ANNEAL_AGENT_ENGINES}
release_tag: ${ANNEAL_RELEASE_TAG}
version: ${ANNEAL_VERSION}
EOF
}

configure_control_plane_tui() {
  finalize_control_plane_defaults
  ANNEAL_DOMAIN="$(prompt_text \
    "Anneal • Control Plane" \
    "$(text "Укажи домен или ссылку панели. Приватный путь будет сгенерирован автоматически." "Enter the panel domain or URL. The private path will be generated automatically.")" \
    "${ANNEAL_DOMAIN}")"
  finalize_control_plane_defaults
  ANNEAL_SUPERADMIN_EMAIL="$(prompt_text \
    "Anneal • Control Plane" \
    "$(text "Email bootstrap-суперадмина." "Enter the bootstrap superadmin email.")" \
    "${ANNEAL_SUPERADMIN_EMAIL}")"
  ANNEAL_SUPERADMIN_DISPLAY_NAME="$(prompt_text \
    "Anneal • Control Plane" \
    "$(text "Отображаемое имя суперадмина." "Enter the superadmin display name.")" \
    "${ANNEAL_SUPERADMIN_DISPLAY_NAME}")"
  if ! prompt_confirm "$(text "Подтверждение" "Confirmation")" "$(control_plane_summary)"; then
    exit 1
  fi
}

configure_all_in_one_tui() {
  finalize_single_server_defaults
  ANNEAL_DOMAIN="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Укажи домен или ссылку панели. Приватный путь будет сгенерирован автоматически." "Enter the panel domain or URL. The private path will be generated automatically.")" \
    "${ANNEAL_DOMAIN}")"
  finalize_single_server_defaults
  ANNEAL_SUPERADMIN_EMAIL="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Email bootstrap-суперадмина." "Enter the bootstrap superadmin email.")" \
    "${ANNEAL_SUPERADMIN_EMAIL}")"
  ANNEAL_SUPERADMIN_DISPLAY_NAME="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Отображаемое имя суперадмина." "Enter the superadmin display name.")" \
    "${ANNEAL_SUPERADMIN_DISPLAY_NAME}")"
  ANNEAL_RESELLER_TENANT_NAME="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Название tenant по умолчанию." "Enter the default tenant name.")" \
    "${ANNEAL_RESELLER_TENANT_NAME}")"
  ANNEAL_NODE_GROUP_NAME="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Имя локальной ноды." "Enter the local node name.")" \
    "${ANNEAL_NODE_GROUP_NAME}")"
  ANNEAL_AGENT_ENGINES="$(prompt_checklist \
    "Anneal • Runtime packages" \
    "$(text "Выбери runtime-пакеты для локальной ноды." "Choose runtime packages for the local node.")" \
    "xray" "Xray" "ON" \
    "singbox" "Sing-box" "ON")"
  if ! prompt_confirm "$(text "Подтверждение" "Confirmation")" "$(all_in_one_summary)"; then
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
      all-in-one) configure_all_in_one_tui ;;
      control-plane) configure_control_plane_tui ;;
      node) configure_node_tui ;;
      *)
        show_error "$(text "Неизвестная роль." "Unknown role.")"
        exit 1
        ;;
    esac
  else
    [[ -n "${ROLE}" ]] || {
      show_error "$(text "Передай --role all-in-one|control-plane|node." "Pass --role all-in-one|control-plane|node.")"
      exit 1
    }
    [[ -n "${DEPLOYMENT_MODE}" ]] || {
      show_error "$(text "Передай --mode native|docker." "Pass --mode native|docker.")"
      exit 1
    }
    case "${ROLE}" in
      all-in-one) finalize_single_server_defaults ;;
      control-plane) finalize_control_plane_defaults ;;
      node)
        finalize_node_defaults
        validate_node_bootstrap
        ;;
      *)
        show_error "$(text "Неизвестная роль." "Unknown role.")"
        exit 1
        ;;
    esac
  fi
}

release_bundle_asset() {
  printf 'anneal-%s-%s.tar.gz' "${ANNEAL_VERSION}" "${ANNEAL_TARGET_TRIPLE}"
}

download_release_asset() {
  local asset="$1"
  local destination="$2"
  curl --retry 5 --retry-all-errors --location --silent --show-error \
    "${ANNEAL_RELEASE_BASE_URL}/${asset}" \
    -o "${destination}"
}

validate_tar_gz() {
  local archive="$1"
  tar -tzf "${archive}" >/dev/null 2>&1
}

resolve_bundle_root() {
  local extracted_root="$1"
  local candidate
  if [[ -f "${extracted_root}/release-manifest.json" ]]; then
    printf '%s' "${extracted_root}"
    return
  fi
  shopt -s nullglob
  local entries=("${extracted_root}"/*)
  shopt -u nullglob
  if [[ "${#entries[@]}" -eq 1 && -d "${entries[0]}" && -f "${entries[0]}/release-manifest.json" ]]; then
    printf '%s' "${entries[0]}"
    return
  fi
  for candidate in "${entries[@]}"; do
    if [[ -d "${candidate}" && -f "${candidate}/release-manifest.json" ]]; then
      printf '%s' "${candidate}"
      return
    fi
  done
  show_error "$(text "Не удалось определить корень release bundle." "Failed to resolve the release bundle root.")"
  exit 1
}

load_release_bundle_metadata() {
  local bundle_root="$1"
  local version
  version="$(
    sed -n 's/^[[:space:]]*"version":[[:space:]]*"\([^"]*\)".*/\1/p' "${bundle_root}/release-manifest.json" |
      head -n 1
  )"
  if [[ -z "${version}" ]]; then
    show_error "$(text "Не удалось прочитать версию release bundle." "Failed to read the release bundle version.")"
    exit 1
  fi
  ANNEAL_VERSION="${version}"
}

prepare_deploy_assets() {
  local bundle_asset
  local bundle_archive
  ensure_release_metadata
  if [[ -n "${RELEASE_BUNDLE_ROOT:-}" && -d "${RELEASE_BUNDLE_ROOT}" ]]; then
    DEPLOY_ASSET_ROOT="${RELEASE_BUNDLE_ROOT}/deploy"
    return
  fi
  DEPLOY_TEMP_DIR="$(mktemp -d)"
  bundle_archive="${DEPLOY_TEMP_DIR}/release-bundle.tar.gz"
  bundle_asset="$(release_bundle_asset)"
  if [[ -f "${SCRIPT_DIR}/../dist/${bundle_asset}" ]]; then
    cp "${SCRIPT_DIR}/../dist/${bundle_asset}" "${bundle_archive}"
  else
    download_release_asset "${bundle_asset}" "${bundle_archive}" || {
      show_error "$(text "Не удалось скачать release bundle ${bundle_asset} для тега ${ANNEAL_RELEASE_TAG}." "Failed to download release bundle ${bundle_asset} for tag ${ANNEAL_RELEASE_TAG}.")"
      exit 1
    }
  fi
  if ! validate_tar_gz "${bundle_archive}"; then
    show_error "$(text "Релиз ${ANNEAL_RELEASE_TAG} не содержит bundle ${bundle_asset}. Опубликуй semver release и попробуй снова." "Release ${ANNEAL_RELEASE_TAG} does not contain bundle ${bundle_asset}. Publish the semver release and try again.")"
    exit 1
  fi
  tar -xzf "${bundle_archive}" -C "${DEPLOY_TEMP_DIR}"
  RELEASE_BUNDLE_ROOT="$(resolve_bundle_root "${DEPLOY_TEMP_DIR}")"
  load_release_bundle_metadata "${RELEASE_BUNDLE_ROOT}"
  DEPLOY_ASSET_ROOT="${RELEASE_BUNDLE_ROOT}/deploy"
}

sync_directory_contents() {
  local source="$1"
  local destination="$2"
  install -d "${destination}"
  rm -rf "${destination:?}"/*
  cp -a "${source}"/. "${destination}/"
}

install_bundle_binary() {
  local source_name="$1"
  local target_name="${2:-$1}"
  install -m 0755 "${RELEASE_BUNDLE_ROOT}/bin/${source_name}" "/opt/anneal/bin/${target_name}"
}

cleanup_temp_dir() {
  if [[ -n "${DEPLOY_TEMP_DIR:-}" && -d "${DEPLOY_TEMP_DIR}" ]]; then
    rm -rf "${DEPLOY_TEMP_DIR}"
  fi
}

load_platform_info() {
  if [[ -n "${ANNEAL_PLATFORM_ID:-}" ]]; then
    return
  fi
  [[ -f /etc/os-release ]] || {
    show_error "$(text "Не найден /etc/os-release." "The /etc/os-release file was not found.")"
    exit 1
  }
  . /etc/os-release
  ANNEAL_PLATFORM_ID="${ID:-}"
  ANNEAL_PLATFORM_VERSION_ID="${VERSION_ID:-}"
  ANNEAL_PLATFORM_CODENAME="${UBUNTU_CODENAME:-${VERSION_CODENAME:-}}"
}

is_supported_debian_platform() {
  load_platform_info
  [[ "${ANNEAL_PLATFORM_ID}" == "debian" ]] || return 1
  case "${ANNEAL_PLATFORM_VERSION_ID}" in
    10|11|12|13) return 0 ;;
  esac
  return 1
}

is_supported_ubuntu_platform() {
  load_platform_info
  [[ "${ANNEAL_PLATFORM_ID}" == "ubuntu" ]] || return 1
  case "${ANNEAL_PLATFORM_CODENAME}" in
    jammy|noble|plucky|questing) return 0 ;;
  esac
  return 1
}

require_supported_platform() {
  if is_supported_debian_platform || is_supported_ubuntu_platform; then
    return
  fi
  show_error "$(text "Поддерживаются Debian 10/11/12/13 и Ubuntu 22.04/24.04/25.04/25.10." "Supported distributions are Debian 10/11/12/13 and Ubuntu 22.04/24.04/25.04/25.10.")"
  exit 1
}

postgres_repository_base_url() {
  load_platform_info
  if [[ "${ANNEAL_PLATFORM_ID}" == "debian" && "${ANNEAL_PLATFORM_VERSION_ID}" == "10" ]]; then
    printf '%s' "https://apt-archive.postgresql.org/pub/repos/apt"
    return
  fi
  printf '%s' "https://apt.postgresql.org/pub/repos/apt"
}

docker_repository_base_url() {
  load_platform_info
  if [[ "${ANNEAL_PLATFORM_ID}" == "ubuntu" ]]; then
    printf '%s' "https://download.docker.com/linux/ubuntu"
    return
  fi
  printf '%s' "https://download.docker.com/linux/debian"
}

docker_repository_supported_platform() {
  load_platform_info
  if [[ "${ANNEAL_PLATFORM_ID}" == "debian" ]]; then
    case "${ANNEAL_PLATFORM_VERSION_ID}" in
      11|12|13) return 0 ;;
    esac
    return 1
  fi
  case "${ANNEAL_PLATFORM_CODENAME}" in
    jammy|noble|questing) return 0 ;;
  esac
  return 1
}

apt_package_exists() {
  local package_name="$1"
  apt-cache show "${package_name}" >/dev/null 2>&1
}

setup_caddy_repository() {
  local keyring_path
  keyring_path="/usr/share/keyrings/caddy-stable-archive-keyring.asc"
  install -d -m 0755 /usr/share/keyrings
  curl --fail --retry 5 --retry-all-errors --location --silent --show-error \
    https://dl.cloudsmith.io/public/caddy/stable/gpg.key \
    -o "${keyring_path}"
  chmod 0644 "${keyring_path}"
  cat >/etc/apt/sources.list.d/caddy-stable.list <<EOF
deb [signed-by=${keyring_path}] https://dl.cloudsmith.io/public/caddy/stable/deb/debian any-version main
deb-src [signed-by=${keyring_path}] https://dl.cloudsmith.io/public/caddy/stable/deb/debian any-version main
EOF
}

setup_postgres_repository() {
  local codename
  local keyring_path
  keyring_path="/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc"
  load_platform_info
  codename="$(. /etc/os-release && echo "${VERSION_CODENAME}")"
  install -d -m 0755 /usr/share/postgresql-common/pgdg
  curl --fail --retry 5 --retry-all-errors --location --silent --show-error \
    https://www.postgresql.org/media/keys/ACCC4CF8.asc \
    -o "${keyring_path}"
  chmod 0644 "${keyring_path}"
  cat >/etc/apt/sources.list.d/pgdg.list <<EOF
deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] $(postgres_repository_base_url) ${codename}-pgdg main
EOF
}

install_native_control_plane_packages() {
  export DEBIAN_FRONTEND=noninteractive
  require_supported_platform
  setup_postgres_repository
  setup_caddy_repository
  apt-get update
  apt-get install -y ca-certificates curl tar openssl jq whiptail iproute2 debian-keyring debian-archive-keyring apt-transport-https postgresql-17 postgresql-client-17 postgresql-contrib-17 caddy
}

install_native_node_packages() {
  export DEBIAN_FRONTEND=noninteractive
  require_supported_platform
  apt-get update
  apt-get install -y curl tar ca-certificates openssl jq whiptail iproute2
}

setup_docker_repository() {
  local keyring_path
  load_platform_info
  keyring_path="/etc/apt/keyrings/docker.asc"
  install -d -m 0755 /etc/apt/keyrings
  curl --fail --retry 5 --retry-all-errors --location --silent --show-error \
    "$(docker_repository_base_url)/gpg" \
    -o "${keyring_path}"
  chmod 0644 "${keyring_path}"
  cat >/etc/apt/sources.list.d/docker.sources <<EOF
Types: deb
URIs: $(docker_repository_base_url)
Suites: ${ANNEAL_PLATFORM_CODENAME}
Components: stable
Signed-By: ${keyring_path}
EOF
}

remove_conflicting_docker_packages() {
  local packages=()
  local package_name
  for package_name in docker.io docker-doc docker-compose docker-compose-v2 podman-docker containerd runc; do
    if dpkg -s "${package_name}" >/dev/null 2>&1; then
      packages+=("${package_name}")
    fi
  done
  if [[ "${#packages[@]}" -gt 0 ]]; then
    apt-get remove -y "${packages[@]}"
  fi
}

install_docker_packages_from_distro() {
  local packages=(
    curl
    tar
    ca-certificates
    openssl
    jq
    whiptail
    iproute2
    docker.io
  )
  if apt_package_exists docker-compose-plugin; then
    packages+=(docker-compose-plugin)
  elif apt_package_exists docker-compose-v2; then
    packages+=(docker-compose-v2)
  elif apt_package_exists docker-compose; then
    packages+=(docker-compose)
  fi
  apt-get update
  apt-get install -y "${packages[@]}"
}

install_docker_packages() {
  export DEBIAN_FRONTEND=noninteractive
  require_supported_platform
  if docker_repository_supported_platform; then
    remove_conflicting_docker_packages
    setup_docker_repository
    apt-get update
    apt-get install -y curl tar ca-certificates openssl jq whiptail iproute2 docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
  else
    install_docker_packages_from_distro
  fi
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

disable_conflicting_caddy_services() {
  local service
  for service in caddy caddy-api; do
    if systemctl list-unit-files "${service}.service" >/dev/null 2>&1; then
      systemctl disable --now "${service}" >/dev/null 2>&1 || true
    fi
  done
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
  install -m 0755 "${RELEASE_BUNDLE_ROOT}/runtime/xray" /opt/anneal/bin/xray
  install -m 0755 "${RELEASE_BUNDLE_ROOT}/runtime/hiddify-core" /opt/anneal/bin/hiddify-core
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
  rm -rf "${stack_root}/bundle"
  mkdir -p "${stack_root}/bundle"
  cp -a "${RELEASE_BUNDLE_ROOT}/bin" "${stack_root}/bundle/"
  cp -a "${RELEASE_BUNDLE_ROOT}/migrations" "${stack_root}/bundle/"
  cp -a "${RELEASE_BUNDLE_ROOT}/runtime" "${stack_root}/bundle/"
  cp -a "${RELEASE_BUNDLE_ROOT}/web" "${stack_root}/bundle/"
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
ANNEAL_BOOTSTRAP_TOKEN=${ANNEAL_BOOTSTRAP_TOKEN}
ANNEAL_DATA_ENCRYPTION_KEY=${ANNEAL_DATA_ENCRYPTION_KEY}
ANNEAL_TOKEN_HASH_KEY=${ANNEAL_TOKEN_HASH_KEY}
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
ANNEAL_DB_NAME=${ANNEAL_DB_NAME}
ANNEAL_DB_USER=${ANNEAL_DB_USER}
ANNEAL_DB_PASSWORD=${ANNEAL_DB_PASSWORD}
ANNEAL_BIND_ADDRESS=0.0.0.0:8080
ANNEAL_DATABASE_URL=postgres://${ANNEAL_DB_USER}:${ANNEAL_DB_PASSWORD}@postgres:5432/${ANNEAL_DB_NAME}
ANNEAL_MIGRATIONS_DIR=/opt/anneal/migrations
ANNEAL_BOOTSTRAP_TOKEN=${ANNEAL_BOOTSTRAP_TOKEN}
ANNEAL_DATA_ENCRYPTION_KEY=${ANNEAL_DATA_ENCRYPTION_KEY}
ANNEAL_TOKEN_HASH_KEY=${ANNEAL_TOKEN_HASH_KEY}
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
ANNEAL_AGENT_BOOTSTRAP_TOKEN=${ANNEAL_AGENT_BOOTSTRAP_TOKEN}
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
ANNEAL_AGENT_SERVER_URL=${ANNEAL_AGENT_SERVER_URL}
ANNEAL_AGENT_NAME=${ANNEAL_AGENT_NAME}
ANNEAL_AGENT_VERSION=${ANNEAL_VERSION}
ANNEAL_AGENT_ENGINES=${ANNEAL_AGENT_ENGINES}
ANNEAL_AGENT_PROTOCOLS_XRAY=${ANNEAL_AGENT_PROTOCOLS_XRAY}
ANNEAL_AGENT_PROTOCOLS_SINGBOX=${ANNEAL_AGENT_PROTOCOLS_SINGBOX}
ANNEAL_AGENT_BOOTSTRAP_TOKEN=${ANNEAL_AGENT_BOOTSTRAP_TOKEN}
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
      -H "x-bootstrap-token: ${ANNEAL_BOOTSTRAP_TOKEN}" \
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
    ANNEAL_VERSION "${ANNEAL_VERSION}" \
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
      ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}" \
      ANNEAL_VERSION "${ANNEAL_VERSION}"
    return
  fi
  write_kv_file "${SUMMARY_FILE}" \
    ANNEAL_AGENT_SERVER_URL "${ANNEAL_AGENT_SERVER_URL}" \
    ANNEAL_AGENT_NAME "${ANNEAL_AGENT_NAME}" \
    ANNEAL_AGENT_ENGINES "${ANNEAL_AGENT_ENGINES}" \
    ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}" \
    ANNEAL_VERSION "${ANNEAL_VERSION}"
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
  if [[ -n "${RELEASE_BUNDLE_ROOT:-}" && -f "${RELEASE_BUNDLE_ROOT}/install.sh" ]]; then
    install -m 0755 "${RELEASE_BUNDLE_ROOT}/install.sh" "${CONTROL_UTILITY_PATH}"
    return
  fi
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
version: ${ANNEAL_VERSION}
EOF
}

node_install_message() {
  cat <<EOF
$(text "Установка завершена." "Installation completed.")

control_plane_url: ${ANNEAL_AGENT_SERVER_URL}
node_name: ${ANNEAL_AGENT_NAME}
runtimes: ${ANNEAL_AGENT_ENGINES}
release_tag: ${ANNEAL_RELEASE_TAG}
version: ${ANNEAL_VERSION}
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
  install_bundle_binary api
  install_bundle_binary worker
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/web" /opt/anneal/web
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/migrations" /opt/anneal/migrations
  write_control_plane_env_native
  render_native_caddyfile
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service
  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  disable_conflicting_caddy_services
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
  install_bundle_binary node-agent
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
      disable_conflicting_caddy_services
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
  install_bundle_binary api
  install_bundle_binary worker
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/web" /opt/anneal/web
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/migrations" /opt/anneal/migrations
  render_native_caddyfile
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service
  disable_conflicting_caddy_services
  systemctl daemon-reload
  restart_current_install
}

update_native_node() {
  prepare_deploy_assets
  install_bundle_binary node-agent
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
  reset_release_metadata_to_latest
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
    disable_conflicting_caddy_services
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

primary_stack_role() {
  if role_includes_control_plane; then
    printf '%s' "control-plane"
    return
  fi
  printf '%s' "node"
}

secondary_stack_role() {
  if [[ "${ROLE}" == "all-in-one" ]]; then
    printf '%s' "node"
  fi
}

docker_stack_root_for_role() {
  case "$1" in
    control-plane) echo "/opt/anneal/docker/control-plane" ;;
    node) echo "/opt/anneal/docker/node" ;;
    *)
      show_error "$(text "Неизвестная роль." "Unknown role.")"
      exit 1
      ;;
  esac
}

docker_stack_root() {
  docker_stack_root_for_role "$(primary_stack_role)"
}

configure_panel_web_root() {
  local web_root="$1"
  local index_file="${web_root}/index.html"
  if [[ ! -f "${index_file}" ]]; then
    return
  fi
  sed -i "s#<base href=\"[^\"]*\" />#<base href=\"$(panel_base_href)\" />#" "${index_file}"
}

render_control_plane_caddyfile() {
  local output_path="$1"
  local proxy_target="$2"
  local web_root="$3"
  local panel_prefix
  panel_prefix="$(panel_path_prefix)"
  if [[ -z "${panel_prefix}" ]]; then
    cat >"${output_path}" <<EOF
${ANNEAL_DOMAIN} {
    encode gzip zstd

    handle /api/* {
        reverse_proxy ${proxy_target}
    }

    handle /s/* {
        reverse_proxy ${proxy_target}
    }

    handle /swagger-ui* {
        reverse_proxy ${proxy_target}
    }

    handle /api-doc/* {
        reverse_proxy ${proxy_target}
    }

    handle {
        root * ${web_root}
        try_files {path} /index.html
        file_server
    }
}
EOF
    return
  fi
  cat >"${output_path}" <<EOF
${ANNEAL_DOMAIN} {
    encode gzip zstd

    redir ${panel_prefix} $(panel_base_href) 308

    handle ${panel_prefix}/api/* {
        uri strip_prefix ${panel_prefix}
        reverse_proxy ${proxy_target}
    }

    handle ${panel_prefix}/s/* {
        uri strip_prefix ${panel_prefix}
        reverse_proxy ${proxy_target}
    }

    handle ${panel_prefix}/swagger-ui* {
        uri strip_prefix ${panel_prefix}
        reverse_proxy ${proxy_target}
    }

    handle ${panel_prefix}/api-doc/* {
        uri strip_prefix ${panel_prefix}
        reverse_proxy ${proxy_target}
    }

    handle_path ${panel_prefix}/* {
        root * ${web_root}
        try_files {path} /index.html
        file_server
    }

    respond 404
}
EOF
}

render_native_caddyfile() {
  render_control_plane_caddyfile /etc/anneal/Caddyfile 127.0.0.1:8080 /opt/anneal/web
}

write_control_plane_docker_files() {
  local stack_root="$1"
  cp "${stack_root}/control-plane.compose.yml" "${stack_root}/compose.yml"
  render_control_plane_caddyfile "${stack_root}/Caddyfile" api:8080 /srv
}

write_all_in_one_env_native() {
  cat >"${ENV_FILE}" <<EOF
ANNEAL_BIND_ADDRESS=127.0.0.1:8080
ANNEAL_DATABASE_URL=${ANNEAL_DATABASE_URL}
ANNEAL_MIGRATIONS_DIR=/opt/anneal/migrations
ANNEAL_BOOTSTRAP_TOKEN=${ANNEAL_BOOTSTRAP_TOKEN}
ANNEAL_DATA_ENCRYPTION_KEY=${ANNEAL_DATA_ENCRYPTION_KEY}
ANNEAL_TOKEN_HASH_KEY=${ANNEAL_TOKEN_HASH_KEY}
ANNEAL_ACCESS_JWT_SECRET=${ANNEAL_ACCESS_JWT_SECRET}
ANNEAL_PRE_AUTH_JWT_SECRET=${ANNEAL_PRE_AUTH_JWT_SECRET}
ANNEAL_PUBLIC_BASE_URL=${ANNEAL_PUBLIC_BASE_URL}
ANNEAL_CADDY_DOMAIN=${ANNEAL_DOMAIN}
ANNEAL_OTLP_ENDPOINT=${ANNEAL_OTLP_ENDPOINT}
ANNEAL_AGENT_SERVER_URL=${ANNEAL_AGENT_SERVER_URL}
ANNEAL_AGENT_NAME=${ANNEAL_AGENT_NAME}
ANNEAL_AGENT_VERSION=${ANNEAL_VERSION}
ANNEAL_AGENT_ENGINES=${ANNEAL_AGENT_ENGINES}
ANNEAL_AGENT_PROTOCOLS_XRAY=${ANNEAL_AGENT_PROTOCOLS_XRAY}
ANNEAL_AGENT_PROTOCOLS_SINGBOX=${ANNEAL_AGENT_PROTOCOLS_SINGBOX}
ANNEAL_AGENT_BOOTSTRAP_TOKEN=${ANNEAL_AGENT_BOOTSTRAP_TOKEN}
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

save_install_metadata() {
  local primary_role
  local primary_stack_root
  local secondary_role
  local secondary_stack_root
  primary_role="$(primary_stack_role)"
  primary_stack_root="$(docker_stack_root_for_role "${primary_role}")"
  secondary_role="$(secondary_stack_role)"
  secondary_stack_root=""
  if [[ -n "${secondary_role}" ]]; then
    secondary_stack_root="$(docker_stack_root_for_role "${secondary_role}")"
  fi
  write_kv_file "${META_FILE}" \
    ANNEAL_INSTALLER_LANG "${ANNEAL_INSTALLER_LANG}" \
    ANNEAL_INSTALL_ROLE "${ROLE}" \
    ANNEAL_DEPLOYMENT_MODE "${DEPLOYMENT_MODE}" \
    ANNEAL_GITHUB_REPOSITORY "${ANNEAL_GITHUB_REPOSITORY}" \
    ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}" \
    ANNEAL_RELEASE_BASE_URL "${ANNEAL_RELEASE_BASE_URL}" \
    ANNEAL_VERSION "${ANNEAL_VERSION}" \
    ANNEAL_TARGET_TRIPLE "${ANNEAL_TARGET_TRIPLE}" \
    ANNEAL_DOMAIN "${ANNEAL_DOMAIN}" \
    ANNEAL_PANEL_PATH "${ANNEAL_PANEL_PATH}" \
    ANNEAL_PUBLIC_BASE_URL "${ANNEAL_PUBLIC_BASE_URL}" \
    ANNEAL_AGENT_SERVER_URL "${ANNEAL_AGENT_SERVER_URL}" \
    ANNEAL_AGENT_NAME "${ANNEAL_AGENT_NAME}" \
    ANNEAL_AGENT_ENGINES "${ANNEAL_AGENT_ENGINES}" \
    ANNEAL_RESELLER_TENANT_NAME "${ANNEAL_RESELLER_TENANT_NAME}" \
    ANNEAL_RESELLER_EMAIL "${ANNEAL_RESELLER_EMAIL}" \
    ANNEAL_RESELLER_DISPLAY_NAME "${ANNEAL_RESELLER_DISPLAY_NAME}" \
    ANNEAL_RESELLER_PASSWORD "${ANNEAL_RESELLER_PASSWORD}" \
    ANNEAL_NODE_GROUP_NAME "${ANNEAL_NODE_GROUP_NAME}" \
    ANNEAL_STACK_ROOT "${primary_stack_root}" \
    ANNEAL_COMPOSE_FILE "${primary_stack_root}/compose.yml" \
    ANNEAL_SECONDARY_STACK_ROOT "${secondary_stack_root}" \
    ANNEAL_SECONDARY_COMPOSE_FILE "${secondary_stack_root:+${secondary_stack_root}/compose.yml}"
}

save_admin_summary() {
  if [[ "${ROLE}" == "all-in-one" ]]; then
    write_kv_file "${SUMMARY_FILE}" \
      ANNEAL_PUBLIC_BASE_URL "${ANNEAL_PUBLIC_BASE_URL}" \
      ANNEAL_PANEL_PATH "${ANNEAL_PANEL_PATH}" \
      ANNEAL_SUPERADMIN_EMAIL "${ANNEAL_SUPERADMIN_EMAIL}" \
      ANNEAL_SUPERADMIN_PASSWORD "${ANNEAL_SUPERADMIN_PASSWORD}" \
      ANNEAL_DATABASE_URL "${ANNEAL_DATABASE_URL}" \
      ANNEAL_RESELLER_TENANT_NAME "${ANNEAL_RESELLER_TENANT_NAME}" \
      ANNEAL_RESELLER_EMAIL "${ANNEAL_RESELLER_EMAIL}" \
      ANNEAL_RESELLER_PASSWORD "${ANNEAL_RESELLER_PASSWORD}" \
      ANNEAL_NODE_GROUP_NAME "${ANNEAL_NODE_GROUP_NAME}" \
      ANNEAL_AGENT_ENGINES "${ANNEAL_AGENT_ENGINES}" \
      ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}" \
      ANNEAL_VERSION "${ANNEAL_VERSION}"
    return
  fi
  if [[ "${ROLE}" == "control-plane" ]]; then
    write_kv_file "${SUMMARY_FILE}" \
      ANNEAL_PUBLIC_BASE_URL "${ANNEAL_PUBLIC_BASE_URL}" \
      ANNEAL_PANEL_PATH "${ANNEAL_PANEL_PATH}" \
      ANNEAL_SUPERADMIN_EMAIL "${ANNEAL_SUPERADMIN_EMAIL}" \
      ANNEAL_SUPERADMIN_PASSWORD "${ANNEAL_SUPERADMIN_PASSWORD}" \
      ANNEAL_DATABASE_URL "${ANNEAL_DATABASE_URL}" \
      ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}" \
      ANNEAL_VERSION "${ANNEAL_VERSION}"
    return
  fi
  write_kv_file "${SUMMARY_FILE}" \
    ANNEAL_AGENT_SERVER_URL "${ANNEAL_AGENT_SERVER_URL}" \
    ANNEAL_AGENT_NAME "${ANNEAL_AGENT_NAME}" \
    ANNEAL_AGENT_ENGINES "${ANNEAL_AGENT_ENGINES}" \
    ANNEAL_RELEASE_TAG "${ANNEAL_RELEASE_TAG}" \
    ANNEAL_VERSION "${ANNEAL_VERSION}"
}

control_plane_install_message() {
  cat <<EOF
$(text "Установка завершена." "Installation completed.")

panel_url: ${ANNEAL_PUBLIC_BASE_URL}
panel_path: $(panel_path_prefix)
admin_email: ${ANNEAL_SUPERADMIN_EMAIL}
admin_password: ${ANNEAL_SUPERADMIN_PASSWORD}
database_url: ${ANNEAL_DATABASE_URL}
release_tag: ${ANNEAL_RELEASE_TAG}
version: ${ANNEAL_VERSION}
EOF
}

all_in_one_install_message() {
  cat <<EOF
$(text "Установка завершена." "Installation completed.")

panel_url: ${ANNEAL_PUBLIC_BASE_URL}
panel_path: $(panel_path_prefix)
admin_email: ${ANNEAL_SUPERADMIN_EMAIL}
admin_password: ${ANNEAL_SUPERADMIN_PASSWORD}
tenant_name: ${ANNEAL_RESELLER_TENANT_NAME}
tenant_admin_email: ${ANNEAL_RESELLER_EMAIL}
tenant_admin_password: ${ANNEAL_RESELLER_PASSWORD}
node_name: ${ANNEAL_NODE_GROUP_NAME}
runtimes: ${ANNEAL_AGENT_ENGINES}
release_tag: ${ANNEAL_RELEASE_TAG}
version: ${ANNEAL_VERSION}
EOF
}

install_native_control_plane() {
  finalize_control_plane_defaults
  [[ -n "${ANNEAL_DOMAIN}" ]] || {
    show_error "$(text "Для control-plane нужен домен." "Control-plane requires a domain.")"
    exit 1
  }
  prepare_deploy_assets
  install_native_control_plane_packages
  ensure_user
  ensure_postgres
  install_bundle_binary api
  install_bundle_binary worker
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/web" /opt/anneal/web
  configure_panel_web_root /opt/anneal/web
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/migrations" /opt/anneal/migrations
  write_control_plane_env_native
  render_native_caddyfile
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service
  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  disable_conflicting_caddy_services
  systemctl daemon-reload
  activate_native_services postgresql anneal-api anneal-worker anneal-caddy
  wait_for_api
  bootstrap_superadmin
}

install_native_node() {
  finalize_node_defaults
  prepare_deploy_assets
  install_native_node_packages
  ensure_user
  install_bundle_binary node-agent
  install_runtime_bundle_native
  install_runtime_defaults
  write_node_env_native
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-node-agent.service" /etc/systemd/system/anneal-node-agent.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-xray.service" /etc/systemd/system/anneal-xray.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-singbox.service" /etc/systemd/system/anneal-singbox.service
  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  systemctl daemon-reload
  activate_native_services anneal-xray anneal-singbox anneal-node-agent
}

install_native_all_in_one() {
  finalize_single_server_defaults
  prepare_deploy_assets
  install_native_control_plane_packages
  ensure_user
  ensure_postgres
  install_bundle_binary api
  install_bundle_binary worker
  install_bundle_binary node-agent
  install_runtime_bundle_native
  install_runtime_defaults
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/web" /opt/anneal/web
  configure_panel_web_root /opt/anneal/web
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/migrations" /opt/anneal/migrations
  write_all_in_one_env_native
  render_native_caddyfile
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-node-agent.service" /etc/systemd/system/anneal-node-agent.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-xray.service" /etc/systemd/system/anneal-xray.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-singbox.service" /etc/systemd/system/anneal-singbox.service
  chown -R "${ANNEAL_USER}:${ANNEAL_GROUP}" /opt/anneal /var/lib/anneal
  disable_conflicting_caddy_services
  systemctl daemon-reload
  activate_native_services postgresql anneal-api anneal-worker anneal-caddy
  wait_for_api
  bootstrap_superadmin
  bootstrap_single_server_node
  wait_for_public_api
  write_all_in_one_env_native
  activate_native_services anneal-xray anneal-singbox anneal-node-agent
}

install_docker_control_plane() {
  finalize_control_plane_defaults
  [[ -n "${ANNEAL_DOMAIN}" ]] || {
    show_error "$(text "Для control-plane нужен домен." "Control-plane requires a domain.")"
    exit 1
  }
  prepare_deploy_assets
  install_docker_packages
  local stack_root
  stack_root="$(docker_stack_root_for_role control-plane)"
  sync_docker_stack_assets "${stack_root}"
  configure_panel_web_root "${stack_root}/bundle/web"
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
  stack_root="$(docker_stack_root_for_role node)"
  sync_docker_stack_assets "${stack_root}"
  write_node_docker_files "${stack_root}"
  write_node_env_docker "${stack_root}"
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" build
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" up -d
}

install_docker_all_in_one() {
  local control_stack_root
  local node_stack_root
  finalize_single_server_defaults
  prepare_deploy_assets
  install_docker_packages
  control_stack_root="$(docker_stack_root_for_role control-plane)"
  node_stack_root="$(docker_stack_root_for_role node)"
  sync_docker_stack_assets "${control_stack_root}"
  configure_panel_web_root "${control_stack_root}/bundle/web"
  write_control_plane_docker_files "${control_stack_root}"
  write_control_plane_env_docker "${control_stack_root}"
  compose_cmd -f "${control_stack_root}/compose.yml" --env-file "${control_stack_root}/.env" build
  compose_cmd -f "${control_stack_root}/compose.yml" --env-file "${control_stack_root}/.env" up -d
  wait_for_api
  bootstrap_superadmin
  bootstrap_single_server_node
  wait_for_public_api
  sync_docker_stack_assets "${node_stack_root}"
  write_node_docker_files "${node_stack_root}"
  write_node_env_docker "${node_stack_root}"
  compose_cmd -f "${node_stack_root}/compose.yml" --env-file "${node_stack_root}/.env" build
  compose_cmd -f "${node_stack_root}/compose.yml" --env-file "${node_stack_root}/.env" up -d
}

docker_status_output() {
  local stack_root="$1"
  local label="$2"
  local env_file="${stack_root}/.env"
  if [[ ! -f "${env_file}" ]]; then
    return
  fi
  printf '%s\n%s\n' "${label}" "$(compose_cmd -f "${stack_root}/compose.yml" --env-file "${env_file}" ps)"
}

status_summary() {
  if [[ "${DEPLOYMENT_MODE}" == "native" ]]; then
    {
      if role_includes_control_plane; then
        service_status_line postgresql
        service_status_line anneal-api
        service_status_line anneal-worker
        service_status_line anneal-caddy
      fi
      if role_includes_node; then
        service_status_line anneal-node-agent
        service_status_line anneal-xray
        service_status_line anneal-singbox
      fi
    }
    return
  fi
  {
    docker_status_output "${ANNEAL_STACK_ROOT}" "primary stack"
    if [[ -n "${ANNEAL_SECONDARY_STACK_ROOT:-}" ]]; then
      docker_status_output "${ANNEAL_SECONDARY_STACK_ROOT}" "secondary stack"
    fi
  }
}

restart_current_install() {
  if [[ "${DEPLOYMENT_MODE}" == "native" ]]; then
    if role_includes_control_plane; then
      disable_conflicting_caddy_services
      systemctl restart anneal-api anneal-worker anneal-caddy
    fi
    if role_includes_node; then
      systemctl restart anneal-node-agent anneal-xray anneal-singbox
    fi
    return
  fi
  compose_cmd -f "${ANNEAL_COMPOSE_FILE}" --env-file "${ANNEAL_STACK_ROOT}/.env" restart
  if [[ -n "${ANNEAL_SECONDARY_STACK_ROOT:-}" ]]; then
    compose_cmd -f "${ANNEAL_SECONDARY_COMPOSE_FILE}" --env-file "${ANNEAL_SECONDARY_STACK_ROOT}/.env" restart
  fi
}

update_native_control_plane() {
  prepare_deploy_assets
  install_bundle_binary api
  install_bundle_binary worker
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/web" /opt/anneal/web
  configure_panel_web_root /opt/anneal/web
  sync_directory_contents "${RELEASE_BUNDLE_ROOT}/migrations" /opt/anneal/migrations
  render_native_caddyfile
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-api.service" /etc/systemd/system/anneal-api.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-worker.service" /etc/systemd/system/anneal-worker.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-caddy.service" /etc/systemd/system/anneal-caddy.service
}

update_native_node() {
  prepare_deploy_assets
  install_bundle_binary node-agent
  install_runtime_bundle_native
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-node-agent.service" /etc/systemd/system/anneal-node-agent.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-xray.service" /etc/systemd/system/anneal-xray.service
  install -m 0644 "${DEPLOY_ASSET_ROOT}/systemd/anneal-singbox.service" /etc/systemd/system/anneal-singbox.service
}

update_docker_stack() {
  local role_name="$1"
  local stack_root
  stack_root="$(docker_stack_root_for_role "${role_name}")"
  sync_docker_stack_assets "${stack_root}"
  if [[ "${role_name}" == "control-plane" ]]; then
    configure_panel_web_root "${stack_root}/bundle/web"
    write_control_plane_docker_files "${stack_root}"
  else
    write_node_docker_files "${stack_root}"
  fi
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" build
  compose_cmd -f "${stack_root}/compose.yml" --env-file "${stack_root}/.env" up -d
}

update_current_install() {
  load_admin_summary
  reset_release_metadata_to_latest
  if [[ "${DEPLOYMENT_MODE}" == "native" ]]; then
    if role_includes_control_plane; then
      update_native_control_plane
    fi
    if role_includes_node; then
      update_native_node
    fi
    disable_conflicting_caddy_services
    systemctl daemon-reload
    restart_current_install
  else
    update_docker_stack "$(primary_stack_role)"
    if [[ "${ROLE}" == "all-in-one" ]]; then
      update_docker_stack node
    fi
  fi
  install_control_utility
  install_profile_hook
  save_install_metadata
  save_admin_summary
}

uninstall_native_current() {
  if role_includes_control_plane; then
    disable_conflicting_caddy_services
    systemctl disable --now anneal-api anneal-worker anneal-caddy >/dev/null 2>&1 || true
    rm -f /etc/systemd/system/anneal-api.service /etc/systemd/system/anneal-worker.service /etc/systemd/system/anneal-caddy.service
    load_admin_summary
    [[ -n "${ANNEAL_DATABASE_URL:-}" ]] && drop_local_database_if_possible
  fi
  if role_includes_node; then
    systemctl disable --now anneal-node-agent anneal-xray anneal-singbox >/dev/null 2>&1 || true
    rm -f /etc/systemd/system/anneal-node-agent.service /etc/systemd/system/anneal-xray.service /etc/systemd/system/anneal-singbox.service
  fi
  systemctl daemon-reload
}

uninstall_docker_current() {
  compose_cmd -f "${ANNEAL_COMPOSE_FILE}" --env-file "${ANNEAL_STACK_ROOT}/.env" down -v || true
  rm -rf "${ANNEAL_STACK_ROOT}"
  if [[ -n "${ANNEAL_SECONDARY_STACK_ROOT:-}" ]]; then
    compose_cmd -f "${ANNEAL_SECONDARY_COMPOSE_FILE}" --env-file "${ANNEAL_SECONDARY_STACK_ROOT}/.env" down -v || true
    rm -rf "${ANNEAL_SECONDARY_STACK_ROOT}"
  fi
}

show_admin_details() {
  load_admin_summary
  if [[ "${ROLE}" == "all-in-one" ]]; then
    show_info "$(text "Данные установки" "Install details")" "$(all_in_one_install_message)"
    return
  fi
  if [[ "${ROLE}" == "control-plane" ]]; then
    show_info "$(text "Данные администратора" "Administrator details")" "$(control_plane_install_message)"
    return
  fi
  show_info "$(text "Данные ноды" "Node server details")" "$(node_install_message)"
}

run_install() {
  configure_installation
  case "${ROLE}:${DEPLOYMENT_MODE}" in
    all-in-one:native) install_native_all_in_one ;;
    all-in-one:docker) install_docker_all_in_one ;;
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
  case "${ROLE}" in
    all-in-one) printf '%s\n' "$(all_in_one_install_message)" ;;
    control-plane) printf '%s\n' "$(control_plane_install_message)" ;;
    *) printf '%s\n' "$(node_install_message)" ;;
  esac
}

logo_block() {
  printf '%b' '\u2581\u2583\u2586\u2588 Anneal'
}

print_banner() {
  printf '%b' '\033[38;5;64m\u2581\033[38;5;107m\u2583\033[38;5;149m\u2586\033[38;5;191m\u2588\033[0m \033[38;5;194mAnneal\033[0m'
  printf '\n\n'
}

show_step() {
  local message="$1"
  printf '%s\n' "${message}"
}

activate_native_services() {
  local services=("$@")
  systemctl enable "${services[@]}" >/dev/null 2>&1
  systemctl restart "${services[@]}"
}

wait_for_public_api() {
  local health_url
  health_url="${ANNEAL_PUBLIC_BASE_URL}/api/v1/health"
  show_step "$(text "Жду публичный HTTPS панели..." "Waiting for the public panel HTTPS...")"
  for _ in $(seq 1 120); do
    if curl --silent --show-error --fail --insecure --connect-timeout 2 --max-time 5 --resolve "${ANNEAL_DOMAIN}:443:127.0.0.1" "${health_url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  show_error "$(text "Публичный HTTPS панели не поднялся вовремя. Проверь Caddy через systemctl status anneal-caddy и journalctl -u anneal-caddy -n 100 --no-pager." "Public panel HTTPS did not become ready in time. Check Caddy with systemctl status anneal-caddy and journalctl -u anneal-caddy -n 100 --no-pager.")"
  exit 1
}

wait_for_api() {
  local url="http://127.0.0.1:8080/api/v1/health"
  show_step "$(text "Жду готовность Anneal API на 127.0.0.1:8080..." "Waiting for Anneal API on 127.0.0.1:8080...")"
  for _ in $(seq 1 120); do
    if curl --silent --show-error --fail --connect-timeout 2 --max-time 5 "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  show_error "$(text "API не поднялся вовремя. Проверь systemctl status anneal-api anneal-worker anneal-caddy --no-pager -l и journalctl -u anneal-api -n 100 --no-pager." "API did not become ready in time. Check systemctl status anneal-api anneal-worker anneal-caddy --no-pager -l and journalctl -u anneal-api -n 100 --no-pager.")"
  exit 1
}

bootstrap_superadmin() {
  local response_file
  local status
  show_step "$(text "Выполняю bootstrap супер-админа..." "Bootstrapping the superadmin...")"
  response_file="$(mktemp)"
  status="$(
    curl -sS -o "${response_file}" -w '%{http_code}' \
      http://127.0.0.1:8080/api/v1/bootstrap \
      -H 'content-type: application/json' \
      -H "x-bootstrap-token: ${ANNEAL_BOOTSTRAP_TOKEN}" \
      --data "$(jq -nc --arg email "${ANNEAL_SUPERADMIN_EMAIL}" --arg display_name "${ANNEAL_SUPERADMIN_DISPLAY_NAME}" --arg password "${ANNEAL_SUPERADMIN_PASSWORD}" '{email:$email, display_name:$display_name, password:$password}')"
  )"
  if [[ "${status}" == "200" || "${status}" == "409" ]]; then
    rm -f "${response_file}"
    return
  fi
  cat "${response_file}" >&2
  rm -f "${response_file}"
  show_error "$(text "Не удалось выполнить bootstrap супер-админа." "Failed to bootstrap the superadmin.")"
  exit 1
}

domain_prompt_default() {
  local value="${ANNEAL_DOMAIN:-panel.example.com}"
  normalize_domain_input "${value}"
}

finalize_control_plane_defaults() {
  hydrate_control_plane_access_from_public_url
  if [[ -z "${ANNEAL_DOMAIN}" ]]; then
    ANNEAL_DOMAIN="panel.example.com"
  else
    ANNEAL_DOMAIN="$(normalize_domain_input "${ANNEAL_DOMAIN}")"
  fi
  if [[ -z "${ANNEAL_PANEL_PATH}" ]]; then
    ANNEAL_PANEL_PATH="$(generate_panel_path)"
  fi
  ANNEAL_PUBLIC_BASE_URL="https://${ANNEAL_DOMAIN}$(panel_path_prefix)"
  if [[ -z "${ANNEAL_SUPERADMIN_EMAIL}" ]]; then
    ANNEAL_SUPERADMIN_EMAIL="admin-$(generate_hex 3)@${ANNEAL_DOMAIN}"
  fi
}

finalize_single_server_defaults() {
  finalize_control_plane_defaults
  finalize_node_defaults
  if [[ -z "${ANNEAL_RESELLER_TENANT_NAME}" ]]; then
    ANNEAL_RESELLER_TENANT_NAME="Default Tenant"
  fi
  if [[ -z "${ANNEAL_RESELLER_DISPLAY_NAME}" ]]; then
    ANNEAL_RESELLER_DISPLAY_NAME="Tenant Admin"
  fi
  if [[ -z "${ANNEAL_RESELLER_EMAIL}" ]]; then
    ANNEAL_RESELLER_EMAIL="tenant-$(generate_hex 3)@${ANNEAL_DOMAIN}"
  fi
  if [[ -z "${ANNEAL_RESELLER_PASSWORD}" ]]; then
    ANNEAL_RESELLER_PASSWORD="$(generate_secret 18)"
  fi
  if [[ -z "${ANNEAL_NODE_GROUP_NAME}" ]]; then
    ANNEAL_NODE_GROUP_NAME="edge-$(generate_hex 3)"
  fi
  ANNEAL_AGENT_NAME="${ANNEAL_NODE_GROUP_NAME}"
}

configure_control_plane_tui() {
  local domain_default
  domain_default="$(domain_prompt_default)"
  ANNEAL_DOMAIN="$(prompt_text \
    "Anneal • Control Plane" \
    "$(text "Укажи домен или ссылку панели. Приватный путь будет сгенерирован автоматически." "Enter the panel domain or URL. The private path will be generated automatically.")" \
    "${domain_default}")"
  finalize_control_plane_defaults
  ANNEAL_SUPERADMIN_EMAIL="$(prompt_text \
    "Anneal • Control Plane" \
    "$(text "Email bootstrap-суперадмина." "Enter the bootstrap superadmin email.")" \
    "${ANNEAL_SUPERADMIN_EMAIL}")"
  ANNEAL_SUPERADMIN_DISPLAY_NAME="$(prompt_text \
    "Anneal • Control Plane" \
    "$(text "Отображаемое имя суперадмина." "Enter the superadmin display name.")" \
    "${ANNEAL_SUPERADMIN_DISPLAY_NAME}")"
  if ! prompt_confirm "$(text "Подтверждение" "Confirmation")" "$(control_plane_summary)"; then
    exit 1
  fi
}

configure_all_in_one_tui() {
  local domain_default
  domain_default="$(domain_prompt_default)"
  ANNEAL_DOMAIN="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Укажи домен или ссылку панели. Приватный путь будет сгенерирован автоматически." "Enter the panel domain or URL. The private path will be generated automatically.")" \
    "${domain_default}")"
  finalize_single_server_defaults
  ANNEAL_SUPERADMIN_EMAIL="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Email bootstrap-суперадмина." "Enter the bootstrap superadmin email.")" \
    "${ANNEAL_SUPERADMIN_EMAIL}")"
  ANNEAL_SUPERADMIN_DISPLAY_NAME="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Отображаемое имя суперадмина." "Enter the superadmin display name.")" \
    "${ANNEAL_SUPERADMIN_DISPLAY_NAME}")"
  ANNEAL_RESELLER_TENANT_NAME="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Название tenant по умолчанию." "Enter the default tenant name.")" \
    "${ANNEAL_RESELLER_TENANT_NAME}")"
  ANNEAL_NODE_GROUP_NAME="$(prompt_text \
    "Anneal • All-in-one" \
    "$(text "Имя локальной ноды." "Enter the local node name.")" \
    "${ANNEAL_NODE_GROUP_NAME}")"
  ANNEAL_AGENT_ENGINES="$(prompt_checklist \
    "Anneal • Runtime packages" \
    "$(text "Выбери runtime-пакеты для локальной ноды." "Choose runtime packages for the local node.")" \
    "xray" "Xray" "ON" \
    "singbox" "Sing-box" "ON")"
  if ! prompt_confirm "$(text "Подтверждение" "Confirmation")" "$(all_in_one_summary)"; then
    exit 1
  fi
}

ACTION="${ACTION:-install}"
ROLE="${ROLE:-}"
DEPLOYMENT_MODE="${DEPLOYMENT_MODE:-}"
LOGIN_SHELL=0
ANNEAL_INSTALLER_LANG="${ANNEAL_INSTALLER_LANG:-}"
ANNEAL_INSTALLER_UI="${ANNEAL_INSTALLER_UI:-auto}"
ANNEAL_GITHUB_REPOSITORY="${ANNEAL_GITHUB_REPOSITORY:-Anneal-Team/Anneal-Panel}"
REQUESTED_RELEASE_TAG="${ANNEAL_RELEASE_TAG:-}"
REQUESTED_RELEASE_VERSION="${ANNEAL_VERSION:-}"
REQUESTED_RELEASE_BASE_URL="${ANNEAL_RELEASE_BASE_URL:-}"
ANNEAL_RELEASE_TAG="${REQUESTED_RELEASE_TAG}"
ANNEAL_TARGET_TRIPLE="${ANNEAL_TARGET_TRIPLE:-linux-amd64}"
ANNEAL_VERSION="${REQUESTED_RELEASE_VERSION}"
ANNEAL_RELEASE_BASE_URL="${REQUESTED_RELEASE_BASE_URL}"
ANNEAL_USER="${ANNEAL_USER:-anneal}"
ANNEAL_GROUP="${ANNEAL_GROUP:-anneal}"
ANNEAL_DOMAIN="${ANNEAL_DOMAIN:-}"
ANNEAL_PANEL_PATH="${ANNEAL_PANEL_PATH:-}"
ANNEAL_PUBLIC_BASE_URL="${ANNEAL_PUBLIC_BASE_URL:-}"
ANNEAL_DB_NAME="${ANNEAL_DB_NAME:-anneal_$(generate_hex 4)}"
ANNEAL_DB_USER="${ANNEAL_DB_USER:-anneal_$(generate_hex 4)}"
ANNEAL_DB_PASSWORD="${ANNEAL_DB_PASSWORD:-$(generate_secret 18)}"
ANNEAL_DATABASE_HOST="${ANNEAL_DATABASE_HOST:-127.0.0.1}"
ANNEAL_DATABASE_PORT="${ANNEAL_DATABASE_PORT:-5432}"
ANNEAL_DATABASE_URL="${ANNEAL_DATABASE_URL:-postgres://${ANNEAL_DB_USER}:${ANNEAL_DB_PASSWORD}@${ANNEAL_DATABASE_HOST}:${ANNEAL_DATABASE_PORT}/${ANNEAL_DB_NAME}}"
ANNEAL_BOOTSTRAP_TOKEN="${ANNEAL_BOOTSTRAP_TOKEN:-$(generate_secret 24)}"
ANNEAL_DATA_ENCRYPTION_KEY="${ANNEAL_DATA_ENCRYPTION_KEY:-$(generate_hex 32)}"
ANNEAL_TOKEN_HASH_KEY="${ANNEAL_TOKEN_HASH_KEY:-$(generate_hex 32)}"
ANNEAL_ACCESS_JWT_SECRET="${ANNEAL_ACCESS_JWT_SECRET:-$(generate_hex 32)}"
ANNEAL_PRE_AUTH_JWT_SECRET="${ANNEAL_PRE_AUTH_JWT_SECRET:-$(generate_hex 32)}"
ANNEAL_SUPERADMIN_EMAIL="${ANNEAL_SUPERADMIN_EMAIL:-}"
ANNEAL_SUPERADMIN_DISPLAY_NAME="${ANNEAL_SUPERADMIN_DISPLAY_NAME:-Superadmin}"
ANNEAL_SUPERADMIN_PASSWORD="${ANNEAL_SUPERADMIN_PASSWORD:-$(generate_secret 18)}"
ANNEAL_RESELLER_TENANT_NAME="${ANNEAL_RESELLER_TENANT_NAME:-}"
ANNEAL_RESELLER_EMAIL="${ANNEAL_RESELLER_EMAIL:-}"
ANNEAL_RESELLER_DISPLAY_NAME="${ANNEAL_RESELLER_DISPLAY_NAME:-}"
ANNEAL_RESELLER_PASSWORD="${ANNEAL_RESELLER_PASSWORD:-}"
ANNEAL_NODE_GROUP_NAME="${ANNEAL_NODE_GROUP_NAME:-}"
ANNEAL_OTLP_ENDPOINT="${ANNEAL_OTLP_ENDPOINT:-}"
ANNEAL_AGENT_SERVER_URL="${ANNEAL_AGENT_SERVER_URL:-}"
ANNEAL_AGENT_NAME="${ANNEAL_AGENT_NAME:-}"
ANNEAL_AGENT_ENGINES="${ANNEAL_AGENT_ENGINES:-}"
ANNEAL_AGENT_PROTOCOLS_XRAY="${ANNEAL_AGENT_PROTOCOLS_XRAY:-}"
ANNEAL_AGENT_PROTOCOLS_SINGBOX="${ANNEAL_AGENT_PROTOCOLS_SINGBOX:-}"
ANNEAL_AGENT_XRAY_TOKEN="${ANNEAL_AGENT_XRAY_TOKEN:-}"
ANNEAL_AGENT_SINGBOX_TOKEN="${ANNEAL_AGENT_SINGBOX_TOKEN:-}"
ANNEAL_AGENT_ENROLLMENT_TOKENS="${ANNEAL_AGENT_ENROLLMENT_TOKENS:-}"
ANNEAL_AGENT_BOOTSTRAP_TOKEN="${ANNEAL_AGENT_BOOTSTRAP_TOKEN:-}"
SELF_SOURCE="$(detect_self_source)"
SCRIPT_DIR="$(detect_script_dir "${SELF_SOURCE}")"
ENV_FILE="/etc/anneal/anneal.env"
META_FILE="/etc/anneal/install.meta"
SUMMARY_FILE="/etc/anneal/admin-summary.env"
CONTROL_UTILITY_PATH="/usr/local/bin/annealctl"
PROFILE_HOOK_PATH="/etc/profile.d/anneal-menu.sh"
DEPLOY_ASSET_ROOT=""
DEPLOY_TEMP_DIR=""
RELEASE_BUNDLE_ROOT=""
ANNEAL_SECONDARY_STACK_ROOT=""
ANNEAL_SECONDARY_COMPOSE_FILE=""

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
