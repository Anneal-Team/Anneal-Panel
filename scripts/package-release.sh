#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
TMP_DIR="${DIST_DIR}/tmp"
TARGET_TRIPLE="${TARGET_TRIPLE:-linux-amd64}"
XRAY_RELEASE_URL="${XRAY_RELEASE_URL:-https://github.com/XTLS/Xray-core/releases/download/v26.2.6/Xray-linux-64.zip}"
XRAY_RELEASE_FALLBACK_URL="${XRAY_RELEASE_FALLBACK_URL:-https://github.com/XTLS/Xray-core/releases/latest/download/Xray-linux-64.zip}"
SINGBOX_RELEASE_URL="${SINGBOX_RELEASE_URL:-https://github.com/hiddify/hiddify-core/releases/download/v4.0.4/hiddify-core-linux-amd64.tar.gz}"
SINGBOX_RELEASE_FALLBACK_URL="${SINGBOX_RELEASE_FALLBACK_URL:-https://github.com/hiddify/hiddify-core/releases/latest/download/hiddify-core-linux-amd64.tar.gz}"
BUNDLE_VERSION_LABEL="${BUNDLE_VERSION_LABEL:-}"
ANNEAL_VERSION=""
BUNDLE_NAME=""
BUNDLE_ROOT=""
BUNDLE_ARCHIVE=""

require_tool() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required tool: $1" >&2
    exit 1
  }
}

workspace_version() {
  awk '
    $0 == "[workspace.package]" { in_section = 1; next }
    /^\[/ { in_section = 0 }
    in_section && $1 == "version" {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' "${ROOT_DIR}/Cargo.toml"
}

web_version() {
  (
    cd "${ROOT_DIR}"
    node -p "JSON.parse(require('fs').readFileSync('./web/package.json', 'utf8')).version"
  )
}

load_version() {
  local cargo_version web_package_version
  cargo_version="$(workspace_version)"
  web_package_version="$(web_version)"
  if [[ -z "${cargo_version}" ]]; then
    echo "workspace version is not set in Cargo.toml" >&2
    exit 1
  fi
  if [[ "${cargo_version}" != "${web_package_version}" ]]; then
    echo "workspace version ${cargo_version} does not match web version ${web_package_version}" >&2
    exit 1
  fi
  ANNEAL_VERSION="${cargo_version}"
  if [[ -z "${BUNDLE_VERSION_LABEL}" ]]; then
    BUNDLE_VERSION_LABEL="${ANNEAL_VERSION}"
  fi
  BUNDLE_NAME="anneal-${BUNDLE_VERSION_LABEL}-${TARGET_TRIPLE}"
  BUNDLE_ROOT="${TMP_DIR}/${BUNDLE_NAME}"
  BUNDLE_ARCHIVE="${DIST_DIR}/${BUNDLE_NAME}.tar.gz"
}

download_asset() {
  local url="$1"
  local destination="$2"
  curl \
    --retry 5 \
    --retry-all-errors \
    --location \
    --silent \
    --show-error \
    --user-agent "Anneal-Packager/1.0" \
    "${url}" \
    -o "${destination}"
}

validate_zip() {
  local archive="$1"
  unzip -tq "${archive}" >/dev/null
}

validate_tar_gz() {
  local archive="$1"
  tar -tzf "${archive}" >/dev/null
}

download_validated_asset() {
  local primary_url="$1"
  local fallback_url="$2"
  local destination="$3"
  local validator="$4"

  rm -f "${destination}"
  download_asset "${primary_url}" "${destination}"
  if "${validator}" "${destination}"; then
    return
  fi

  if [[ -n "${fallback_url}" ]]; then
    echo "primary asset download is invalid, retrying with fallback URL: ${primary_url}" >&2
    rm -f "${destination}"
    download_asset "${fallback_url}" "${destination}"
    "${validator}" "${destination}" || {
      echo "fallback asset download is invalid: ${fallback_url}" >&2
      exit 1
    }
    return
  fi

  echo "downloaded asset is invalid: ${primary_url}" >&2
  exit 1
}

prepare_workspace() {
  rm -rf "${DIST_DIR}"
  mkdir -p \
    "${TMP_DIR}" \
    "${BUNDLE_ROOT}/bin" \
    "${BUNDLE_ROOT}/deploy" \
    "${BUNDLE_ROOT}/migrations" \
    "${BUNDLE_ROOT}/runtime" \
    "${BUNDLE_ROOT}/web"
}

build_backend() {
  cargo build --locked --release -p api -p worker -p node-agent
}

build_web() {
  (
    cd "${ROOT_DIR}/web"
    npm ci
    npm run build
  )
}

package_backend() {
  install -m 0755 "${ROOT_DIR}/target/release/api" "${BUNDLE_ROOT}/bin/api"
  install -m 0755 "${ROOT_DIR}/target/release/worker" "${BUNDLE_ROOT}/bin/worker"
  install -m 0755 "${ROOT_DIR}/target/release/node-agent" "${BUNDLE_ROOT}/bin/node-agent"
  cp -a "${ROOT_DIR}/migrations"/. "${BUNDLE_ROOT}/migrations/"
}

package_web() {
  cp -a "${ROOT_DIR}/web/dist"/. "${BUNDLE_ROOT}/web/"
}

package_runtime_bundle() {
  local xray_dir="${TMP_DIR}/runtime-xray"
  local singbox_dir="${TMP_DIR}/runtime-singbox"

  rm -rf "${xray_dir}" "${singbox_dir}"
  mkdir -p "${xray_dir}" "${singbox_dir}"

  download_validated_asset \
    "${XRAY_RELEASE_URL}" \
    "${XRAY_RELEASE_FALLBACK_URL}" \
    "${TMP_DIR}/xray-runtime.zip" \
    validate_zip
  unzip -oq "${TMP_DIR}/xray-runtime.zip" -d "${xray_dir}"

  download_validated_asset \
    "${SINGBOX_RELEASE_URL}" \
    "${SINGBOX_RELEASE_FALLBACK_URL}" \
    "${TMP_DIR}/singbox-runtime.tar.gz" \
    validate_tar_gz
  tar -xzf "${TMP_DIR}/singbox-runtime.tar.gz" -C "${singbox_dir}"

  install -m 0755 "${xray_dir}/xray" "${BUNDLE_ROOT}/runtime/xray"
  install -m 0755 "$(find "${singbox_dir}" -type f -name 'hiddify-core' | head -n 1)" "${BUNDLE_ROOT}/runtime/hiddify-core"
}

package_deploy_bundle() {
  cp -a "${ROOT_DIR}/deploy"/. "${BUNDLE_ROOT}/deploy/"
  install -m 0755 "${ROOT_DIR}/scripts/install.sh" "${BUNDLE_ROOT}/install.sh"
}

write_release_manifest() {
  cat >"${BUNDLE_ROOT}/release-manifest.json" <<EOF
{
  "version": "${ANNEAL_VERSION}",
  "target": "${TARGET_TRIPLE}",
  "bundle": "${BUNDLE_NAME}.tar.gz",
  "paths": {
    "api": "bin/api",
    "worker": "bin/worker",
    "node_agent": "bin/node-agent",
    "xray": "runtime/xray",
    "singbox": "runtime/hiddify-core",
    "web": "web",
    "migrations": "migrations",
    "deploy": "deploy",
    "installer": "install.sh"
  }
}
EOF
}

write_checksums() {
  (
    cd "${BUNDLE_ROOT}"
    local checksums_file
    checksums_file="$(mktemp)"
    find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > "${checksums_file}"
    mv "${checksums_file}" SHA256SUMS
  )
}

bundle_release() {
  tar -czf "${BUNDLE_ARCHIVE}" -C "${TMP_DIR}" "${BUNDLE_NAME}"
}

main() {
  cd "${ROOT_DIR}"

  require_tool cargo
  require_tool npm
  require_tool node
  require_tool curl
  require_tool tar
  require_tool unzip
  require_tool sha256sum
  require_tool find
  require_tool sort
  require_tool xargs

  load_version
  prepare_workspace
  build_backend
  build_web
  package_backend
  package_web
  package_runtime_bundle
  package_deploy_bundle
  write_release_manifest
  write_checksums
  bundle_release
}

main "$@"
