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
import { api, type Node, type NodeRuntime, type ProxyEngine } from "@/lib/api";
import { formatDate, formatNodeName, formatNodeStatus } from "@/lib/format";

const runtimeProtocols: Record<ProxyEngine, string> = {
  xray: "vless_reality,vmess,trojan,shadowsocks_2022",
  singbox: "vless_reality,vmess,trojan,shadowsocks_2022,tuic,hysteria2",
};

type NodeDialogMode = "create" | "edit";

function buildInstallBlock(name: string, bootstrapToken: string) {
  return [
    `ANNEAL_AGENT_SERVER_URL=${window.location.origin}`,
    `ANNEAL_AGENT_NAME=${name}`,
    `ANNEAL_AGENT_BOOTSTRAP_TOKEN=${bootstrapToken}`,
    "ANNEAL_AGENT_ENGINES=xray,singbox",
    `ANNEAL_AGENT_PROTOCOLS_XRAY=${runtimeProtocols.xray}`,
    `ANNEAL_AGENT_PROTOCOLS_SINGBOX=${runtimeProtocols.singbox}`,
    "./install.sh --role node",
  ].join("\n");
}

function runtimeByEngine(node: Node, engine: ProxyEngine) {
  return node.runtimes.find((runtime) => runtime.engine === engine);
}

function overallStatus(node: Node) {
  const states = node.runtimes.map((runtime) => runtime.status);
  if (states.includes("online")) {
    return "online";
  }
  if (states.includes("pending")) {
    return "pending";
  }
  return "offline";
}

function runtimeTone(runtime: NodeRuntime | undefined) {
  if (!runtime) {
    return "muted" as const;
  }
  if (runtime.status === "online") {
    return "success" as const;
  }
  if (runtime.status === "pending") {
    return "warning" as const;
  }
  return "danger" as const;
}

function runtimeLabel(runtime: NodeRuntime | undefined, missingLabel: string) {
  if (!runtime) {
    return missingLabel;
  }
  return formatNodeStatus(runtime.status);
}

function tenantLabel(tenantId: string, tenantNames: Map<string, string>) {
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
  const [deleteTarget, setDeleteTarget] = useState<Node | null>(null);
  const [installBlock, setInstallBlock] = useState<string | null>(null);
  const [form, setForm] = useState({
    node_id: "",
    tenant_id: "",
    name: "",
  });

  const resellersQuery = useQuery({
    queryKey: ["resellers"],
    queryFn: api.listResellers,
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

  const nodes = useMemo(
    () => [...(nodesQuery.data ?? [])].sort((left, right) => left.name.localeCompare(right.name)),
    [nodesQuery.data],
  );

  const createNodeMutation = useMutation({
    mutationFn: async () => {
      const name = form.name.trim();
      const node = await api.createNode({
        tenant_id: form.tenant_id,
        name,
      });
      const bootstrap = await api.createBootstrapToken(node.id, {
        tenant_id: form.tenant_id,
        engines: ["xray", "singbox"],
      });
      return {
        installBlock: buildInstallBlock(name, bootstrap.bootstrap_token),
      };
    },
    onSuccess: async (result) => {
      setError(null);
      setMessage(null);
      setInstallBlock(result.installBlock);
      setDialogOpen(false);
      setForm({
        node_id: "",
        tenant_id: "",
        name: "",
      });
      await queryClient.invalidateQueries({ queryKey: ["nodes"] });
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const updateNodeMutation = useMutation({
    mutationFn: () =>
      api.updateNode(form.node_id, {
        tenant_id: form.tenant_id,
        name: form.name.trim(),
      }),
    onSuccess: async () => {
      setError(null);
      setMessage(null);
      setDialogOpen(false);
      setForm({
        node_id: "",
        tenant_id: "",
        name: "",
      });
      await queryClient.invalidateQueries({ queryKey: ["nodes"] });
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const deleteNodeMutation = useMutation({
    mutationFn: (target: Node) => api.deleteNode(target.id, target.tenant_id),
    onSuccess: async () => {
      setError(null);
      setMessage(null);
      setDeleteTarget(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["nodes"] }),
        queryClient.invalidateQueries({ queryKey: ["node-domains"] }),
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
      node_id: "",
      tenant_id: "",
      name: "",
    });
    setDialogOpen(true);
  }

  function openEditDialog(node: Node) {
    setDialogMode("edit");
    setInstallBlock(null);
    setForm({
      node_id: node.id,
      tenant_id: node.tenant_id,
      name: node.name,
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

  const onlineNodes = nodes.filter((node) => overallStatus(node) === "online").length;
  const pendingNodes = nodes.filter((node) => overallStatus(node) === "pending").length;

  return (
    <div className="space-y-8">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
            {t("nav_group.infrastructure")}
          </div>
          <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("nodes.title")}</h1>
          <p className="mt-3 max-w-4xl text-base text-[#485644]">{t("nodes.subtitle")}</p>
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
            <div className="text-xs uppercase tracking-[0.25em] text-foreground/80">
              {t("nodes.create.title")}
            </div>
            <h2 className="mt-3 text-2xl font-semibold">{t("nodes.create.token_hint")}</h2>
          </div>
          <Textarea className="min-h-48 font-mono text-xs" readOnly value={installBlock} />
        </Card>
      ) : null}

      <div className="grid gap-4 md:grid-cols-3">
        <MetricCard
          label={t("nodes.stat.total")}
          value={nodes.length.toString()}
          hint={t("nodes.stat.total_hint")}
        />
        <MetricCard
          label={t("nodes.stat.online")}
          value={onlineNodes.toString()}
          hint={t("nodes.stat.online_hint")}
        />
        <MetricCard
          label={t("nodes.stat.pending")}
          value={pendingNodes.toString()}
          hint={t("nodes.stat.pending_hint")}
        />
      </div>

      <Card className="space-y-4 shadow-sm">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
          <div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("nodes.list.title")}</h2>
          </div>
          <div className="rounded-2xl bg-muted px-4 py-3 text-sm text-foreground/80">
            {t("common.total")}: {nodes.length}
          </div>
        </div>

        {nodes.length > 0 ? (
          <div className="space-y-3">
            {nodes.map((node) => {
              const status = overallStatus(node);
              return (
                <div key={node.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                  <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-3">
                        <div className="text-lg font-semibold">{formatNodeName(node.name)}</div>
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
                        {tenantLabel(node.tenant_id, tenantNames)}
                      </div>
                      <div className="mt-1 font-mono text-xs text-foreground/90">{node.id}</div>

                      <div className="mt-4 grid gap-3 md:grid-cols-2">
                        {(["xray", "singbox"] as const).map((engine) => {
                          const runtime = runtimeByEngine(node, engine);
                          return (
                            <div
                              key={engine}
                              className="rounded-[22px] border border-border bg-card px-4 py-3"
                            >
                              <div className="flex items-center justify-between gap-3">
                                <div className="text-sm font-semibold">{engine}</div>
                                <Badge tone={runtimeTone(runtime)}>
                                  {runtimeLabel(runtime, t("nodes.runtime.missing"))}
                                </Badge>
                              </div>
                              <div className="mt-2 text-sm text-foreground/80">
                                {runtime ? runtime.version : t("nodes.runtime.missing")}
                              </div>
                              <div className="mt-2 text-xs text-foreground/90">
                                {t("nodes.list.last_seen")}: {formatDate(runtime?.last_seen_at ?? null)}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    </div>

                    <div className="flex flex-wrap gap-3">
                      <Button type="button" variant="secondary" onClick={() => openEditDialog(node)}>
                        {t("common.actions.edit")}
                      </Button>
                      <Button type="button" variant="danger" onClick={() => setDeleteTarget(node)}>
                        {t("common.actions.delete")}
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
        title={dialogMode === "create" ? t("nodes.groups.create") : t("common.actions.edit")}
        description={t("nodes.subtitle")}
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
              <option value="">{t("nodes.create.select_tenant")}</option>
              {tenantOptions.map((tenant) => (
                <option key={tenant.tenant_id} value={tenant.tenant_id}>
                  {tenant.label}
                </option>
              ))}
            </Select>
          ) : (
            <div className="rounded-[24px] bg-[#f2efe4] px-4 py-3 text-sm text-foreground/90">
              {tenantLabel(form.tenant_id, tenantNames)}
            </div>
          )}

          <Input
            placeholder={t("nodes.groups.name")}
            value={form.name}
            onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))}
          />

          <div className="flex justify-end gap-3">
            <Button type="button" variant="secondary" onClick={() => setDialogOpen(false)}>
              {t("common.cancel")}
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
                  ? t("nodes.groups.creating")
                  : t("common.actions.create")
                : updateNodeMutation.isPending
                  ? t("common.save")
                  : t("common.save")}
            </Button>
          </div>
        </form>
      </Dialog>

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        title={t("common.actions.delete")}
        description={deleteTarget?.name ?? ""}
        confirmLabel={t("common.actions.delete")}
        pendingLabel={t("subscriptions.delete.pending")}
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
