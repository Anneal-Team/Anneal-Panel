#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
TMP_DIR="${DIST_DIR}/tmp"
TARGET_TRIPLE="${TARGET_TRIPLE:-linux-amd64}"

cd "${ROOT_DIR}"

cargo build --release -p api -p worker -p node-agent

cd "${ROOT_DIR}/web"
npm run build
cd "${ROOT_DIR}"

rm -rf "${TMP_DIR}"
mkdir -p "${TMP_DIR}/api" "${TMP_DIR}/worker" "${TMP_DIR}/node-agent" "${TMP_DIR}/migrations" "${DIST_DIR}"

cp "${ROOT_DIR}/target/release/api" "${TMP_DIR}/api/api"
cp "${ROOT_DIR}/target/release/worker" "${TMP_DIR}/worker/worker"
cp "${ROOT_DIR}/target/release/node-agent" "${TMP_DIR}/node-agent/node-agent"
cp -a "${ROOT_DIR}/migrations"/. "${TMP_DIR}/migrations/"

tar -czf "${DIST_DIR}/api-${TARGET_TRIPLE}.tar.gz" -C "${TMP_DIR}/api" api
tar -czf "${DIST_DIR}/worker-${TARGET_TRIPLE}.tar.gz" -C "${TMP_DIR}/worker" worker
tar -czf "${DIST_DIR}/node-agent-${TARGET_TRIPLE}.tar.gz" -C "${TMP_DIR}/node-agent" node-agent
tar -czf "${DIST_DIR}/migrations.tar.gz" -C "${TMP_DIR}/migrations" .
tar -czf "${DIST_DIR}/web.tar.gz" -C "${ROOT_DIR}/web/dist" .
