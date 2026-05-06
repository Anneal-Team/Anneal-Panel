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

export function formatNotificationBody(kind: string, value: string) {
  return value;
}
