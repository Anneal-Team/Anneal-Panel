import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { AuthRequired } from "@/components/auth-required";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { api } from "@/lib/api";
import { formatDate } from "@/lib/format";

export function DevicesPage() {
  const { t } = useTranslation();
  const session = api.readSession();
  const devicesQuery = useQuery({
    queryKey: ["devices"],
    queryFn: api.listDevices,
    enabled: Boolean(session.accessToken),
  });

  if (!session.accessToken) {
    return <AuthRequired title={t("devices.unauthorized")} />;
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
          {t("nav_group.system")}
        </div>
        <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("devices.title")}</h1>
        <p className="mt-3 max-w-4xl text-base text-[#485644]">
          {t("devices.subtitle")}
        </p>
      </div>
      <Card className="space-y-4 shadow-sm">
        <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("devices.list.title")}</h2>
        <div className="space-y-3">
          {(devicesQuery.data ?? []).map((device) => (
            <div key={device.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
              <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
                <div>
                  <div className="flex items-center gap-3">
                    <div className="text-lg font-semibold">{device.name}</div>
                    <Badge tone={device.suspended ? "danger" : "success"}>
                      {device.suspended ? "заблокировано" : "активно"}
                    </Badge>
                  </div>
                  <div className="mt-2 text-sm text-foreground/80">
                    tenant {device.tenant_id} · user {device.user_id}
                  </div>
                  <div className="mt-1 break-all font-mono text-xs text-foreground/80">
                    {device.id}
                  </div>
                </div>
                <div className="text-xs text-foreground/90">{formatDate(device.created_at)}</div>
              </div>
            </div>
          ))}
          {devicesQuery.data?.length === 0 ? (
            <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-4 py-10 text-center text-sm text-foreground/80">
              Пока нет видимых устройств. После полной автопривязки здесь останется только
              системный инвентарь без ручного CRUD.
            </div>
          ) : null}
        </div>
      </Card>
    </div>
  );
}
