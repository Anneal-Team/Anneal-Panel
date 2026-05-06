import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { AuthRequired } from "@/components/auth-required";
import { ExpandableText } from "@/components/expandable-text";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { api } from "@/lib/api";
import { formatBytes, formatDate, formatNotificationBody, formatQuotaState } from "@/lib/format";

export function NotificationsPage() {
  const { t } = useTranslation();
  const session = api.readSession();
  const notificationsQuery = useQuery({
    queryKey: ["notifications"],
    queryFn: api.listNotifications,
    enabled: Boolean(session.accessToken),
  });
  const usageQuery = useQuery({
    queryKey: ["usage"],
    queryFn: api.listUsage,
    enabled: Boolean(session.accessToken),
  });
  const auditQuery = useQuery({
    queryKey: ["audit"],
    queryFn: api.listAudit,
    enabled: Boolean(session.accessToken),
  });

  if (!session.accessToken) {
    return <AuthRequired title={t("notifications.unauthorized")} />;
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
          {t("nav_group.overview")}
        </div>
        <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("notifications.title")}</h1>
        <p className="mt-3 max-w-4xl text-base text-[#485644]">
          {t("notifications.subtitle")}
        </p>
      </div>
      <div className="grid gap-6 xl:grid-cols-[1.05fr_0.95fr]">
        <Card className="space-y-4 shadow-sm">
          <div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("notifications.events.title")}</h2>
          </div>
          <div className="space-y-3">
            {(notificationsQuery.data ?? []).map((event) => (
              <div key={event.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-lg font-semibold">{event.title}</div>
                  <Badge tone={event.kind === "quota100" ? "danger" : "warning"}>
                    {event.kind}
                  </Badge>
                </div>
                <div className="mt-2 text-sm text-foreground/90">
                  <ExpandableText text={formatNotificationBody(event.kind, event.body)} />
                </div>
                <div className="mt-2 text-xs text-foreground/90">
                  {formatDate(event.created_at)} · доставлено {formatDate(event.delivered_at)}
                </div>
              </div>
            ))}
          </div>
        </Card>
        <div className="space-y-6">
          <Card className="space-y-4 shadow-sm">
            <div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("notifications.quota.title")}</h2>
            </div>
            <div className="space-y-3">
              {(usageQuery.data ?? []).slice(0, 8).map((entry) => (
                <div key={entry.subscription_id} className="rounded-2xl border border-border bg-[#f8f5f0] p-4 text-sm">
                  <div className="flex items-center justify-between gap-3">
                    <div className="font-semibold">{entry.subscription_name}</div>
                    <Badge
                      tone={
                        entry.quota_state === "exhausted"
                          ? "danger"
                          : entry.quota_state === "normal"
                            ? "success"
                            : "warning"
                      }
                    >
                      {formatQuotaState(entry.quota_state)}
                    </Badge>
                  </div>
                  <div className="mt-2 text-foreground/80">{entry.device_name}</div>
                  <div className="mt-2 text-foreground/80">
                    {formatBytes(entry.used_bytes)} / {formatBytes(entry.traffic_limit_bytes)}
                  </div>
                </div>
              ))}
            </div>
          </Card>
          <Card className="space-y-4 shadow-sm">
            <div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("notifications.audit.title")}</h2>
            </div>
            <div className="space-y-3">
              {(auditQuery.data ?? []).slice(0, 10).map((entry) => (
                <div key={entry.id} className="rounded-2xl border border-border bg-[#f8f5f0] p-4 text-sm">
                  <div className="font-semibold">{entry.action}</div>
                  <div className="mt-2 text-foreground/80">
                    {entry.resource_type} · {entry.resource_id ?? "n/a"}
                  </div>
                  <div className="mt-2 text-xs text-foreground/90">{formatDate(entry.created_at)}</div>
                </div>
              ))}
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
