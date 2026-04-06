import i18n from "./i18n";
import { panelBasePath } from "./panel-base";

export type UserRole = "superadmin" | "admin" | "reseller" | "user";
export type UserStatus = "active" | "suspended";
export type ProxyEngine = "xray" | "singbox";
export type ProtocolKind =
  | "vless_reality"
  | "vmess"
  | "trojan"
  | "shadowsocks_2022"
  | "tuic"
  | "hysteria2";
export type DeploymentStatus =
  | "queued"
  | "rendering"
  | "validating"
  | "ready"
  | "applied"
  | "rolled_back"
  | "failed";
export type QuotaState = "normal" | "warning80" | "warning95" | "exhausted";
export type TransportKind = "tcp" | "ws" | "grpc" | "http_upgrade";
export type SecurityKind = "none" | "tls" | "reality";
export type NodeDomainMode =
  | "direct"
  | "legacy_direct"
  | "cdn"
  | "auto_cdn"
  | "relay"
  | "worker"
  | "reality"
  | "fake";

export interface SessionTokens {
  access_token: string;
  refresh_token: string;
  access_expires_at: string;
  refresh_expires_at: string;
}

export type LoginResult =
  | { status: "authenticated"; tokens: SessionTokens }
  | { status: "totp_required"; pre_auth_token: string }
  | { status: "totp_setup_required"; pre_auth_token: string };

export interface TotpSetup {
  secret: string;
  otpauth_url: string;
}

export interface User {
  id: string;
  tenant_id: string | null;
  tenant_name: string | null;
  email: string;
  display_name: string;
  role: UserRole;
  status: UserStatus;
  totp_confirmed: boolean;
  created_at: string;
  updated_at: string;
}

export interface Device {
  id: string;
  tenant_id: string;
  user_id: string;
  name: string;
  suspended: boolean;
  created_at: string;
  updated_at: string;
}

export interface Subscription {
  id: string;
  tenant_id: string;
  user_id: string;
  device_id: string;
  name: string;
  note: string | null;
  traffic_limit_bytes: number;
  used_bytes: number;
  quota_state: QuotaState;
  suspended: boolean;
  expires_at: string;
  created_at: string;
  updated_at: string;
  delivery_url: string | null;
}

export interface CreateSubscriptionResponse {
  subscription: Subscription;
  delivery_url: string;
}

export interface RotateSubscriptionLinkResponse {
  delivery_url: string;
}

export interface PublicSubscription {
  name: string;
  note: string | null;
  traffic_limit_bytes: number;
  used_bytes: number;
  quota_state: QuotaState;
  suspended: boolean;
  expires_at: string;
  delivery_url: string;
}

export interface NodeRuntime {
  id: string;
  tenant_id: string;
  node_id: string;
  engine: ProxyEngine;
  version: string;
  status: "pending" | "online" | "offline";
  last_seen_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Node {
  id: string;
  tenant_id: string;
  name: string;
  created_at: string;
  updated_at: string;
  runtimes: NodeRuntime[];
}

export interface NodeDomain {
  id: string;
  node_id: string;
  mode: NodeDomainMode;
  domain: string;
  alias: string | null;
  server_names: string[];
  host_headers: string[];
  created_at: string;
  updated_at: string;
}

export interface NodeEndpoint {
  id: string;
  node_runtime_id: string;
  protocol: ProtocolKind;
  listen_host: string;
  listen_port: number;
  public_host: string;
  public_port: number;
  transport: TransportKind;
  security: SecurityKind;
  server_name: string | null;
  host_header: string | null;
  path: string | null;
  service_name: string | null;
  flow: string | null;
  reality_public_key: string | null;
  reality_short_id: string | null;
  fingerprint: string | null;
  alpn: string[];
  cipher: string | null;
  tls_certificate_path: string | null;
  tls_key_path: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface NodeEndpointInput {
  protocol: ProtocolKind;
  listen_host: string;
  listen_port: number;
  public_host: string;
  public_port: number;
  transport: TransportKind;
  security: SecurityKind;
  server_name: string | null;
  host_header: string | null;
  path: string | null;
  service_name: string | null;
  flow: string | null;
  fingerprint: string | null;
  alpn: string[];
  cipher: string | null;
  tls_certificate_path: string | null;
  tls_key_path: string | null;
  enabled: boolean;
}

export interface EnrollmentGrant {
  token: string;
  record: {
    id: string;
    tenant_id: string;
    node_id: string;
    token_hash: string;
    engine: ProxyEngine;
    expires_at: string;
    created_at: string;
    used_at: string | null;
  };
}

export interface DeploymentRollout {
  id: string;
  tenant_id: string;
  node_runtime_id: string;
  config_revision_id: string;
  engine: ProxyEngine;
  revision_name: string;
  target_path: string;
  status: DeploymentStatus;
  failure_reason: string | null;
  created_at: string;
  updated_at: string;
  applied_at: string | null;
}

export interface NotificationEvent {
  id: string;
  tenant_id: string;
  kind: "quota80" | "quota95" | "quota100" | "node_offline";
  title: string;
  body: string;
  delivered_at: string | null;
  created_at: string;
}

export interface AuditLog {
  id: string;
  actor_user_id: string | null;
  tenant_id: string | null;
  action: string;
  resource_type: string;
  resource_id: string | null;
  payload: unknown;
  created_at: string;
}

export interface UsageOverview {
  subscription_id: string;
  tenant_id: string;
  subscription_name: string;
  device_id: string;
  device_name: string;
  traffic_limit_bytes: number;
  used_bytes: number;
  quota_state: QuotaState;
  suspended: boolean;
  updated_at: string;
}

export interface SessionState {
  accessToken: string | null;
  refreshToken: string | null;
  preAuthToken: string | null;
}

export interface AccessClaims {
  sub: string;
  role: UserRole;
  tenant_id: string | null;
  kind: string;
  challenge_id: string | null;
  purpose: string | null;
  exp: number;
  iat: number;
}

export interface RefreshSessionInfo {
  id: string;
  user_id: string;
  refresh_token_hash: string;
  user_agent: string | null;
  ip_address: string | null;
  expires_at: string;
  revoked_at: string | null;
  rotated_from_session_id: string | null;
  created_at: string;
}

const sessionStorageKey = "anneal.session";
let refreshPromise: Promise<SessionTokens> | null = null;

function normalizeApiPath(path: string) {
  if (!path.startsWith("/")) {
    throw new Error(`API path must start with "/": ${path}`);
  }
  if (path.startsWith("//") || /^[a-z][a-z\d+\-.]*:\/\//i.test(path)) {
    throw new Error(`Absolute URLs are not allowed in API paths: ${path}`);
  }
  return path;
}

function getBaseUrl() {
  const configuredBaseUrl = import.meta.env.VITE_API_BASE_URL as string | undefined;
  if (configuredBaseUrl) {
    return configuredBaseUrl;
  }
  return `${panelBasePath() || ""}/api/v1`;
}

function getSession(): SessionState {
  const raw = window.localStorage.getItem(sessionStorageKey);
  if (!raw) {
    return { accessToken: null, refreshToken: null, preAuthToken: null };
  }
  try {
    return JSON.parse(raw) as SessionState;
  } catch {
    return { accessToken: null, refreshToken: null, preAuthToken: null };
  }
}

function setSession(next: SessionState) {
  window.localStorage.setItem(sessionStorageKey, JSON.stringify(next));
}

function decodeAccessClaims(token: string | null): AccessClaims | null {
  if (!token) {
    return null;
  }
  const [, payload] = token.split(".");
  if (!payload) {
    return null;
  }
  try {
    const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
    const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
    return JSON.parse(window.atob(padded)) as AccessClaims;
  } catch {
    return null;
  }
}

export function readSession() {
  return getSession();
}

export function readAccessClaims() {
  return decodeAccessClaims(getSession().accessToken);
}

export function storeAuthenticatedSession(tokens: SessionTokens) {
  setSession({
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
    preAuthToken: null,
  });
}

export function storePreAuthToken(preAuthToken: string) {
  const session = getSession();
  setSession({
    ...session,
    preAuthToken,
  });
}

export function clearSession() {
  setSession({ accessToken: null, refreshToken: null, preAuthToken: null });
}

async function apiFetch<T>(
  path: string,
  init: RequestInit = {},
  auth: "none" | "access" | "preauth" = "access",
  retryOnUnauthorized = true,
): Promise<T> {
  const normalizedPath = normalizeApiPath(path);
  const session = getSession();
  const headers = new Headers(init.headers ?? {});
  if (!headers.has("content-type") && init.body) {
    headers.set("content-type", "application/json");
  }
  if (auth === "access" && session.accessToken) {
    headers.set("authorization", `Bearer ${session.accessToken}`);
  }
  if (auth === "preauth" && session.preAuthToken) {
    headers.set("authorization", `Bearer ${session.preAuthToken}`);
  }
  const response = await fetch(`${getBaseUrl()}${normalizedPath}`, {
    ...init,
    headers,
  });
  if (
    response.status === 401 &&
    auth === "access" &&
    retryOnUnauthorized &&
    session.refreshToken
  ) {
    await refreshAccessToken();
    return apiFetch<T>(normalizedPath, init, auth, false);
  }
  if (!response.ok) {
    const payload = (await response.json().catch(() => ({ message: response.statusText }))) as {
      message?: string;
    };
    if (response.status === 401 && auth !== "none") {
      clearSession();
    }
    throw new Error(payload.message ?? response.statusText);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

async function refreshAccessToken() {
  const session = getSession();
  if (!session.refreshToken) {
    clearSession();
    throw new Error(i18n.t("auth.session_expired"));
  }
  if (!refreshPromise) {
    refreshPromise = apiFetch<SessionTokens>(
      "/auth/refresh",
      {
        method: "POST",
        body: JSON.stringify({ refresh_token: session.refreshToken }),
      },
      "none",
      false,
    )
      .then((tokens) => {
        storeAuthenticatedSession(tokens);
        return tokens;
      })
      .catch((error) => {
        clearSession();
        throw error;
      })
      .finally(() => {
        refreshPromise = null;
      });
  }
  return refreshPromise;
}

export const api = {
  readSession,
  readAccessClaims,
  clearSession,
  storeAuthenticatedSession,
  storePreAuthToken,
  login(input: { email: string; password: string; totp_code?: string }) {
    return apiFetch<LoginResult>("/auth/login", {
      method: "POST",
      body: JSON.stringify(input),
    }, "none");
  },
  beginTotpSetup() {
    return apiFetch<TotpSetup>("/auth/totp/setup", { method: "POST" }, "preauth");
  },
  verifyTotp(code: string) {
    return apiFetch<SessionTokens>(
      "/auth/totp/verify",
      { method: "POST", body: JSON.stringify({ code }) },
      "preauth",
    );
  },
  refreshSession() {
    return refreshAccessToken();
  },
  async logout() {
    const refreshToken = getSession().refreshToken;
    if (!refreshToken) {
      clearSession();
      return { ok: true as const };
    }
    try {
      return await apiFetch<{ ok: true }>(
        "/auth/logout",
        { method: "POST", body: JSON.stringify({ refresh_token: refreshToken }) },
        "none",
        false,
      );
    } finally {
      clearSession();
    }
  },
  disableTotp(password: string) {
    return apiFetch<{ ok: true }>(
      "/auth/totp/disable",
      { method: "POST", body: JSON.stringify({ password }) },
    );
  },
  logoutAll() {
    return apiFetch<{ ok: true }>("/auth/logout-all", { method: "POST" });
  },
  listSessions() {
    return apiFetch<RefreshSessionInfo[]>("/auth/sessions");
  },
  changePassword(current_password: string, new_password: string) {
    return apiFetch<{ ok: true }>(
      "/auth/password",
      { method: "POST", body: JSON.stringify({ current_password, new_password }) },
    );
  },
  listUsers() {
    return apiFetch<User[]>("/users");
  },
  listResellers() {
    return apiFetch<User[]>("/resellers");
  },
  createUser(input: {
    target_tenant_id?: string | null;
    email: string;
    display_name: string;
    role: UserRole;
    password: string;
  }) {
    return apiFetch<User>("/users", { method: "POST", body: JSON.stringify(input) });
  },
  updateUser(
    userId: string,
    input: {
      email: string;
      display_name: string;
      role: UserRole;
      status: UserStatus;
      password?: string | null;
    },
  ) {
    return apiFetch<User>(`/users/${userId}`, {
      method: "PATCH",
      body: JSON.stringify(input),
    });
  },
  deleteUser(userId: string) {
    return apiFetch<{ ok: true }>(`/users/${userId}`, { method: "DELETE" });
  },
  createReseller(input: {
    tenant_name: string;
    email: string;
    display_name: string;
    password: string;
  }) {
    return apiFetch<User>("/resellers", { method: "POST", body: JSON.stringify(input) });
  },
  updateReseller(
    userId: string,
    input: {
      tenant_name: string;
      email: string;
      display_name: string;
      status: UserStatus;
      password?: string | null;
    },
  ) {
    return apiFetch<User>(`/resellers/${userId}`, {
      method: "PATCH",
      body: JSON.stringify(input),
    });
  },
  deleteReseller(userId: string) {
    return apiFetch<{ ok: true }>(`/resellers/${userId}`, { method: "DELETE" });
  },
  listDevices() {
    return apiFetch<Device[]>("/devices");
  },
  listSubscriptions() {
    return apiFetch<Subscription[]>("/subscriptions");
  },
  getPublicSubscription(token: string) {
    return publicFetch<PublicSubscription>(`/subscriptions/public/${token}`);
  },
  createSubscription(input: {
    tenant_id: string;
    name: string;
    note?: string | null;
    traffic_limit_bytes: number;
    expires_at: string;
  }) {
    return apiFetch<CreateSubscriptionResponse>("/subscriptions", {
      method: "POST",
      body: JSON.stringify(input),
    });
  },
  updateSubscription(
    subscriptionId: string,
    input: {
      name: string;
      note?: string | null;
      traffic_limit_bytes: number;
      expires_at: string;
      suspended: boolean;
    },
  ) {
    return apiFetch<Subscription>(`/subscriptions/${subscriptionId}`, {
      method: "PATCH",
      body: JSON.stringify(input),
    });
  },
  deleteSubscription(subscriptionId: string, tenantId: string) {
    return apiFetch<{ ok: true }>(
      `/subscriptions/${subscriptionId}?tenant_id=${tenantId}`,
      { method: "DELETE" },
    );
  },
  rotateSubscriptionLink(subscriptionId: string, tenantId: string) {
    return apiFetch<RotateSubscriptionLinkResponse>(
      `/subscriptions/${subscriptionId}/rotate-link?tenant_id=${tenantId}`,
      { method: "POST" },
    );
  },
  createNode(input: { tenant_id: string; name: string }) {
    return apiFetch<Node>("/nodes", {
      method: "POST",
      body: JSON.stringify(input),
    });
  },
  updateNode(nodeId: string, input: { tenant_id: string; name: string }) {
    return apiFetch<Node>(`/nodes/${nodeId}`, {
      method: "PATCH",
      body: JSON.stringify(input),
    });
  },
  deleteNode(nodeId: string, tenantId: string) {
    return apiFetch<{ ok: true }>(`/nodes/${nodeId}?tenant_id=${tenantId}`, {
      method: "DELETE",
    });
  },
  listNodes() {
    return apiFetch<Node[]>("/nodes");
  },
  listNodeDomains(nodeId: string, tenantId: string) {
    return apiFetch<NodeDomain[]>(`/nodes/${nodeId}/domains?tenant_id=${tenantId}`);
  },
  replaceNodeDomains(
    nodeId: string,
    input: {
      tenant_id: string;
      domains: Omit<NodeDomain, "id" | "node_id" | "created_at" | "updated_at">[];
    },
  ) {
    return apiFetch<NodeDomain[]>(`/nodes/${nodeId}/domains`, {
      method: "POST",
      body: JSON.stringify(input),
    });
  },
  listNodeRuntimeEndpoints(runtimeId: string, tenantId: string) {
    return apiFetch<NodeEndpoint[]>(`/node-runtimes/${runtimeId}/endpoints?tenant_id=${tenantId}`);
  },
  replaceNodeRuntimeEndpoints(
    runtimeId: string,
    input: { tenant_id: string; endpoints: NodeEndpointInput[] },
  ) {
    return apiFetch<NodeEndpoint[]>(`/node-runtimes/${runtimeId}/endpoints`, {
      method: "POST",
      body: JSON.stringify(input),
    });
  },
  createBootstrapToken(nodeId: string, input: { tenant_id: string; engines: ProxyEngine[] }) {
    return apiFetch<{
      bootstrap_token: string;
      tenant_id: string;
      node_id: string;
      node_name: string;
      engines: ProxyEngine[];
      expires_at: string;
    }>(`/nodes/${nodeId}/bootstrap-sessions`, {
      method: "POST",
      body: JSON.stringify(input),
    });
  },
  listRollouts() {
    return apiFetch<DeploymentRollout[]>("/rollouts");
  },
  listNotifications() {
    return apiFetch<NotificationEvent[]>("/notifications");
  },
  listUsage() {
    return apiFetch<UsageOverview[]>("/usage");
  },
  listAudit() {
    return apiFetch<AuditLog[]>("/audit");
  },
};

async function publicFetch<T>(path: string, init?: RequestInit) {
  const normalizedPath = normalizeApiPath(path);
  const response = await fetch(`${getBaseUrl()}${normalizedPath}`, {
    ...init,
    headers: {
      "content-type": "application/json",
      ...(init?.headers ?? {}),
    },
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return (await response.json()) as T;
}

