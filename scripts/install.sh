#!/usr/bin/env bash
set -euo pipefail

ANNEAL_GITHUB_REPOSITORY="${ANNEAL_GITHUB_REPOSITORY:-Anneal-Team/Anneal-Panel}"
ANNEAL_RELEASE_TAG="${ANNEAL_RELEASE_TAG:-}"
ANNEAL_VERSION="${ANNEAL_VERSION:-}"
ANNEAL_RELEASE_BASE_URL="${ANNEAL_RELEASE_BASE_URL:-}"
ANNEAL_TARGET_TRIPLE="${ANNEAL_TARGET_TRIPLE:-}"
TEMP_DIR=""

cleanup() {
  if [[ -n "${TEMP_DIR}" && -d "${TEMP_DIR}" ]]; then
    rm -rf "${TEMP_DIR}"
  fi
}

trap cleanup EXIT

require_tool() {
  command -v "$1" >/dev/null 2>&1 || {
    printf 'missing required tool: %s\n' "$1" >&2
    exit 1
  }
}

require_root() {
  if [[ "$(id -u)" != "0" ]]; then
    printf 'Run the installer as root.\n' >&2
    exit 1
  fi
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
    --user-agent "Anneal-Installer/2.0" \
    -H "Accept: application/vnd.github+json" \
    "${url}"
}

resolve_latest_release_tag() {
  local response
  response="$(github_api_get "https://api.github.com/repos/${ANNEAL_GITHUB_REPOSITORY}/releases/latest")"
  printf '%s' "${response}" |
    tr -d '\r\n' |
    sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
    head -n 1
}

resolve_default_release_tag() {
  if github_api_get "https://api.github.com/repos/${ANNEAL_GITHUB_REPOSITORY}/releases/tags/rolling-master" >/dev/null 2>&1; then
    printf '%s' "rolling-master"
    return
  fi
  resolve_latest_release_tag
}

detect_target_triple() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"
  [[ "${os}" == "linux" ]] || {
    printf 'unsupported OS: %s\n' "${os}" >&2
    exit 1
  }
  case "${arch}" in
    x86_64|amd64) printf '%s' "linux-amd64" ;;
    aarch64|arm64) printf '%s' "linux-arm64" ;;
    *)
      printf 'unsupported architecture: %s\n' "${arch}" >&2
      exit 1
      ;;
  esac
}

load_release_metadata() {
  if [[ -z "${ANNEAL_RELEASE_TAG}" ]]; then
    ANNEAL_RELEASE_TAG="$(resolve_default_release_tag)"
  fi
  [[ -n "${ANNEAL_RELEASE_TAG}" ]] || {
    printf 'failed to resolve Anneal release tag\n' >&2
    exit 1
  }
  if [[ -z "${ANNEAL_VERSION}" ]]; then
    ANNEAL_VERSION="$(normalize_release_version "${ANNEAL_RELEASE_TAG}")"
  fi
  if [[ -z "${ANNEAL_TARGET_TRIPLE}" ]]; then
    ANNEAL_TARGET_TRIPLE="$(detect_target_triple)"
  fi
  if [[ -z "${ANNEAL_RELEASE_BASE_URL}" ]]; then
    ANNEAL_RELEASE_BASE_URL="https://github.com/${ANNEAL_GITHUB_REPOSITORY}/releases/download/${ANNEAL_RELEASE_TAG}"
  fi
}

download_asset() {
  local url="$1"
  local destination="$2"
  curl \
    --fail \
    --retry 5 \
    --retry-all-errors \
    --location \
    --silent \
    --show-error \
    --user-agent "Anneal-Installer/2.0" \
    "${url}" \
    -o "${destination}"
}

verify_bundle_checksums() {
  local bundle_root="$1"
  [[ -f "${bundle_root}/SHA256SUMS" ]] || {
    printf 'bundle is missing SHA256SUMS: %s\n' "${bundle_root}" >&2
    exit 1
  }
  (
    cd "${bundle_root}"
    sha256sum -c "SHA256SUMS" >/dev/null
  )
}

extract_bundle_root() {
  local archive="$1"
  local extract_root="$2"
  tar -xzf "${archive}" -C "${extract_root}"
  shopt -s nullglob
  local entries=("${extract_root}"/*)
  shopt -u nullglob
  [[ "${#entries[@]}" -eq 1 && -d "${entries[0]}" ]] || {
    printf 'bundle archive must contain exactly one top-level directory\n' >&2
    exit 1
  }
  printf '%s' "${entries[0]}"
}

launch_installer() {
  local control_utility="$1"
  local bundle_root="$2"
  shift 2
  if [[ ! -t 0 && -r /dev/tty ]]; then
    "${control_utility}" install --bundle-root "${bundle_root}" "$@" </dev/tty
    return
  fi
  "${control_utility}" install --bundle-root "${bundle_root}" "$@"
}

main() {
  require_tool curl
  require_tool tar
  require_tool sha256sum
  require_tool sed
  require_tool uname
  require_tool id

  require_root

  load_release_metadata

  TEMP_DIR="$(mktemp -d)"
  local control_utility_path bundle_archive bundle_root
  bundle_archive="anneal-${ANNEAL_VERSION}-${ANNEAL_TARGET_TRIPLE}.tar.gz"

  download_asset "${ANNEAL_RELEASE_BASE_URL}/${bundle_archive}" "${TEMP_DIR}/${bundle_archive}"

  mkdir -p "${TEMP_DIR}/bundle"
  bundle_root="$(extract_bundle_root "${TEMP_DIR}/${bundle_archive}" "${TEMP_DIR}/bundle")"
  verify_bundle_checksums "${bundle_root}"

  control_utility_path="${bundle_root}/bin/annealctl"

  [[ -x "${control_utility_path}" ]] || {
    printf 'bundle is missing annealctl: %s\n' "${control_utility_path}" >&2
    exit 1
  }

  launch_installer "${control_utility_path}" "${bundle_root}" "$@"
}

main "$@"
