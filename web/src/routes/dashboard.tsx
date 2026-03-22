import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { AuthRequired } from "@/components/auth-required";
import { ExpandableText } from "@/components/expandable-text";
import { MetricCard } from "@/components/metric-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import {
  formatBytes,
  formatDate,
  formatDeploymentStatus,
  formatNodeName,
  formatNodeStatus,
  formatNotificationBody,
  formatQuotaState,
} from "@/lib/format";
import { useNow } from "@/lib/use-now";

type AttentionItem = {
  id: string;
  title: string;
  description: string;
  tone: "warning" | "danger";
};

function isExpiringSoon(value: string, now: number) {
  return new Date(value).getTime() - now < 3 * 24 * 60 * 60 * 1000;
}

function sumBytes(values: number[]) {
  return values.reduce((total, value) => total + value, 0);
}

function rolloutTone(status: string) {
  if (status === "applied" || status === "ready") {
    return "success" as const;
  }
  if (status === "failed" || status === "rolled_back") {
    return "danger" as const;
  }
  return "warning" as const;
}

function notificationTone(kind: string) {
  if (kind === "node_offline" || kind === "quota100") {
    return "danger" as const;
  }
  return "warning" as const;
}

function sessionLabel(userAgent: string | null, ipAddress: string | null, fallback: string) {
  const parts = [userAgent?.trim(), ipAddress?.trim()].filter(Boolean);
  if (parts.length === 0) {
    return fallback;
  }
  return parts.join(" / ");
}

export function DashboardPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = api.readSession();
  const now = useNow();
  const [passwordForm, setPasswordForm] = useState({
    current_password: "",
    new_password: "",
  });
  const [disableTotpPassword, setDisableTotpPassword] = useState("");
  const [securityMessage, setSecurityMessage] = useState<string | null>(null);
  const [securityError, setSecurityError] = useState<string | null>(null);

  const usersQuery = useQuery({ queryKey: ["users"], queryFn: api.listUsers, enabled: Boolean(session.accessToken) });
  const nodesQuery = useQuery({ queryKey: ["nodes"], queryFn: api.listNodes, enabled: Boolean(session.accessToken) });
  const subscriptionsQuery = useQuery({
    queryKey: ["subscriptions"],
    queryFn: api.listSubscriptions,
    enabled: Boolean(session.accessToken),
  });
  const rolloutsQuery = useQuery({
    queryKey: ["rollouts"],
    queryFn: api.listRollouts,
    enabled: Boolean(session.accessToken),
  });
  const notificationsQuery = useQuery({
    queryKey: ["notifications"],
    queryFn: api.listNotifications,
    enabled: Boolean(session.accessToken),
  });
  const sessionsQuery = useQuery({
    queryKey: ["sessions"],
    queryFn: api.listSessions,
    enabled: Boolean(session.accessToken),
  });

  const users = usersQuery.data ?? [];
  const serverNodes = [...(nodesQuery.data ?? [])].sort((left, right) => {
    const leftTime = new Date(left.updated_at).getTime();
    const rightTime = new Date(right.updated_at).getTime();
    return rightTime - leftTime;
  });
  const nodes = serverNodes
    .flatMap((node) =>
      node.runtimes.map((runtime) => ({
        ...runtime,
        node_name: node.name,
      })),
    )
    .sort((left, right) => {
      const leftTime = new Date(left.updated_at).getTime();
      const rightTime = new Date(right.updated_at).getTime();
      return rightTime - leftTime;
    });
  const subscriptions = [...(subscriptionsQuery.data ?? [])].sort((left, right) => {
    const leftTime = new Date(left.updated_at).getTime();
    const rightTime = new Date(right.updated_at).getTime();
    return rightTime - leftTime;
  });
  const rollouts = [...(rolloutsQuery.data ?? [])].sort((left, right) => {
    const leftTime = new Date(left.updated_at).getTime();
    const rightTime = new Date(right.updated_at).getTime();
    return rightTime - leftTime;
  });
  const notifications = [...(notificationsQuery.data ?? [])].sort((left, right) => {
    const leftTime = new Date(left.created_at).getTime();
    const rightTime = new Date(right.created_at).getTime();
    return rightTime - leftTime;
  });
  const sessions = [...(sessionsQuery.data ?? [])].sort((left, right) => {
    const leftTime = new Date(left.created_at).getTime();
    const rightTime = new Date(right.created_at).getTime();
    return rightTime - leftTime;
  });

  const activeUsers = users.filter((user) => user.status === "active");
  const resellers = users.filter((user) => user.role === "reseller");
  const onlineNodes = nodes.filter((node) => node.status === "online");
  const offlineNodes = nodes.filter((node) => node.status === "offline");
  const activeSubscriptions = subscriptions.filter((subscription) => !subscription.suspended);
  const suspendedSubscriptions = subscriptions.filter((subscription) => subscription.suspended);
  const quotaProblems = subscriptions.filter((subscription) => subscription.quota_state !== "normal");
  const expiringSoon = subscriptions.filter(
    (subscription) => !subscription.suspended && isExpiringSoon(subscription.expires_at, now),
  );
  const activeSessions = sessions.filter((entry) => !entry.revoked_at && new Date(entry.expires_at).getTime() > now);
  const failedRollouts = rollouts.filter((rollout) => rollout.status === "failed");
  const queuedRollouts = rollouts.filter((rollout) =>
    ["queued", "rendering", "validating", "ready"].includes(rollout.status),
  );
  const totalUsedBytes = sumBytes(subscriptions.map((subscription) => subscription.used_bytes));
  const totalLimitBytes = sumBytes(subscriptions.map((subscription) => subscription.traffic_limit_bytes));

  const attentionItems: AttentionItem[] = [
    ...offlineNodes.map((node) => ({
      id: `node-${node.id}`,
      title: `${formatNodeName(node.node_name)} / ${node.engine}`,
      description: t("dashboard.attention.node_offline", { date: formatDate(node.last_seen_at) }),
      tone: "danger" as const,
    })),
    ...failedRollouts.map((rollout) => ({
      id: `rollout-${rollout.id}`,
      title: t("dashboard.attention.rollout_failed", { name: rollout.revision_name }),
      description: rollout.failure_reason ?? `${rollout.engine} / ${rollout.target_path}`,
      tone: "danger" as const,
    })),
    ...subscriptions
      .filter((subscription) => subscription.suspended || subscription.quota_state === "exhausted")
      .map((subscription) => ({
        id: `subscription-danger-${subscription.id}`,
        title: t("dashboard.attention.subscription_blocked", { name: subscription.name }),
        description: t("dashboard.attention.subscription_blocked_description", {
          used: formatBytes(subscription.used_bytes),
          limit: formatBytes(subscription.traffic_limit_bytes),
          date: formatDate(subscription.expires_at),
        }),
        tone: "danger" as const,
      })),
    ...subscriptions
      .filter(
        (subscription) =>
          !subscription.suspended &&
          subscription.quota_state !== "normal" &&
          subscription.quota_state !== "exhausted",
      )
      .map((subscription) => ({
        id: `subscription-warning-${subscription.id}`,
        title: t("dashboard.attention.subscription_warning", { name: subscription.name }),
        description: t("dashboard.attention.subscription_warning_description", {
          state: formatQuotaState(subscription.quota_state),
          used: formatBytes(subscription.used_bytes),
          limit: formatBytes(subscription.traffic_limit_bytes),
        }),
        tone: "warning" as const,
      })),
    ...expiringSoon.map((subscription) => ({
      id: `subscription-expiring-${subscription.id}`,
      title: t("dashboard.attention.subscription_expiring", { name: subscription.name }),
      description: t("dashboard.attention.subscription_expiring_description", {
        date: formatDate(subscription.expires_at),
      }),
      tone: "warning" as const,
    })),
  ].slice(0, 8);

  const disableTotpMutation = useMutation({
    mutationFn: () => api.disableTotp(disableTotpPassword),
    onSuccess: async () => {
      setSecurityError(null);
      setSecurityMessage(t("dashboard.security.disable_success"));
      setDisableTotpPassword("");
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
    onError: (error) => {
      setSecurityMessage(null);
      setSecurityError(error.message);
    },
  });

  const changePasswordMutation = useMutation({
    mutationFn: () => api.changePassword(passwordForm.current_password, passwordForm.new_password),
    onSuccess: async () => {
      setSecurityError(null);
      setSecurityMessage(t("dashboard.security.password_success"));
      setPasswordForm({ current_password: "", new_password: "" });
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
    onError: (error) => {
      setSecurityMessage(null);
      setSecurityError(error.message);
    },
  });

  const logoutAllMutation = useMutation({
    mutationFn: api.logoutAll,
    onSuccess: async () => {
      setSecurityError(null);
      setSecurityMessage(t("dashboard.security.logout_all_success"));
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
    onError: (error) => {
      setSecurityMessage(null);
      setSecurityError(error.message);
    },
  });

  if (!session.accessToken) {
    return <AuthRequired title={t("app.subtitle")} />;
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
          {t("nav_group.overview")}
        </div>
        <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("dashboard.title")}</h1>
        <p className="mt-3 max-w-4xl text-base text-[#485644]">
          {t("dashboard.subtitle")}
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label={t("dashboard.metrics.users_label")}
          value={String(activeUsers.length)}
          hint={t("dashboard.metrics.users_hint", { count: resellers.length })}
        />
        <MetricCard
          label={t("dashboard.metrics.subscriptions_label")}
          value={String(activeSubscriptions.length)}
          hint={t("dashboard.metrics.subscriptions_hint", {
            count: quotaProblems.length + suspendedSubscriptions.length,
          })}
        />
        <MetricCard
          label={t("dashboard.metrics.runtimes_label")}
          value={String(onlineNodes.length)}
          hint={t("dashboard.metrics.runtimes_hint", {
            total: nodes.length,
            offline: offlineNodes.length,
          })}
        />
        <MetricCard
          label={t("dashboard.metrics.traffic_label")}
          value={formatBytes(totalUsedBytes)}
          hint={t("dashboard.metrics.traffic_hint", { limit: formatBytes(totalLimitBytes) })}
        />
      </div>

      <div className="grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
        <div className="space-y-6">
          <Card className="space-y-4 shadow-sm">
            <div>
              <div className="text-xs uppercase tracking-widest text-[#485644]">{t("dashboard.attention.group")}</div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("dashboard.attention.title")}</h2>
            </div>
            {attentionItems.length > 0 ? (
              <div className="space-y-3">
                {attentionItems.map((item) => (
                  <div key={item.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                    <div className="flex flex-wrap items-center gap-3">
                      <div className="text-lg font-semibold">{item.title}</div>
                      <Badge tone={item.tone}>
                        {item.tone === "danger"
                          ? t("dashboard.attention.badge.critical")
                          : t("dashboard.attention.badge.review")}
                      </Badge>
                    </div>
                    <div className="mt-2 text-sm text-foreground/80">
                    <ExpandableText text={item.description} />
                  </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-4 py-8 text-center text-sm text-[#485644]">
                {t("dashboard.attention.empty")}
              </div>
            )}
          </Card>

          <Card className="space-y-4 shadow-sm">
            <div>
              <div className="text-xs uppercase tracking-widest text-[#485644]">{t("dashboard.rollouts.group")}</div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("dashboard.rollouts.title")}</h2>
            </div>
            <div className="space-y-3">
              {rollouts.slice(0, 6).map((rollout) => (
                <div key={rollout.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                  <div className="flex flex-wrap items-center gap-3">
                    <div className="text-lg font-semibold">{rollout.revision_name}</div>
                    <Badge tone={rolloutTone(rollout.status)}>{formatDeploymentStatus(rollout.status)}</Badge>
                  </div>
                  <div className="mt-2 text-sm text-foreground/80">
                    {rollout.engine} / {rollout.target_path}
                  </div>
                  {rollout.failure_reason ? (
                    <div className="mt-2 text-sm text-danger">
                      <ExpandableText text={rollout.failure_reason} />
                    </div>
                  ) : null}
                  <div className="mt-2 text-xs text-foreground/90">{formatDate(rollout.updated_at)}</div>
                </div>
              ))}
              {rollouts.length === 0 ? (
                <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-4 py-8 text-center text-sm text-[#485644]">
                  {t("dashboard.rollouts.empty")}
                </div>
              ) : null}
            </div>
          </Card>

          <Card className="space-y-4 shadow-sm">
            <div>
              <div className="text-xs uppercase tracking-widest text-[#485644]">{t("dashboard.events.group")}</div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("dashboard.events.title")}</h2>
            </div>
            <div className="space-y-3">
              {notifications.slice(0, 6).map((event) => (
                <div key={event.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                  <div className="flex flex-wrap items-center gap-3">
                    <div className="text-lg font-semibold">{event.title}</div>
                    <Badge tone={notificationTone(event.kind)}>{event.kind}</Badge>
                  </div>
                  <div className="mt-2 text-sm text-foreground/80">
                    <ExpandableText text={formatNotificationBody(event.kind, event.body)} />
                  </div>
                  <div className="mt-2 text-xs text-foreground/90">{formatDate(event.created_at)}</div>
                </div>
              ))}
              {notifications.length === 0 ? (
                <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-4 py-8 text-center text-sm text-[#485644]">
                  {t("dashboard.events.empty")}
                </div>
              ) : null}
            </div>
          </Card>
        </div>

        <div className="space-y-6">
          <Card className="space-y-4 shadow-sm">
            <div>
              <div className="text-xs uppercase tracking-widest text-[#485644]">{t("dashboard.structure.group")}</div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("dashboard.structure.title")}</h2>
            </div>
            <div className="grid gap-3">
              <div className="rounded-[18px] bg-[#fbf7ef] px-4 py-3 text-sm text-[#485644]">
                {t("dashboard.stat.node_groups")}: <span className="font-bold text-[#1d271a]">{serverNodes.length}</span>
              </div>
              <div className="rounded-[18px] bg-[#fbf7ef] px-4 py-3 text-sm text-[#485644]">
                {t("dashboard.stat.pending_runtimes")}: <span className="font-bold text-[#1d271a]">{nodes.filter((node) => node.status === "pending").length}</span>
              </div>
              <div className="rounded-[18px] bg-[#fbf7ef] px-4 py-3 text-sm text-[#485644]">
                {t("dashboard.stat.expiring_soon")}: <span className="font-bold text-[#1d271a]">{expiringSoon.length}</span>
              </div>
              <div className="rounded-[18px] bg-[#fbf7ef] px-4 py-3 text-sm text-[#485644]">
                {t("dashboard.stat.active_sessions")}: <span className="font-bold text-[#1d271a]">{activeSessions.length}</span>
              </div>
              <div className="rounded-[18px] bg-[#fbf7ef] px-4 py-3 text-sm text-[#485644]">
                {t("dashboard.stat.queued_rollouts")}: <span className="font-bold text-[#1d271a]">{queuedRollouts.length}</span>
              </div>
            </div>
          </Card>

          <Card className="space-y-4 shadow-sm">
            <div>
              <div className="text-xs uppercase tracking-widest text-[#485644]">{t("dashboard.nodes.group")}</div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("dashboard.nodes.title")}</h2>
            </div>
            <div className="space-y-3">
              {nodes.slice(0, 6).map((node) => (
                <div key={node.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                  <div className="flex flex-wrap items-center gap-3">
                    <div className="text-lg font-semibold">{formatNodeName(node.node_name)}</div>
                    <Badge
                      tone={
                        node.status === "online"
                          ? "success"
                          : node.status === "offline"
                            ? "danger"
                            : "warning"
                      }
                    >
                      {formatNodeStatus(node.status)}
                    </Badge>
                    <Badge tone="muted">{node.engine}</Badge>
                  </div>
                  <div className="mt-2 text-sm text-foreground/80">
                    {t("dashboard.nodes.version", { version: node.version })}
                  </div>
                  <div className="mt-2 text-xs text-foreground/90">
                    {t("dashboard.nodes.last_seen")}: {formatDate(node.last_seen_at)}
                  </div>
                </div>
              ))}
              {nodes.length === 0 ? (
                <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-4 py-8 text-center text-sm text-[#485644]">
                  {t("dashboard.nodes.empty")}
                </div>
              ) : null}
            </div>
          </Card>

          <Card className="space-y-4 shadow-sm p-6">
            <div>
              <div className="text-xs uppercase tracking-widest text-[#485644]">{t("dashboard.security.group")}</div>
              <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("dashboard.security.access_title")}</h2>
            </div>

            <div className="space-y-3">
              {activeSessions.slice(0, 4).map((entry) => (
                <div key={entry.id} className="rounded-[22px] border border-border bg-[#f8f5f0] px-4 py-3 text-sm">
                  <div className="font-semibold">
                    {sessionLabel(entry.user_agent, entry.ip_address, t("dashboard.security.session_fallback"))}
                  </div>
                  <div className="mt-1 text-foreground/80">
                    {t("dashboard.security.session_created")}: {formatDate(entry.created_at)}
                  </div>
                  <div className="mt-1 text-foreground/80">
                    {t("dashboard.security.session_expires")}: {formatDate(entry.expires_at)}
                  </div>
                </div>
              ))}
              {activeSessions.length === 0 ? (
                <div className="rounded-[22px] border border-dashed border-border bg-[#f8f5f0] px-4 py-6 text-sm text-[#485644]">
                  {t("dashboard.security.sessions_empty")}
                </div>
              ) : null}
            </div>

            <form
              className="grid gap-3"
              onSubmit={(event) => {
                event.preventDefault();
                changePasswordMutation.mutate();
              }}
            >
              <Input
                placeholder={t("dashboard.security.current_password")}
                type="password"
                value={passwordForm.current_password}
                onChange={(event) =>
                  setPasswordForm((current) => ({
                    ...current,
                    current_password: event.target.value,
                  }))
                }
              />
              <Input
                placeholder={t("dashboard.security.new_password")}
                type="password"
                value={passwordForm.new_password}
                onChange={(event) =>
                  setPasswordForm((current) => ({
                    ...current,
                    new_password: event.target.value,
                  }))
                }
              />
              <Button disabled={changePasswordMutation.isPending} type="submit">
                {changePasswordMutation.isPending ? t("dashboard.security.changing") : t("dashboard.security.change_password")}
              </Button>
            </form>

            <div className="grid gap-3 rounded-[18px] bg-[#fbf7ef] p-4">
              <Input
                placeholder={t("dashboard.security.password_for_totp")}
                type="password"
                value={disableTotpPassword}
                onChange={(event) => setDisableTotpPassword(event.target.value)}
              />
              <Button
                variant="secondary"
                disabled={disableTotpMutation.isPending}
                onClick={() => disableTotpMutation.mutate()}
                type="button"
              >
                {disableTotpMutation.isPending ? t("dashboard.security.disabling") : t("dashboard.security.disable_totp")}
              </Button>
              <Button
                variant="secondary"
                disabled={logoutAllMutation.isPending}
                onClick={() => logoutAllMutation.mutate()}
                type="button"
              >
                {logoutAllMutation.isPending ? t("dashboard.security.logging_out") : t("dashboard.security.logout_all")}
              </Button>
              {securityMessage ? <div className="text-sm text-emerald-700">{securityMessage}</div> : null}
              {securityError ? <div className="text-sm text-danger">{securityError}</div> : null}
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
