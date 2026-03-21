import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { AuthRequired } from "@/components/auth-required";
import { ExpandableText } from "@/components/expandable-text";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { api } from "@/lib/api";
import { formatDate, formatDeploymentStatus } from "@/lib/format";

export function RolloutsPage() {
  const { t } = useTranslation();
  const session = api.readSession();
  const rolloutsQuery = useQuery({
    queryKey: ["rollouts"],
    queryFn: api.listRollouts,
    enabled: Boolean(session.accessToken),
  });

  if (!session.accessToken) {
    return <AuthRequired title={t("rollouts.unauthorized")} />;
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
          {t("nav_group.system")}
        </div>
        <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("rollouts.title")}</h1>
        <p className="mt-3 max-w-4xl text-base text-[#485644]">
          {t("rollouts.subtitle")}
        </p>
      </div>
      <Card className="space-y-4 shadow-sm">
        <div>
          <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("rollouts.list.title")}</h2>
        </div>
        <div className="space-y-3">
          {(rolloutsQuery.data ?? []).map((rollout) => (
            <div key={rollout.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
              <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
                <div>
                  <div className="flex flex-wrap items-center gap-3">
                    <div className="text-lg font-semibold">{rollout.revision_name}</div>
                    <Badge
                      tone={
                        rollout.status === "applied" || rollout.status === "ready"
                          ? "success"
                          : rollout.status === "failed"
                            ? "danger"
                            : "warning"
                      }
                    >
                      {formatDeploymentStatus(rollout.status)}
                    </Badge>
                    <Badge tone="muted">{rollout.engine}</Badge>
                  </div>
                  <div className="mt-2 text-sm text-foreground/80">{rollout.target_path}</div>
                  <div className="mt-2 text-xs text-foreground/90">
                    revision {rollout.config_revision_id} · {formatDate(rollout.updated_at)}
                  </div>
                  {rollout.failure_reason ? (
                    <div className="mt-2 text-sm text-danger cursor-text">
                      <ExpandableText text={rollout.failure_reason} />
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          ))}
        </div>
      </Card>
    </div>
  );
}
