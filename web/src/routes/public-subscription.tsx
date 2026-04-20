import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { api } from "@/lib/api";
import { formatBytes, formatDate, formatQuotaState } from "@/lib/format";
import { panelAssetUrl } from "@/lib/panel-base";
import { useNow } from "@/lib/use-now";

function statusTone(suspended: boolean, expired: boolean, quotaState: string) {
  if (suspended || expired) {
    return "danger" as const;
  }
  if (quotaState === "normal") {
    return "success" as const;
  }
  return "warning" as const;
}

export function PublicSubscriptionPage() {
  const { t } = useTranslation();
  const { token } = useParams({ from: "/import/$token" });
  const now = useNow();
  const [copied, setCopied] = useState(false);
  const sidebarAssetUrl = panelAssetUrl("anneal-sidebar.svg");
  const subscriptionQuery = useQuery({
    queryKey: ["public-subscription", token],
    queryFn: () => api.getPublicSubscription(token),
  });

  useEffect(() => {
    if (!copied) {
      return;
    }
    const timeout = window.setTimeout(() => {
      setCopied(false);
    }, 1600);
    return () => {
      window.clearTimeout(timeout);
    };
  }, [copied]);

  if (subscriptionQuery.isLoading) {
    return (
      <div className="space-y-8">
        <div>
          <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
            {t("public_subscription.group")}
          </div>
          <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">
            {t("public_subscription.loading_title")}
          </h1>
          <p className="mt-3 max-w-3xl text-base text-[#485644]">
            {t("public_subscription.loading_subtitle")}
          </p>
        </div>
      </div>
    );
  }

  if (subscriptionQuery.isError || !subscriptionQuery.data) {
    return (
      <div className="space-y-8">
        <div>
          <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
            {t("public_subscription.group")}
          </div>
          <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">
            {t("public_subscription.error_title")}
          </h1>
          <p className="mt-3 max-w-3xl text-base text-[#485644]">
            {t("public_subscription.error_subtitle")}
          </p>
        </div>
      </div>
    );
  }

  const subscription = subscriptionQuery.data;
  const expired = new Date(subscription.expires_at).getTime() <= now;
  const tone = statusTone(subscription.suspended, expired, subscription.quota_state);
  const statusLabel = subscription.suspended
    ? t("public_subscription.status.suspended")
    : expired
      ? t("public_subscription.status.expired")
      : t("public_subscription.status.active");

  async function handleCopy() {
    await navigator.clipboard.writeText(subscription.delivery_url);
    setCopied(true);
  }

  return (
    <div className="space-y-8">
      <div className="rounded-[32px] border border-white/60 bg-gradient-to-br from-[#f8f5f0] via-[#f2efe7] to-[#e8f0d9] p-6 shadow-panel md:p-8">
        <div className="flex flex-col gap-4">
          <div className="inline-flex w-fit items-center justify-center rounded-[20px] bg-[#141813] px-4 py-3 shadow-panel ring-1 ring-white/10">
            <img src={sidebarAssetUrl} alt="Anneal" className="block h-7 w-auto object-contain" />
          </div>
          <div>
            <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
              {t("public_subscription.group")}
            </div>
            <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{subscription.name}</h1>
            <p className="mt-3 max-w-4xl text-base text-[#485644]">
              {t("public_subscription.subtitle")}
            </p>
          </div>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <Card className="space-y-2 shadow-sm">
          <div className="text-xs uppercase tracking-widest text-[#485644]">
            {t("public_subscription.status_label")}
          </div>
          <div className="flex items-center gap-3">
            <div className="text-2xl font-bold text-[#1d271a]">{statusLabel}</div>
            <Badge tone={tone}>{formatQuotaState(subscription.quota_state)}</Badge>
          </div>
        </Card>
        <Card className="space-y-2 shadow-sm">
          <div className="text-xs uppercase tracking-widest text-[#485644]">
            {t("public_subscription.limit")}
          </div>
          <div className="text-2xl font-bold text-[#1d271a]">
            {formatBytes(subscription.traffic_limit_bytes)}
          </div>
        </Card>
        <Card className="space-y-2 shadow-sm">
          <div className="text-xs uppercase tracking-widest text-[#485644]">
            {t("public_subscription.used")}
          </div>
          <div className="text-2xl font-bold text-[#1d271a]">
            {formatBytes(subscription.used_bytes)}
          </div>
        </Card>
        <Card className="space-y-2 shadow-sm">
          <div className="text-xs uppercase tracking-widest text-[#485644]">
            {t("public_subscription.expires")}
          </div>
          <div className="text-2xl font-bold text-[#1d271a]">
            {formatDate(subscription.expires_at)}
          </div>
        </Card>
      </div>

      <div className="grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
        <Card className="space-y-4 shadow-sm">
          <div>
            <div className="text-xs uppercase tracking-widest text-[#485644]">
              {t("public_subscription.link_group")}
            </div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">
              {t("public_subscription.link_title")}
            </h2>
          </div>
          <div className="rounded-[24px] bg-[#f8f5f0] px-4 py-4">
            <div className="text-[11px] uppercase tracking-[0.2em] text-foreground/70">
              {t("public_subscription.delivery_url")}
            </div>
            <a
              href={subscription.delivery_url}
              className="mt-3 block break-all rounded-[18px] border border-[#d8d1c3] bg-white px-4 py-3 font-mono text-xs text-[#1d271a] transition hover:border-[#9bb779] hover:text-[#24331d]"
            >
              {subscription.delivery_url}
            </a>
          </div>
          <div className="flex flex-wrap gap-3">
            <Button type="button" onClick={handleCopy}>
              {copied ? t("public_subscription.copied") : t("public_subscription.copy")}
            </Button>
            <Button
              type="button"
              variant="secondary"
              onClick={() => {
                window.open(`${subscription.delivery_url}?raw=1`, "_blank");
              }}
            >
              {t("public_subscription.raw")}
            </Button>
          </div>
          <div className="rounded-[24px] bg-[#f2efe4] px-4 py-4 text-sm text-foreground/80">
            {t("public_subscription.link_hint")}
          </div>
        </Card>

        <Card className="space-y-4 shadow-sm">
          <div>
            <div className="text-xs uppercase tracking-widest text-[#485644]">
              {t("public_subscription.note_group")}
            </div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">
              {t("public_subscription.note_title")}
            </h2>
          </div>
          <div className="rounded-[24px] bg-[#f8f5f0] px-4 py-4 text-sm text-foreground/80">
            {subscription.note?.trim() || t("public_subscription.note_empty")}
          </div>
        </Card>
      </div>
    </div>
  );
}
