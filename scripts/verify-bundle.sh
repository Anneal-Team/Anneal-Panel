#!/usr/bin/env bash
set -euo pipefail

BUNDLE_ARCHIVE="${1:?bundle archive path is required}"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TEMP_DIR}"' EXIT

tar -xzf "${BUNDLE_ARCHIVE}" -C "${TEMP_DIR}"
shopt -s nullglob
entries=("${TEMP_DIR}"/*)
shopt -u nullglob

if [[ "${#entries[@]}" -ne 1 || ! -d "${entries[0]}" ]]; then
  echo "bundle archive must contain exactly one top-level directory" >&2
  exit 1
fi

BUNDLE_ROOT="${entries[0]}"

for required_path in \
  "${BUNDLE_ROOT}/bin/api" \
  "${BUNDLE_ROOT}/bin/worker" \
  "${BUNDLE_ROOT}/bin/node-agent" \
  "${BUNDLE_ROOT}/runtime/xray" \
  "${BUNDLE_ROOT}/runtime/hiddify-core" \
  "${BUNDLE_ROOT}/migrations" \
  "${BUNDLE_ROOT}/web" \
  "${BUNDLE_ROOT}/deploy/systemd/anneal-api.service" \
  "${BUNDLE_ROOT}/deploy/systemd/anneal-worker.service" \
  "${BUNDLE_ROOT}/deploy/systemd/anneal-caddy.service" \
  "${BUNDLE_ROOT}/deploy/systemd/anneal-node-agent.service" \
  "${BUNDLE_ROOT}/install.sh" \
  "${BUNDLE_ROOT}/release-manifest.json" \
  "${BUNDLE_ROOT}/SHA256SUMS"; do
  [[ -e "${required_path}" ]] || {
    echo "missing bundle path: ${required_path}" >&2
    exit 1
  }
done

(
  cd "${BUNDLE_ROOT}"
  sha256sum -c SHA256SUMS >/dev/null
)
