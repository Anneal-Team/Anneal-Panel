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

require_tool() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required tool: $1" >&2
    exit 1
  }
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
    "${TMP_DIR}/api" \
    "${TMP_DIR}/worker" \
    "${TMP_DIR}/node-agent" \
    "${TMP_DIR}/migrations" \
    "${TMP_DIR}/runtime-bundle" \
    "${TMP_DIR}/deploy-bundle"
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
  cp "${ROOT_DIR}/target/release/api" "${TMP_DIR}/api/api"
  cp "${ROOT_DIR}/target/release/worker" "${TMP_DIR}/worker/worker"
  cp "${ROOT_DIR}/target/release/node-agent" "${TMP_DIR}/node-agent/node-agent"
  cp -a "${ROOT_DIR}/migrations"/. "${TMP_DIR}/migrations/"

  tar -czf "${DIST_DIR}/api-${TARGET_TRIPLE}.tar.gz" -C "${TMP_DIR}/api" api
  tar -czf "${DIST_DIR}/worker-${TARGET_TRIPLE}.tar.gz" -C "${TMP_DIR}/worker" worker
  tar -czf "${DIST_DIR}/node-agent-${TARGET_TRIPLE}.tar.gz" -C "${TMP_DIR}/node-agent" node-agent
  tar -czf "${DIST_DIR}/migrations.tar.gz" -C "${TMP_DIR}/migrations" .
}

package_web() {
  tar -czf "${DIST_DIR}/web.tar.gz" -C "${ROOT_DIR}/web/dist" .
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

  install -m 0755 "${xray_dir}/xray" "${TMP_DIR}/runtime-bundle/xray"
  install -m 0755 "$(find "${singbox_dir}" -type f -name 'hiddify-core' | head -n 1)" "${TMP_DIR}/runtime-bundle/hiddify-core"

  tar -czf "${DIST_DIR}/runtime-bundle-${TARGET_TRIPLE}.tar.gz" -C "${TMP_DIR}/runtime-bundle" .
}

package_deploy_bundle() {
  mkdir -p "${TMP_DIR}/deploy-bundle/deploy"
  cp -a "${ROOT_DIR}/deploy"/. "${TMP_DIR}/deploy-bundle/deploy/"
  install -m 0755 "${ROOT_DIR}/scripts/install.sh" "${TMP_DIR}/deploy-bundle/install.sh"

  tar -czf "${DIST_DIR}/deploy-bundle.tar.gz" -C "${TMP_DIR}/deploy-bundle" .
}

write_release_manifest() {
  cat >"${DIST_DIR}/release-manifest.json" <<EOF
{
  "target": "${TARGET_TRIPLE}",
  "assets": [
    "api-${TARGET_TRIPLE}.tar.gz",
    "worker-${TARGET_TRIPLE}.tar.gz",
    "node-agent-${TARGET_TRIPLE}.tar.gz",
    "migrations.tar.gz",
    "web.tar.gz",
    "runtime-bundle-${TARGET_TRIPLE}.tar.gz",
    "deploy-bundle.tar.gz"
  ]
}
EOF
}

write_checksums() {
  (
    cd "${DIST_DIR}"
    sha256sum \
      "api-${TARGET_TRIPLE}.tar.gz" \
      "worker-${TARGET_TRIPLE}.tar.gz" \
      "node-agent-${TARGET_TRIPLE}.tar.gz" \
      "migrations.tar.gz" \
      "web.tar.gz" \
      "runtime-bundle-${TARGET_TRIPLE}.tar.gz" \
      "deploy-bundle.tar.gz" \
      "release-manifest.json" > SHA256SUMS
  )
}

main() {
  cd "${ROOT_DIR}"

  require_tool cargo
  require_tool npm
  require_tool curl
  require_tool tar
  require_tool unzip
  require_tool sha256sum
  require_tool find

  prepare_workspace
  build_backend
  build_web
  package_backend
  package_web
  package_runtime_bundle
  package_deploy_bundle
  write_release_manifest
  write_checksums
}

main "$@"
