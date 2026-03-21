import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { AuthRequired } from "@/components/auth-required";
import { MetricCard } from "@/components/metric-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { api, type Node, type NodeGroup, type ProxyEngine } from "@/lib/api";
import { formatDate, formatNodeStatus } from "@/lib/format";

const runtimeProtocols: Record<ProxyEngine, string> = {
  xray: "vless_reality,vmess,trojan,shadowsocks_2022",
  singbox: "vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2",
};

type ServerCard = {
  id: string;
  tenant_id: string;
  name: string;
  runtimes: Partial<Record<ProxyEngine, Node>>;
};

type NodeDialogMode = "create" | "edit";

function buildInstallBlock(name: string, tokens: Record<ProxyEngine, string>) {
  return [
    `ANNEAL_AGENT_SERVER_URL=${window.location.origin}`,
    `ANNEAL_AGENT_NAME=${name}`,
    "ANNEAL_AGENT_ENGINES=xray,singbox",
    `ANNEAL_AGENT_PROTOCOLS_XRAY=${runtimeProtocols.xray}`,
    `ANNEAL_AGENT_PROTOCOLS_SINGBOX=${runtimeProtocols.singbox}`,
    `ANNEAL_AGENT_ENROLLMENT_TOKENS=xray:${tokens.xray},singbox:${tokens.singbox}`,
    "./install.sh --role node",
  ].join("\n");
}

function overallStatus(server: ServerCard) {
  const states = Object.values(server.runtimes)
    .filter(Boolean)
    .map((runtime) => runtime.status);
  if (states.includes("online")) {
    return "online";
  }
  if (states.includes("pending")) {
    return "pending";
  }
  return "offline";
}

function groupServers(nodeGroups: NodeGroup[] | undefined, nodes: Node[] | undefined) {
  const servers = new Map<string, ServerCard>();
  for (const group of nodeGroups ?? []) {
    servers.set(group.id, {
      id: group.id,
      tenant_id: group.tenant_id,
      name: group.name,
      runtimes: {},
    });
  }
  for (const node of nodes ?? []) {
    const existing = servers.get(node.node_group_id) ?? {
      id: node.node_group_id,
      tenant_id: node.tenant_id,
      name: node.name,
      runtimes: {},
    };
    existing.runtimes[node.engine] = node;
    servers.set(node.node_group_id, existing);
  }
  return Array.from(servers.values()).sort((left, right) => left.name.localeCompare(right.name));
}

function runtimeTone(node: Node | undefined) {
  if (!node) {
    return "muted" as const;
  }
  if (node.status === "online") {
    return "success" as const;
  }
  if (node.status === "pending") {
    return "warning" as const;
  }
  return "danger" as const;
}

function runtimeLabel(node: Node | undefined) {
  if (!node) {
    return "Не зарегистрирован";
  }
  return formatNodeStatus(node.status);
}

function tenantLabel(
  tenantId: string,
  tenantNames: Map<string, string>,
) {
  return tenantNames.get(tenantId) ?? tenantId;
}

export function NodesPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = api.readSession();
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dialogMode, setDialogMode] = useState<NodeDialogMode>("create");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<ServerCard | null>(null);
  const [installBlock, setInstallBlock] = useState<string | null>(null);
  const [form, setForm] = useState({
    node_group_id: "",
    tenant_id: "",
    name: "",
  });

  const resellersQuery = useQuery({
    queryKey: ["resellers"],
    queryFn: api.listResellers,
    enabled: Boolean(session.accessToken),
  });
  const nodeGroupsQuery = useQuery({
    queryKey: ["node-groups"],
    queryFn: api.listNodeGroups,
    enabled: Boolean(session.accessToken),
  });
  const nodesQuery = useQuery({
    queryKey: ["nodes"],
    queryFn: api.listNodes,
    enabled: Boolean(session.accessToken),
  });

  const tenantNames = useMemo(() => {
    const entries = (resellersQuery.data ?? [])
      .filter((user) => user.tenant_id)
      .map((user) => [user.tenant_id as string, user.tenant_name ?? user.display_name] as const);
    return new Map(entries);
  }, [resellersQuery.data]);

  const tenantOptions = useMemo(
    () =>
      (resellersQuery.data ?? [])
        .filter((user) => user.tenant_id)
        .map((user) => ({
          tenant_id: user.tenant_id as string,
          label: `${user.tenant_name ?? user.display_name} · ${user.email}`,
        })),
    [resellersQuery.data],
  );

  const servers = useMemo(
    () => groupServers(nodeGroupsQuery.data, nodesQuery.data),
    [nodeGroupsQuery.data, nodesQuery.data],
  );

  const createNodeMutation = useMutation({
    mutationFn: async () => {
      const name = form.name.trim();
      const group = await api.createNodeGroup({
        tenant_id: form.tenant_id,
        name,
      });
      const [xrayGrant, singboxGrant] = await Promise.all([
        api.createEnrollmentToken({
          tenant_id: form.tenant_id,
          node_group_id: group.id,
          engine: "xray",
        }),
        api.createEnrollmentToken({
          tenant_id: form.tenant_id,
          node_group_id: group.id,
          engine: "singbox",
        }),
      ]);
      return {
        group,
        installBlock: buildInstallBlock(name, {
          xray: xrayGrant.token,
          singbox: singboxGrant.token,
        }),
      };
    },
    onSuccess: async (result) => {
      setError(null);
      setMessage(`Сервер ${result.group.name} подготовлен.`);
      setInstallBlock(result.installBlock);
      setDialogOpen(false);
      setForm({
        node_group_id: "",
        tenant_id: "",
        name: "",
      });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["nodes"] }),
        queryClient.invalidateQueries({ queryKey: ["node-groups"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const updateNodeMutation = useMutation({
    mutationFn: () =>
      api.updateNodeGroup(form.node_group_id, {
        tenant_id: form.tenant_id,
        name: form.name.trim(),
      }),
    onSuccess: async (group) => {
      setError(null);
      setMessage(`Сервер ${group.name} обновлён.`);
      setDialogOpen(false);
      setForm({
        node_group_id: "",
        tenant_id: "",
        name: "",
      });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["nodes"] }),
        queryClient.invalidateQueries({ queryKey: ["node-groups"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const deleteNodeMutation = useMutation({
    mutationFn: (target: ServerCard) => api.deleteNodeGroup(target.id, target.tenant_id),
    onSuccess: async () => {
      setError(null);
      setMessage("Сервер удалён.");
      setDeleteTarget(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["nodes"] }),
        queryClient.invalidateQueries({ queryKey: ["node-groups"] }),
        queryClient.invalidateQueries({ queryKey: ["node-group-domains"] }),
        queryClient.invalidateQueries({ queryKey: ["server-endpoints"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  function openCreateDialog() {
    setDialogMode("create");
    setInstallBlock(null);
    setForm({
      node_group_id: "",
      tenant_id: "",
      name: "",
    });
    setDialogOpen(true);
  }

  function openEditDialog(server: ServerCard) {
    setDialogMode("edit");
    setInstallBlock(null);
    setForm({
      node_group_id: server.id,
      tenant_id: server.tenant_id,
      name: server.name,
    });
    setDialogOpen(true);
  }

  function submitDialog() {
    if (dialogMode === "create") {
      createNodeMutation.mutate();
      return;
    }
    updateNodeMutation.mutate();
  }

  if (!session.accessToken) {
    return <AuthRequired title={t("nodes.unauthorized")} />;
  }

  const onlineServers = servers.filter((server) => overallStatus(server) === "online").length;
  const pendingServers = servers.filter((server) => overallStatus(server) === "pending").length;

  return (
    <div className="space-y-8">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
            {t("nav_group.infrastructure")}
          </div>
          <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("nodes.title")}</h1>
          <p className="mt-3 max-w-4xl text-base text-[#485644]">
            {t("nodes.subtitle")}
          </p>
        </div>
        <Button onClick={openCreateDialog} type="button">
          {t("nodes.groups.create")}
        </Button>
      </div>

      {message ? <div className="text-sm text-emerald-700">{message}</div> : null}
      {error ? <div className="text-sm text-danger">{error}</div> : null}

      {installBlock ? (
        <Card className="space-y-4">
          <div>
            <div className="text-xs uppercase tracking-[0.25em] text-foreground/80">Install</div>
            <h2 className="mt-3 text-2xl font-semibold">Готовый install-блок</h2>
          </div>
          <Textarea className="min-h-48 font-mono text-xs" readOnly value={installBlock} />
        </Card>
      ) : null}

      <div className="grid gap-4 md:grid-cols-3">
        <MetricCard
          label={t("nodes.stat.total")}
          value={servers.length.toString()}
          hint={t("nodes.stat.total_hint")}
        />
        <MetricCard
          label={t("nodes.stat.online")}
          value={onlineServers.toString()}
          hint={t("nodes.stat.online_hint")}
        />
        <MetricCard
          label={t("nodes.stat.pending")}
          value={pendingServers.toString()}
          hint={t("nodes.stat.pending_hint")}
        />
      </div>

      <Card className="space-y-4 shadow-sm">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
          <div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("nodes.list.title")}</h2>
          </div>
          <div className="rounded-2xl bg-muted px-4 py-3 text-sm text-foreground/80">
            Всего: {servers.length}
          </div>
        </div>

        {servers.length > 0 ? (
          <div className="space-y-3">
            {servers.map((server) => {
              const status = overallStatus(server);

              return (
                <div key={server.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                  <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-3">
                        <div className="text-lg font-semibold">{server.name}</div>
                        <Badge
                          tone={
                            status === "online"
                              ? "success"
                              : status === "pending"
                                ? "warning"
                                : "danger"
                          }
                        >
                          {formatNodeStatus(status)}
                        </Badge>
                      </div>
                      <div className="mt-2 text-sm text-foreground/80">
                        Тенант: {tenantLabel(server.tenant_id, tenantNames)}
                      </div>
                      <div className="mt-1 font-mono text-xs text-foreground/90">
                        {server.id}
                      </div>

                      <div className="mt-4 grid gap-3 md:grid-cols-2">
                        {(["xray", "singbox"] as const).map((engine) => {
                          const runtime = server.runtimes[engine];
                          return (
                            <div
                              key={engine}
                              className="rounded-[22px] border border-border bg-card px-4 py-3"
                            >
                              <div className="flex items-center justify-between gap-3">
                                <div className="text-sm font-semibold">{engine}</div>
                                <Badge tone={runtimeTone(runtime)}>{runtimeLabel(runtime)}</Badge>
                              </div>
                              <div className="mt-2 text-sm text-foreground/80">
                                {runtime ? `version ${runtime.version}` : "runtime ещё не зарегистрирован"}
                              </div>
                              <div className="mt-2 text-xs text-foreground/90">
                                Последний сигнал: {formatDate(runtime?.last_seen_at ?? null)}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    </div>

                    <div className="flex flex-wrap gap-3">
                      <Button type="button" variant="secondary" onClick={() => openEditDialog(server)}>
                        Редактировать
                      </Button>
                      <Button type="button" variant="danger" onClick={() => setDeleteTarget(server)}>
                        Удалить
                      </Button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-8 text-center text-sm text-[#485644]">
            {t("dashboard.nodes.empty")}
          </div>
        )}
      </Card>

      <Dialog
        open={dialogOpen}
        onClose={() => setDialogOpen(false)}
        title={dialogMode === "create" ? "Новая серверная нода" : "Редактирование ноды"}
        description={
          dialogMode === "create"
            ? "Панель создаст серверную группу и сразу подготовит install-блок для обоих runtime."
            : "Измени название серверной ноды. Привязка к тенанту остаётся прежней."
        }
      >
        <form
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            submitDialog();
          }}
        >
          {dialogMode === "create" ? (
            <Select
              value={form.tenant_id}
              onChange={(event) =>
                setForm((current) => ({ ...current, tenant_id: event.target.value }))
              }
            >
              <option value="">Выбери тенант</option>
              {tenantOptions.map((tenant) => (
                <option key={tenant.tenant_id} value={tenant.tenant_id}>
                  {tenant.label}
                </option>
              ))}
            </Select>
          ) : (
            <div className="rounded-[24px] bg-[#f2efe4] px-4 py-3 text-sm text-foreground/90">
              Тенант: {tenantLabel(form.tenant_id, tenantNames)}
            </div>
          )}

          <Input
            placeholder="Имя VPS/VDS сервера"
            value={form.name}
            onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))}
          />

          <div className="rounded-[24px] bg-[#f2efe4] p-4 text-sm text-foreground/90">
            <div>Одна серверная нода получает сразу два runtime: xray и singbox.</div>
            {dialogMode === "create" ? (
              <div className="mt-1">После создания можно сразу запускать готовый install-блок.</div>
            ) : (
              <div className="mt-1">Доменные правила и точки входа сохранятся за этой нодой.</div>
            )}
          </div>

          <div className="flex justify-end gap-3">
            <Button type="button" variant="secondary" onClick={() => setDialogOpen(false)}>
              Отмена
            </Button>
            <Button
              disabled={
                createNodeMutation.isPending ||
                updateNodeMutation.isPending ||
                !form.name.trim() ||
                !form.tenant_id
              }
              type="submit"
            >
              {dialogMode === "create"
                ? createNodeMutation.isPending
                  ? "Создаю..."
                  : "Создать"
                : updateNodeMutation.isPending
                  ? "Сохраняю..."
                  : "Сохранить"}
            </Button>
          </div>
        </form>
      </Dialog>

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        title="Удалить ноду"
        description={
          deleteTarget
            ? `Нода ${deleteTarget.name} будет удалена вместе с runtime, endpoint-ами и связанными данными.`
            : ""
        }
        confirmLabel="Удалить"
        pendingLabel="Удаляю..."
        isPending={deleteNodeMutation.isPending}
        onConfirm={() => {
          if (deleteTarget) {
            deleteNodeMutation.mutate(deleteTarget);
          }
        }}
      />
    </div>
  );
}
