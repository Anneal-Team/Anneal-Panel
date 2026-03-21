#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${ANNEAL_SMOKE_BASE_URL:-http://127.0.0.1}"
API_URL="${ANNEAL_SMOKE_API_URL:-${BASE_URL}/api/v1}"
SUPERADMIN_EMAIL="${ANNEAL_SMOKE_SUPERADMIN_EMAIL:?set ANNEAL_SMOKE_SUPERADMIN_EMAIL}"
SUPERADMIN_PASSWORD="${ANNEAL_SMOKE_SUPERADMIN_PASSWORD:?set ANNEAL_SMOKE_SUPERADMIN_PASSWORD}"
X_RAY_NODE_NAME="${ANNEAL_SMOKE_XRAY_NODE_NAME:-}"
SINGBOX_NODE_NAME="${ANNEAL_SMOKE_SINGBOX_NODE_NAME:-}"

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

api_post() {
  local path="$1"
  local payload="$2"
  curl -fsS "${API_URL}${path}" -H 'content-type: application/json' --data "${payload}"
}

api_post_auth() {
  local path="$1"
  local payload="$2"
  curl -fsS "${API_URL}${path}" -H 'content-type: application/json' -H "authorization: Bearer ${ACCESS_TOKEN}" --data "${payload}"
}

api_get_auth() {
  local path="$1"
  curl -fsS "${API_URL}${path}" -H "authorization: Bearer ${ACCESS_TOKEN}"
}

curl -fsS "${API_URL}/health" >/dev/null
LOGIN_RESPONSE="$(api_post "/auth/login" "$(jq -nc --arg email "${SUPERADMIN_EMAIL}" --arg password "${SUPERADMIN_PASSWORD}" '{email:$email, password:$password}')" )"
STATUS="$(echo "${LOGIN_RESPONSE}" | jq -r '.status')"
if [[ "${STATUS}" == "authenticated" ]]; then
  ACCESS_TOKEN="$(echo "${LOGIN_RESPONSE}" | jq -r '.tokens.access_token')"
else
  PRE_AUTH_TOKEN="$(echo "${LOGIN_RESPONSE}" | jq -r '.pre_auth_token')"
  TOTP_SETUP="$(curl -fsS "${API_URL}/auth/totp/setup" -X POST -H "authorization: Bearer ${PRE_AUTH_TOKEN}")"
  TOTP_SECRET="$(echo "${TOTP_SETUP}" | jq -r '.secret')"
  TOTP_CODE="$(totp_code "${TOTP_SECRET}")"
  TOKENS="$(curl -fsS "${API_URL}/auth/totp/verify" -H 'content-type: application/json' -H "authorization: Bearer ${PRE_AUTH_TOKEN}" --data "$(jq -nc --arg code "${TOTP_CODE}" '{code:$code}')")"
  ACCESS_TOKEN="$(echo "${TOKENS}" | jq -r '.access_token')"
fi

RESELLER="$(api_post_auth "/resellers" "$(jq -nc --arg tenant_name "native-smoke" --arg email "reseller-native@anneal.test" --arg display_name "Native Reseller" --arg password "ResellerPass_123" '{tenant_name:$tenant_name, email:$email, display_name:$display_name, password:$password}')" )"
TENANT_ID="$(echo "${RESELLER}" | jq -r '.tenant_id')"
USER="$(api_post_auth "/users" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg email "user-native@anneal.test" --arg display_name "Native User" --arg role "user" --arg password "UserPass_123" '{target_tenant_id:$tenant_id, email:$email, display_name:$display_name, role:$role, password:$password}')" )"
USER_ID="$(echo "${USER}" | jq -r '.id')"
EXPIRES_AT="$(date -u -d '+30 days' +"%Y-%m-%dT%H:%M:%SZ")"
SUBSCRIPTION="$(api_post_auth "/subscriptions" "$(jq -nc --arg tenant_id "${TENANT_ID}" --arg user_id "${USER_ID}" --arg name "native-bundle" --arg expires_at "${EXPIRES_AT}" '{tenant_id:$tenant_id, user_id:$user_id, name:$name, note:"native-smoke", traffic_limit_bytes:1048576, expires_at:$expires_at}')" )"
DELIVERY_URL="$(echo "${SUBSCRIPTION}" | jq -r '.delivery_url')"

echo "native smoke created subscription: ${DELIVERY_URL}"
curl -fsS "${DELIVERY_URL}" >/dev/null

if [[ -n "${X_RAY_NODE_NAME}" || -n "${SINGBOX_NODE_NAME}" ]]; then
  NODES="$(api_get_auth "/nodes")"
  if [[ -n "${X_RAY_NODE_NAME}" ]]; then
    echo "${NODES}" | jq -e --arg name "${X_RAY_NODE_NAME}" '.[] | select(.name == $name and .status == "online")' >/dev/null
  fi
  if [[ -n "${SINGBOX_NODE_NAME}" ]]; then
    echo "${NODES}" | jq -e --arg name "${SINGBOX_NODE_NAME}" '.[] | select(.name == $name and .status == "online")' >/dev/null
  fi
fi

echo "native smoke completed"
