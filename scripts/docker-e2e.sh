#!/usr/bin/env bash
set -euo pipefail

API_URL="${E2E_API_URL:-http://api:8080}"
WEB_URL="${E2E_WEB_URL:-http://web}"
SHARED_DIR="${E2E_SHARED_DIR:-/e2e}"
SUPERADMIN_EMAIL="${E2E_SUPERADMIN_EMAIL:-admin-e2e@anneal.test}"
SUPERADMIN_DISPLAY_NAME="${E2E_SUPERADMIN_DISPLAY_NAME:-Anneal Admin}"
SUPERADMIN_PASSWORD="${E2E_SUPERADMIN_PASSWORD:-AnnealPass_123!}"
TENANT_NAME="${E2E_TENANT_NAME:-tenant-e2e}"
LEGACY_TENANT_PREFIX="${E2E_LEGACY_TENANT_PREFIX:-tenant-e2e-}"
RESELLER_EMAIL="${E2E_RESELLER_EMAIL:-reseller-e2e@anneal.test}"
RESELLER_DISPLAY_NAME="${E2E_RESELLER_DISPLAY_NAME:-E2E Reseller}"
RESELLER_PASSWORD="${E2E_RESELLER_PASSWORD:-ResellerPass_123}"
USER_EMAIL="${E2E_USER_EMAIL:-user-e2e@anneal.test}"
USER_DISPLAY_NAME="${E2E_USER_DISPLAY_NAME:-E2E User}"
USER_PASSWORD="${E2E_USER_PASSWORD:-UserPass_123}"
NODE_GROUP_NAME="${E2E_NODE_GROUP_NAME:-edge-main}"
XRAY_NODE_NAME="${E2E_XRAY_NODE_NAME:-node-e2e-xray}"
SINGBOX_NODE_NAME="${E2E_SINGBOX_NODE_NAME:-node-e2e-singbox}"
SUBSCRIPTION_NAME="${E2E_SUBSCRIPTION_NAME:-bundle-main}"
QUOTA_BYTES="${E2E_QUOTA_BYTES:-1048576}"
BOOTSTRAP_TOKEN="${E2E_BOOTSTRAP_TOKEN:-test-bootstrap-token}"
DATA_ENCRYPTION_KEY="${E2E_DATA_ENCRYPTION_KEY:-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef}"

mkdir -p "${SHARED_DIR}"

wait_for_url() {
  local url="$1"
  for _ in $(seq 1 120); do
    if curl -fsS "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "timed out waiting for ${url}" >&2
  exit 1
}

totp_code() {
  python3 - "$1" <<'PY'
import base64
import hashlib
import hmac
import struct
import sys
import time

secret = sys.argv[1].strip().upper().replace(" ", "")
padding = "=" * ((8 - len(secret) % 8) % 8)
key = base64.b32decode(secret + padding, casefold=True)
counter = int(time.time()) // 30
message = struct.pack(">Q", counter)
digest = hmac.new(key, message, hashlib.sha1).digest()
offset = digest[-1] & 0x0F
code = (struct.unpack(">I", digest[offset:offset + 4])[0] & 0x7FFFFFFF) % 1000000
print(f"{code:06d}")
PY
}

api_post_public() {
  local path="$1"
  local payload="$2"
  local response_file
  response_file="$(mktemp)"
  local status
  status="$(
    curl -sS -o "${response_file}" -w '%{http_code}' \
      "${API_URL}${path}" \
      -H 'content-type: application/json' \
      --data "${payload}"
  )"
  if [[ "${status}" != 2* ]]; then
    echo "request failed: POST ${path} -> ${status}" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    exit 1
  fi
  cat "${response_file}"
  rm -f "${response_file}"
}

api_post_auth() {
  local path="$1"
  local payload="$2"
  local response_file
  response_file="$(mktemp)"
  local status
  status="$(
    curl -sS -o "${response_file}" -w '%{http_code}' \
      "${API_URL}${path}" \
      -H 'content-type: application/json' \
      -H "authorization: Bearer ${ACCESS_TOKEN}" \
      --data "${payload}"
  )"
  if [[ "${status}" != 2* ]]; then
    echo "request failed: POST ${path} -> ${status}" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    exit 1
  fi
  cat "${response_file}"
  rm -f "${response_file}"
}

api_post_auth_empty() {
  local path="$1"
  local response_file
  response_file="$(mktemp)"
  local status
  status="$(
    curl -sS -o "${response_file}" -w '%{http_code}' \
      "${API_URL}${path}" \
      -X POST \
      -H "authorization: Bearer ${ACCESS_TOKEN}"
  )"
  if [[ "${status}" != 2* ]]; then
    echo "request failed: POST ${path} -> ${status}" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    exit 1
  fi
  cat "${response_file}"
  rm -f "${response_file}"
}

api_delete_auth() {
  local path="$1"
  local response_file
  response_file="$(mktemp)"
  local status
  status="$(
    curl -sS -o "${response_file}" -w '%{http_code}' \
      "${API_URL}${path}" \
      -X DELETE \
      -H "authorization: Bearer ${ACCESS_TOKEN}"
  )"
  if [[ "${status}" != 2* ]]; then
    echo "request failed: DELETE ${path} -> ${status}" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    exit 1
  fi
  cat "${response_file}"
  rm -f "${response_file}"
}

api_get_auth() {
  local path="$1"
  local response_file
  response_file="$(mktemp)"
  local status
  status="$(
    curl -sS -o "${response_file}" -w '%{http_code}' \
      "${API_URL}${path}" \
      -H "authorization: Bearer ${ACCESS_TOKEN}"
  )"
  if [[ "${status}" != 2* ]]; then
    echo "request failed: GET ${path} -> ${status}" >&2
    cat "${response_file}" >&2
    rm -f "${response_file}"
    exit 1
  fi
  cat "${response_file}"
  rm -f "${response_file}"
}

wait_for_node() {
  local name="$1"
  for _ in $(seq 1 120); do
    local nodes
    nodes="$(api_get_auth "/api/v1/nodes")"
    if echo "${nodes}" | jq -e --arg name "${name}" '.[] | select(.name == $name and .status == "online")' >/dev/null; then
      return 0
    fi
    sleep 2
  done
  echo "node ${name} did not become online" >&2
  exit 1
}

wait_for_subscription_suspended() {
  local subscription_id="$1"
  for _ in $(seq 1 120); do
    local subscriptions
    subscriptions="$(api_get_auth "/api/v1/subscriptions")"
    if echo "${subscriptions}" | jq -e --arg id "${subscription_id}" '.[] | select(.id == $id and .suspended == true)' >/dev/null; then
      return 0
    fi
    sleep 2
  done
  echo "subscription ${subscription_id} was not suspended" >&2
  exit 1
}

cleanup_legacy_e2e_resellers() {
  local resellers
  resellers="$(api_get_auth "/api/v1/resellers")"
  echo "${resellers}" | jq -r --arg legacy_prefix "${LEGACY_TENANT_PREFIX}" '
    .[]
    | select(((.tenant_name // "") | startswith($legacy_prefix)))
    | .id
  ' | while IFS= read -r reseller_id; do
    [[ -z "${reseller_id}" ]] && continue
    api_delete_auth "/api/v1/resellers/${reseller_id}" >/dev/null
  done
}

echo "waiting for services"
wait_for_url "${API_URL}/api/v1/health"
wait_for_url "${WEB_URL}/"

echo "bootstrapping superadmin"
BOOTSTRAP_STATUS="$(
  curl -sS -o /tmp/bootstrap.json -w '%{http_code}' \
    "${API_URL}/api/v1/bootstrap" \
    -H 'content-type: application/json' \
    -H "x-bootstrap-token: ${BOOTSTRAP_TOKEN}" \
    --data "$(jq -nc --arg email "${SUPERADMIN_EMAIL}" --arg display_name "${SUPERADMIN_DISPLAY_NAME}" --arg password "${SUPERADMIN_PASSWORD}" '{email:$email, display_name:$display_name, password:$password}')"
)"
if [[ "${BOOTSTRAP_STATUS}" != "200" && "${BOOTSTRAP_STATUS}" != "409" ]]; then
  echo "bootstrap failed with status ${BOOTSTRAP_STATUS}" >&2
  cat /tmp/bootstrap.json >&2
  exit 1
fi

echo "starting auth flow"
LOGIN_RESPONSE="$(api_post_public "/api/v1/auth/login" "$(jq -nc --arg email "${SUPERADMIN_EMAIL}" --arg password "${SUPERADMIN_PASSWORD}" '{email:$email, password:$password}')" )"
PRE_AUTH_TOKEN="$(echo "${LOGIN_RESPONSE}" | jq -r '.pre_auth_token')"
TOTP_SETUP="$(curl -fsS "${API_URL}/api/v1/auth/totp/setup" -X POST -H "authorization: Bearer ${PRE_AUTH_TOKEN}")"
TOTP_SECRET="$(echo "${TOTP_SETUP}" | jq -r '.secret')"
TOTP_CODE="$(totp_code "${TOTP_SECRET}")"
TOKENS="$(curl -fsS "${API_URL}/api/v1/auth/totp/verify" -H 'content-type: application/json' -H "authorization: Bearer ${PRE_AUTH_TOKEN}" --data "$(jq -nc --arg code "${TOTP_CODE}" '{code:$code}')")"
ACCESS_TOKEN="$(echo "${TOKENS}" | jq -r '.access_token')"

echo "cleaning legacy e2e data"
cleanup_legacy_e2e_resellers

echo "ensuring reseller, user and subscription"
RESELLERS="$(api_get_auth "/api/v1/resellers")"
RESELLER="$(echo "${RESELLERS}" | jq -c --arg email "${RESELLER_EMAIL}" '.[] | select(.email == $email)' | head -n 1)"
if [[ -z "${RESELLER}" ]]; then
  RESELLER="$(api_post_auth "/api/v1/resellers" "$(jq -nc --arg tenant_name "${TENANT_NAME}" --arg email "${RESELLER_EMAIL}" --arg display_name "${RESELLER_DISPLAY_NAME}" --arg password "${RESELLER_PASSWORD}" '{tenant_name:$tenant_name, email:$email, display_name:$display_name, password:$password}')" )"
fi
TENANT_ID="$(echo "${RESELLER}" | jq -r '.tenant_id')"

USERS="$(api_get_auth "/api/v1/users")"
USER="$(echo "${USERS}" | jq -c --arg email "${USER_EMAIL}" --arg tenant_id "${TENANT_ID}" '.[] | select(.email == $email and .tenant_id == $tenant_id)' | head -n 1)"
if [[ -z "${USER}" ]]; then
  USER="$(api_post_auth "/api/v1/users" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg email "${USER_EMAIL}" --arg display_name "${USER_DISPLAY_NAME}" --arg role "user" --arg password "${USER_PASSWORD}" '{target_tenant_id:$tenant_id, email:$email, display_name:$display_name, role:$role, password:$password}')" )"
fi
USER_ID="$(echo "${USER}" | jq -r '.id')"

SUBSCRIPTIONS="$(api_get_auth "/api/v1/subscriptions")"
SUBSCRIPTION="$(echo "${SUBSCRIPTIONS}" | jq -c --arg tenant_id "${TENANT_ID}" --arg user_id "${USER_ID}" --arg name "${SUBSCRIPTION_NAME}" '.[] | select(.tenant_id == $tenant_id and .user_id == $user_id and .name == $name)' | head -n 1)"
if [[ -n "${SUBSCRIPTION}" ]]; then
  api_delete_auth "/api/v1/subscriptions/$(echo "${SUBSCRIPTION}" | jq -r '.id')?tenant_id=${TENANT_ID}" >/dev/null
fi
EXPIRES_AT="$(date -u -d '+30 days' +"%Y-%m-%dT%H:%M:%SZ")"
SUBSCRIPTION_RESPONSE="$(api_post_auth "/api/v1/subscriptions" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg user_id "${USER_ID}" --arg name "${SUBSCRIPTION_NAME}" --arg expires_at "${EXPIRES_AT}" --argjson traffic_limit_bytes "${QUOTA_BYTES}" '{tenant_id:$tenant_id, user_id:$user_id, name:$name, note:"docker-e2e", traffic_limit_bytes:$traffic_limit_bytes, expires_at:$expires_at}')" )"
SUBSCRIPTION="$(echo "${SUBSCRIPTION_RESPONSE}" | jq -c '.subscription')"
DELIVERY_URL="$(echo "${SUBSCRIPTION_RESPONSE}" | jq -r '.delivery_url')"
SUBSCRIPTION_ID="$(echo "${SUBSCRIPTION}" | jq -r '.id')"
DEVICE_ID="$(echo "${SUBSCRIPTION}" | jq -r '.device_id')"

echo "ensuring node group and runtimes"
NODE_GROUPS="$(api_get_auth "/api/v1/node-groups")"
NODE_GROUP="$(echo "${NODE_GROUPS}" | jq -c --arg tenant_id "${TENANT_ID}" --arg name "${NODE_GROUP_NAME}" '.[] | select(.tenant_id == $tenant_id and .name == $name)' | head -n 1)"
if [[ -z "${NODE_GROUP}" ]]; then
  NODE_GROUP="$(api_post_auth "/api/v1/node-groups" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg name "${NODE_GROUP_NAME}" '{tenant_id:$tenant_id, name:$name}')" )"
fi
NODE_GROUP_ID="$(echo "${NODE_GROUP}" | jq -r '.id')"

NODES="$(api_get_auth "/api/v1/nodes")"
XRAY_NODE="$(echo "${NODES}" | jq -c --arg tenant_id "${TENANT_ID}" --arg node_group_id "${NODE_GROUP_ID}" --arg name "${XRAY_NODE_NAME}" '.[] | select(.tenant_id == $tenant_id and .node_group_id == $node_group_id and .name == $name and .engine == "xray")' | head -n 1)"
if [[ -z "${XRAY_NODE}" ]]; then
  XRAY_GRANT="$(api_post_auth "/api/v1/nodes/enrollment-tokens" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg node_group_id "${NODE_GROUP_ID}" --arg engine "xray" '{tenant_id:$tenant_id, node_group_id:$node_group_id, engine:$engine}')" )"
  XRAY_TOKEN="$(echo "${XRAY_GRANT}" | jq -r '.token')"
  XRAY_NODE="$(api_post_public "/api/v1/agent/register" "$(jq -nc --arg enrollment_token "${XRAY_TOKEN}" --arg name "${XRAY_NODE_NAME}" --arg version "0.1.0" '{enrollment_token:$enrollment_token, name:$name, version:$version, engine:"xray", protocols:["vmess","trojan","shadowsocks_2022","vless_reality"]}')" )"
fi
XRAY_NODE_ID="$(echo "${XRAY_NODE}" | jq -r '.id')"

SINGBOX_NODE="$(echo "${NODES}" | jq -c --arg tenant_id "${TENANT_ID}" --arg node_group_id "${NODE_GROUP_ID}" --arg name "${SINGBOX_NODE_NAME}" '.[] | select(.tenant_id == $tenant_id and .node_group_id == $node_group_id and .name == $name and .engine == "singbox")' | head -n 1)"
if [[ -z "${SINGBOX_NODE}" ]]; then
  SINGBOX_GRANT="$(api_post_auth "/api/v1/nodes/enrollment-tokens" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg node_group_id "${NODE_GROUP_ID}" --arg engine "singbox" '{tenant_id:$tenant_id, node_group_id:$node_group_id, engine:$engine}')" )"
  SINGBOX_TOKEN="$(echo "${SINGBOX_GRANT}" | jq -r '.token')"
  SINGBOX_NODE="$(api_post_public "/api/v1/agent/register" "$(jq -nc --arg enrollment_token "${SINGBOX_TOKEN}" --arg name "${SINGBOX_NODE_NAME}" --arg version "0.1.0" '{enrollment_token:$enrollment_token, name:$name, version:$version, engine:"singbox", protocols:["vmess","trojan","shadowsocks_2022","tuic","hysteria2","vless_reality"]}')" )"
fi
SINGBOX_NODE_ID="$(echo "${SINGBOX_NODE}" | jq -r '.id')"

echo "${XRAY_NODE_ID}" >"${SHARED_DIR}/xray.node_id"
echo "${SINGBOX_NODE_ID}" >"${SHARED_DIR}/singbox.node_id"

echo "waiting for agents to register"
wait_for_node "${XRAY_NODE_NAME}"
wait_for_node "${SINGBOX_NODE_NAME}"

echo "configuring generated domain rules"
api_post_auth "/api/v1/node-groups/${NODE_GROUP_ID}/domains" "$(jq -nc --arg tenant_id "${TENANT_ID}" '{
  tenant_id:$tenant_id,
  domains:[
    {
      mode:"direct",
      domain:"edge.example.com",
      alias:"main",
      server_names:[],
      host_headers:[]
    },
    {
      mode:"worker",
      domain:"worker.example.com",
      alias:"worker",
      server_names:["worker.example.com"],
      host_headers:["worker.example.com","cdn.worker.example.com"]
    },
    {
      mode:"reality",
      domain:"reality.example.com",
      alias:"reality",
      server_names:["www.cloudflare.com","www.apple.com"],
      host_headers:[]
    }
  ]
}')" >/tmp/domains.json

XRAY_ENDPOINTS="$(api_get_auth "/api/v1/nodes/${XRAY_NODE_ID}/endpoints?tenant_id=${TENANT_ID}")"
SINGBOX_ENDPOINTS="$(api_get_auth "/api/v1/nodes/${SINGBOX_NODE_ID}/endpoints?tenant_id=${TENANT_ID}")"
echo "${XRAY_ENDPOINTS}" | jq -e 'map(.protocol) | index("trojan")' >/dev/null
echo "${XRAY_ENDPOINTS}" | jq -e 'map(.protocol) | index("vmess")' >/dev/null
echo "${XRAY_ENDPOINTS}" | jq -e 'map(select(.security == "reality")) | length >= 2' >/dev/null
echo "${SINGBOX_ENDPOINTS}" | jq -e 'map(.protocol) | index("tuic")' >/dev/null
echo "${SINGBOX_ENDPOINTS}" | jq -e 'map(.protocol) | index("hysteria2")' >/dev/null

echo "verifying subscription delivery"
curl -fsS "${DELIVERY_URL}?format=raw" >/tmp/subscription.raw.txt
curl -fsS "${DELIVERY_URL}" >/tmp/subscription.base64.txt
curl -fsS -H 'User-Agent: Clash.Meta' "${DELIVERY_URL}" >/tmp/subscription.clash.yaml
curl -fsS -H 'User-Agent: sing-box' "${DELIVERY_URL}" >/tmp/subscription.singbox.json
curl -fsS -H 'User-Agent: Hiddify' "${DELIVERY_URL}" >/tmp/subscription.hiddify.json
grep -q 'vmess://' /tmp/subscription.raw.txt
grep -q 'trojan://' /tmp/subscription.raw.txt
grep -q 'ss://' /tmp/subscription.raw.txt
grep -q 'tuic://' /tmp/subscription.raw.txt
grep -q 'hysteria2://' /tmp/subscription.raw.txt
grep -q 'type: trojan' /tmp/subscription.clash.yaml
grep -q 'type: tuic' /tmp/subscription.clash.yaml
grep -q 'type: hysteria2' /tmp/subscription.clash.yaml
jq -e '.outbounds | map(.type) | index("trojan")' /tmp/subscription.singbox.json >/dev/null
jq -e '.outbounds | map(.type) | index("tuic")' /tmp/subscription.singbox.json >/dev/null
jq -e '.outbounds | map(.type) | index("hysteria2")' /tmp/subscription.singbox.json >/dev/null
jq -e '.profiles | map(.protocol) | index("trojan")' /tmp/subscription.hiddify.json >/dev/null
jq -e '.profiles | map(.protocol) | index("tuic")' /tmp/subscription.hiddify.json >/dev/null
jq -e '.profiles | map(.protocol) | index("hysteria2")' /tmp/subscription.hiddify.json >/dev/null

echo "sending usage batch to trigger hard stop"
api_post_public "/api/v1/agent/usage/bulk" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg node_id "${XRAY_NODE_ID}" --arg subscription_id "${SUBSCRIPTION_ID}" --arg device_id "${DEVICE_ID}" --arg measured_at "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" '{
  samples:[
    {
      tenant_id:$tenant_id,
      node_id:$node_id,
      subscription_id:$subscription_id,
      device_id:$device_id,
      bytes_in:700000,
      bytes_out:700000,
      measured_at:$measured_at
    }
  ]
}')" >/tmp/usage.json
wait_for_subscription_suspended "${SUBSCRIPTION_ID}"

STATUS_CODE="$(curl -s -o /tmp/subscription.forbidden.txt -w '%{http_code}' "${DELIVERY_URL}")"
if [[ "${STATUS_CODE}" != "403" ]]; then
  echo "expected delivery endpoint to return 403 after suspend, got ${STATUS_CODE}" >&2
  exit 1
fi

echo "docker e2e completed successfully"
