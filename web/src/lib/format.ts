import i18n from "./i18n";

export function formatDate(value: string | null | undefined) {
  if (!value) {
    return "-";
  }
  return new Intl.DateTimeFormat(i18n.language === "ru" ? "ru-RU" : "en-US", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(new Date(value));
}

export function formatBytes(value: number) {
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  let amount = value;
  let unit = 0;
  while (amount >= 1024 && unit < sizes.length - 1) {
    amount /= 1024;
    unit += 1;
  }
  return `${amount.toFixed(amount >= 10 || unit === 0 ? 0 : 1)} ${sizes[unit]}`;
}

export function formatRole(value: string) {
  switch (value) {
    case "superadmin":
      return i18n.t("role.superadmin");
    case "admin":
      return i18n.t("role.admin");
    case "reseller":
      return i18n.t("role.reseller");
    case "user":
      return i18n.t("role.user");
    default:
      return value;
  }
}

export function formatQuotaState(value: string) {
  switch (value) {
    case "normal":
      return i18n.t("quota.normal");
    case "warning80":
      return "80%";
    case "warning95":
      return "95%";
    case "exhausted":
      return i18n.t("quota.exhausted");
    default:
      return value;
  }
}

export function formatNodeStatus(value: string) {
  switch (value) {
    case "pending":
      return i18n.t("status.pending");
    case "online":
      return i18n.t("status.online");
    case "offline":
      return i18n.t("status.offline");
    default:
      return value;
  }
}

export function formatNodeName(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return value;
  }
  return trimmed
    .split(/[-_]+/)
    .filter(Boolean)
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

export function formatNotificationBody(kind: string, value: string) {
  if (kind !== "node_offline") {
    return value;
  }
  const match = /^Node (.+) is offline$/.exec(value.trim());
  if (!match) {
    return value;
  }
  return `Node ${formatNodeName(match[1])} is offline`;
}

export function formatDeploymentStatus(value: string) {
  switch (value) {
    case "queued":
      return i18n.t("deploy.queued");
    case "rendering":
      return i18n.t("deploy.rendering");
    case "validating":
      return i18n.t("deploy.validating");
    case "ready":
      return i18n.t("deploy.ready");
    case "applied":
      return i18n.t("deploy.applied");
    case "rolled_back":
      return i18n.t("deploy.rolled_back");
    case "failed":
      return i18n.t("deploy.failed");
    default:
      return value;
  }
}
