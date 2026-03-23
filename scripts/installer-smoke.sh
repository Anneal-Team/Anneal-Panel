#!/usr/bin/env bash
set -euo pipefail

INSTALLER_PATH="${1:-scripts/install.sh}"

run_file_install_smoke() {
  local output
  output="$(
    ANNEAL_INSTALLER_LANG=en \
      ANNEAL_INSTALLER_UI=plain \
      bash "${INSTALLER_PATH}" --action status 2>&1 || true
  )"
  echo "${output}" | grep -q "Run the installer as root."
}

run_stdin_install_smoke() {
  local output
  output="$(
    env \
      ANNEAL_INSTALLER_LANG=en \
      ANNEAL_INSTALLER_UI=plain \
      bash -s -- --action status < "${INSTALLER_PATH}" 2>&1 || true
  )"
  echo "${output}" | grep -q "Run the installer as root."
}

run_file_install_smoke
run_stdin_install_smoke
