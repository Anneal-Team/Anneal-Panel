import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { AuthRequired } from "@/components/auth-required";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import {
  api,
  type Node,
  type NodeDomain,
  type NodeEndpoint,
  type NodeDomainMode,
  type NodeRuntime,
  type ProxyEngine,
} from "@/lib/api";
import { formatDate, formatNodeName, formatNodeStatus } from "@/lib/format";

type DomainDraft = {
  id: string;
  mode: NodeDomainMode;
  domain: string;
  alias: string;
  server_names_text: string;
  host_headers_text: string;
};

type GeneratedPreviewItem = {
  engine: ProxyEngine;
  runtime_id: string;
  tenant_id: string;
  endpoint: NodeEndpoint;
};

type ModeMeta = {
  title: string;
  description: string;
  generated: string[];
  domainHint: string;
  aliasHint: string;
  serverNameHint: string;
  hostHeaderHint: string;
};

const modeOrder: NodeDomainMode[] = [
  "direct",
  "legacy_direct",
  "cdn",
  "auto_cdn",
  "relay",
  "worker",
  "reality",
  "fake",
];

const modeGenerated: Record<NodeDomainMode, string[]> = {
  direct: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2", "gRPC"],
  legacy_direct: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2"],
  cdn: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "gRPC"],
  auto_cdn: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "gRPC"],
  relay: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2", "gRPC"],
  worker: ["VLESS WS", "VLESS HTTP Upgrade", "Trojan WS", "VMess WS"],
  reality: ["VLESS Reality"],
  fake: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2", "gRPC"],
};

function buildModeMeta(t: (key: string) => string): Record<NodeDomainMode, ModeMeta> {
  return {
    direct: {
      title: t("node_endpoints.modes.direct.title"),
      description: t("node_endpoints.modes.direct.description"),
      generated: modeGenerated.direct,
      domainHint: t("node_endpoints.modes.direct.domain_hint"),
      aliasHint: t("node_endpoints.modes.direct.alias_hint"),
      serverNameHint: t("node_endpoints.modes.direct.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.direct.host_header_hint"),
    },
    legacy_direct: {
      title: t("node_endpoints.modes.legacy_direct.title"),
      description: t("node_endpoints.modes.legacy_direct.description"),
      generated: modeGenerated.legacy_direct,
      domainHint: t("node_endpoints.modes.legacy_direct.domain_hint"),
      aliasHint: t("node_endpoints.modes.legacy_direct.alias_hint"),
      serverNameHint: t("node_endpoints.modes.legacy_direct.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.legacy_direct.host_header_hint"),
    },
    cdn: {
      title: t("node_endpoints.modes.cdn.title"),
      description: t("node_endpoints.modes.cdn.description"),
      generated: modeGenerated.cdn,
      domainHint: t("node_endpoints.modes.cdn.domain_hint"),
      aliasHint: t("node_endpoints.modes.cdn.alias_hint"),
      serverNameHint: t("node_endpoints.modes.cdn.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.cdn.host_header_hint"),
    },
    auto_cdn: {
      title: t("node_endpoints.modes.auto_cdn.title"),
      description: t("node_endpoints.modes.auto_cdn.description"),
      generated: [...modeGenerated.auto_cdn, t("node_endpoints.modes.auto_cdn.generated_auto_ip")],
      domainHint: t("node_endpoints.modes.auto_cdn.domain_hint"),
      aliasHint: t("node_endpoints.modes.auto_cdn.alias_hint"),
      serverNameHint: t("node_endpoints.modes.auto_cdn.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.auto_cdn.host_header_hint"),
    },
    relay: {
      title: t("node_endpoints.modes.relay.title"),
      description: t("node_endpoints.modes.relay.description"),
      generated: modeGenerated.relay,
      domainHint: t("node_endpoints.modes.relay.domain_hint"),
      aliasHint: t("node_endpoints.modes.relay.alias_hint"),
      serverNameHint: t("node_endpoints.modes.relay.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.relay.host_header_hint"),
    },
    worker: {
      title: t("node_endpoints.modes.worker.title"),
      description: t("node_endpoints.modes.worker.description"),
      generated: modeGenerated.worker,
      domainHint: t("node_endpoints.modes.worker.domain_hint"),
      aliasHint: t("node_endpoints.modes.worker.alias_hint"),
      serverNameHint: t("node_endpoints.modes.worker.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.worker.host_header_hint"),
    },
    reality: {
      title: t("node_endpoints.modes.reality.title"),
      description: t("node_endpoints.modes.reality.description"),
      generated: [...modeGenerated.reality, t("node_endpoints.modes.reality.generated_per_sni")],
      domainHint: t("node_endpoints.modes.reality.domain_hint"),
      aliasHint: t("node_endpoints.modes.reality.alias_hint"),
      serverNameHint: t("node_endpoints.modes.reality.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.reality.host_header_hint"),
    },
    fake: {
      title: t("node_endpoints.modes.fake.title"),
      description: t("node_endpoints.modes.fake.description"),
      generated: modeGenerated.fake,
      domainHint: t("node_endpoints.modes.fake.domain_hint"),
      aliasHint: t("node_endpoints.modes.fake.alias_hint"),
      serverNameHint: t("node_endpoints.modes.fake.server_name_hint"),
      hostHeaderHint: t("node_endpoints.modes.fake.host_header_hint"),
    },
  };
}

const protocolTitle: Record<NodeEndpoint["protocol"], string> = {
  vless_reality: "VLESS",
  vmess: "VMess",
  trojan: "Trojan",
  shadowsocks_2022: "Shadowsocks 2022",
  tuic: "TUIC",
  hysteria2: "Hysteria2",
};

const transportTitle: Record<NodeEndpoint["transport"], string> = {
  tcp: "TCP",
  ws: "WS",
  grpc: "gRPC",
  http_upgrade: "HTTP Upgrade",
};

function draftId() {
  return globalThis.crypto.randomUUID();
}

function createDraft(mode: NodeDomainMode = "direct"): DomainDraft {
  return {
    id: draftId(),
    mode,
    domain: "",
    alias: "",
    server_names_text: "",
    host_headers_text: "",
  };
}

function createStarterPack(): DomainDraft[] {
  return modeOrder.map((mode) => createDraft(mode));
}

function draftFromDomain(domain: NodeDomain): DomainDraft {
  return {
    id: domain.id,
    mode: domain.mode,
    domain: domain.domain,
    alias: domain.alias ?? "",
    server_names_text: domain.server_names.join("\n"),
    host_headers_text: domain.host_headers.join("\n"),
  };
}

function formFromDomains(domains: NodeDomain[] | undefined) {
  if (!domains || domains.length === 0) {
    return createStarterPack();
  }
  return domains.map(draftFromDomain);
}

function splitLines(value: string) {
  const normalized = value
    .split(/[\r\n,]+/)
    .map((item) => item.trim())
    .filter(Boolean);
  return normalized.filter((item, index) => normalized.indexOf(item) === index);
}

function runtimeByEngine(node: Node, engine: ProxyEngine) {
  return node.runtimes.find((runtime) => runtime.engine === engine);
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

function endpointInput(endpoint: NodeEndpoint) {
  return {
    protocol: endpoint.protocol,
    listen_host: endpoint.listen_host,
    listen_port: endpoint.listen_port,
    public_host: endpoint.public_host,
    public_port: endpoint.public_port,
    transport: endpoint.transport,
    security: endpoint.security,
    server_name: endpoint.server_name,
    host_header: endpoint.host_header,
    path: endpoint.path,
    service_name: endpoint.service_name,
    flow: endpoint.flow,
    fingerprint: endpoint.fingerprint,
    alpn: endpoint.alpn,
    cipher: endpoint.cipher,
    tls_certificate_path: endpoint.tls_certificate_path,
    tls_key_path: endpoint.tls_key_path,
    enabled: endpoint.enabled,
  };
}

function endpointCaption(item: GeneratedPreviewItem) {
  return `${protocolTitle[item.endpoint.protocol]} / ${item.engine} / ${transportTitle[item.endpoint.transport]}`;
}

function getModeInfo(meta: Record<NodeDomainMode, ModeMeta>, mode: NodeDomainMode) {
  switch (mode) {
    case "legacy_direct":
      return meta.legacy_direct;
    case "cdn":
      return meta.cdn;
    case "auto_cdn":
      return meta.auto_cdn;
    case "relay":
      return meta.relay;
    case "worker":
      return meta.worker;
    case "reality":
      return meta.reality;
    case "fake":
      return meta.fake;
    case "direct":
    default:
      return meta.direct;
  }
}

function getEngineEndpoints(
  data: Partial<Record<ProxyEngine, NodeEndpoint[]>> | undefined,
  engine: ProxyEngine,
) {
  if (!data) {
    return [];
  }
  switch (engine) {
    case "singbox":
      return data.singbox ?? [];
    case "xray":
    default:
      return data.xray ?? [];
  }
}

function ruleTitle(draft: DomainDraft, fallback: string) {
  if (draft.domain.trim()) {
    return draft.domain.trim();
  }
  if (draft.alias.trim()) {
    return draft.alias.trim();
  }
  return fallback;
}

function ruleSubtitle(draft: DomainDraft, modeMeta: Record<NodeDomainMode, ModeMeta>) {
  const parts = [getModeInfo(modeMeta, draft.mode).title];
  if (draft.alias.trim()) {
    parts.push(draft.alias.trim());
  }
  return parts.join(" / ");
}

export function NodeEndpointsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = api.readSession();
  const modeMeta = useMemo(() => buildModeMeta(t), [t]);
  const [selectedServerId, setSelectedServerId] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [form, setForm] = useState<DomainDraft[]>(createStarterPack);
  const [selectedDraftId, setSelectedDraftId] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const nodesQuery = useQuery({
    queryKey: ["nodes"],
    queryFn: api.listNodes,
    enabled: Boolean(session.accessToken),
  });

  const servers = useMemo(
    () => [...(nodesQuery.data ?? [])].sort((left, right) => left.name.localeCompare(right.name)),
    [nodesQuery.data],
  );

  const selectedServer = useMemo(
    () => servers.find((server) => server.id === selectedServerId) ?? servers[0] ?? null,
    [selectedServerId, servers],
  );

  const selectedDraft = useMemo(
    () => form.find((draft) => draft.id === selectedDraftId) || form[0] || null,
    [form, selectedDraftId],
  );

  useEffect(() => {
    if (!selectedServerId && servers[0]) {
      setSelectedServerId(servers[0].id);
    }
  }, [selectedServerId, servers]);

  useEffect(() => {
    if (!selectedDraftId || !form.some((draft) => draft.id === selectedDraftId)) {
      setSelectedDraftId(form[0]?.id ?? "");
    }
  }, [form, selectedDraftId]);

  const domainsQuery = useQuery({
    queryKey: ["node-domains", selectedServer?.id],
    enabled: Boolean(session.accessToken && selectedServer),
    queryFn: () => {
      if (!selectedServer) {
        throw new Error(t("node_endpoints.error.server_required"));
      }
      return api.listNodeDomains(selectedServer.id, selectedServer.tenant_id);
    },
  });

  const endpointsQuery = useQuery({
    queryKey: [
      "server-endpoints",
      selectedServer?.id,
      selectedServer ? runtimeByEngine(selectedServer, "xray")?.id : null,
      selectedServer ? runtimeByEngine(selectedServer, "singbox")?.id : null,
    ],
    enabled: Boolean(session.accessToken && selectedServer),
    queryFn: async () => {
      if (!selectedServer) {
        return {} as Partial<Record<ProxyEngine, NodeEndpoint[]>>;
      }
      const entries = await Promise.all(
        (["xray", "singbox"] as const)
          .filter((engine) => runtimeByEngine(selectedServer, engine))
          .map(async (engine) => {
            const runtime = runtimeByEngine(selectedServer, engine);
            if (!runtime) {
              throw new Error(t("node_endpoints.runtime_missing"));
            }
            const endpoints = await api.listNodeRuntimeEndpoints(runtime.id, runtime.tenant_id);
            return [engine, endpoints] as const;
          }),
      );
      return Object.fromEntries(entries) as Partial<Record<ProxyEngine, NodeEndpoint[]>>;
    },
  });

  useEffect(() => {
    const nextForm = formFromDomains(domainsQuery.data);
    setForm(nextForm);
    setSelectedDraftId(nextForm[0]?.id ?? "");
  }, [domainsQuery.data, selectedServer?.id]);

  const saveMutation = useMutation({
    mutationFn: async () => {
      if (!selectedServer) {
        throw new Error(t("node_endpoints.error.server_required"));
      }
      const domains = form
        .filter((draft) => draft.domain.trim())
        .map((draft) => ({
          mode: draft.mode,
          domain: draft.domain.trim(),
          alias: draft.alias.trim() || null,
          server_names: splitLines(draft.server_names_text),
          host_headers: splitLines(draft.host_headers_text),
        }));
      return api.replaceNodeDomains(selectedServer.id, {
        tenant_id: selectedServer.tenant_id,
        domains,
      });
    },
    onSuccess: async (domains) => {
      setError(null);
      setMessage(t("node_endpoints.saved", { count: domains.length }));
      setSettingsOpen(false);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["node-domains"] }),
        queryClient.invalidateQueries({ queryKey: ["server-endpoints"] }),
        queryClient.invalidateQueries({ queryKey: ["rollouts"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const toggleEndpointMutation = useMutation({
    mutationFn: async ({
      runtimeId,
      tenantId,
      endpoints,
    }: {
      runtimeId: string;
      tenantId: string;
      endpoints: NodeEndpoint[];
    }) =>
      api.replaceNodeRuntimeEndpoints(runtimeId, {
        tenant_id: tenantId,
        endpoints: endpoints.map(endpointInput),
    }),
    onSuccess: async () => {
      setError(null);
      setMessage(t("node_endpoints.toggled"));
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["server-endpoints"] }),
        queryClient.invalidateQueries({ queryKey: ["rollouts"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const generatedPreview = useMemo(() => {
    const items: GeneratedPreviewItem[] = [];
    for (const engine of ["xray", "singbox"] as const) {
      const runtime = selectedServer ? runtimeByEngine(selectedServer, engine) : undefined;
      if (!runtime) {
        continue;
      }
      const engineEndpoints = getEngineEndpoints(endpointsQuery.data, engine);
      for (const endpoint of engineEndpoints) {
        items.push({
          engine,
          runtime_id: runtime.id,
          tenant_id: runtime.tenant_id,
          endpoint,
        });
      }
    }
    return items.sort((left, right) => {
      const leftKey = `${left.endpoint.public_host}-${left.endpoint.public_port}-${left.endpoint.protocol}-${left.engine}`;
      const rightKey = `${right.endpoint.public_host}-${right.endpoint.public_port}-${right.endpoint.protocol}-${right.engine}`;
      return leftKey.localeCompare(rightKey);
    });
  }, [endpointsQuery.data, selectedServer]);

  function updateDraft<K extends keyof DomainDraft>(id: string, key: K, value: DomainDraft[K]) {
    setForm((current) => current.map((draft) => (draft.id === id ? { ...draft, [key]: value } : draft)));
  }

  function addDraft(mode: NodeDomainMode = selectedDraft?.mode || "direct") {
    const next = createDraft(mode);
    setForm((current) => [...current, next]);
    setSelectedDraftId(next.id);
  }

  function removeDraft(id: string) {
    setForm((current) => current.filter((draft) => draft.id !== id));
  }

  function resetPreset() {
    const next = createStarterPack();
    setForm(next);
    setSelectedDraftId(next[0]?.id ?? "");
  }

  function toggleEndpoint(item: GeneratedPreviewItem) {
    const current = endpointsQuery.data?.[item.engine] ?? [];
    const next = current.map((endpoint) =>
      endpoint.id === item.endpoint.id ? { ...endpoint, enabled: !endpoint.enabled } : endpoint,
    );
    toggleEndpointMutation.mutate({
      runtimeId: item.runtime_id,
      tenantId: item.tenant_id,
      endpoints: next,
    });
  }

  if (!session.accessToken) {
    return <AuthRequired title={t("node_endpoints.unauthorized")} />;
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
          {t("nav_group.infrastructure")}
        </div>
        <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("node_endpoints.title")}</h1>
        <p className="mt-3 max-w-4xl text-base text-[#485644]">{t("node_endpoints.subtitle")}</p>
      </div>

      {message ? <div className="text-sm text-emerald-700">{message}</div> : null}
      {error ? <div className="text-sm text-danger">{error}</div> : null}

      <Card className="space-y-5 shadow-sm">
        <div className="grid gap-3 xl:grid-cols-[1.1fr_auto]">
          <Select
            value={selectedServerId}
            onChange={(event) => {
              setSelectedServerId(event.target.value);
            }}
          >
            <option value="">{t("node_endpoints.select_server")}</option>
            {servers.map((server) => (
              <option key={server.id} value={server.id}>
                {formatNodeName(server.name)} / {server.tenant_id}
              </option>
            ))}
          </Select>
          <Button
            type="button"
            disabled={!selectedServer}
            onClick={() => {
              setSettingsOpen(true);
            }}
          >
            {t("node_endpoints.configure")}
          </Button>
        </div>

        {selectedServer ? (
          <div className="grid gap-3 md:grid-cols-2">
            {(["xray", "singbox"] as const).map((engine) => {
              const runtime = runtimeByEngine(selectedServer, engine);
              return (
                <div key={engine} className="rounded-[22px] border border-border bg-[#f8f5f0] px-4 py-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="text-sm font-semibold">{engine}</div>
                    <Badge tone={runtimeTone(runtime)}>
                      {runtimeLabel(runtime, t("node_endpoints.runtime_missing"))}
                    </Badge>
                  </div>
                  <div className="mt-2 text-sm text-foreground/80">
                    {runtime
                      ? t("node_endpoints.runtime_version", { version: runtime.version })
                      : t("node_endpoints.runtime_missing")}
                  </div>
                  <div className="mt-2 text-xs text-foreground/90">
                    {t("node_endpoints.runtime_last_seen")}: {formatDate(runtime?.last_seen_at ?? null)}
                  </div>
                </div>
              );
            })}
          </div>
        ) : null}

        {selectedServer ? (
          domainsQuery.data && domainsQuery.data.length > 0 ? (
            <div className="flex flex-wrap gap-2">
              {domainsQuery.data.map((domain) => (
                <Badge key={domain.id} tone="muted">
                  {getModeInfo(modeMeta, domain.mode).title} / {domain.domain}
                </Badge>
              ))}
            </div>
          ) : (
            <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-6 text-sm text-foreground/80">
              {t("node_endpoints.domains_empty")}
            </div>
          )
        ) : (
          <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-6 text-sm text-foreground/80">
            {t("node_endpoints.server_required")}
          </div>
        )}
      </Card>

      <Card className="space-y-4 shadow-sm">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("node_endpoints.generated_title")}</h2>
          </div>
          <div className="text-sm text-foreground/80">
            {t("common.total")}: {generatedPreview.length}
          </div>
        </div>

        {generatedPreview.length > 0 ? (
          <div className="grid gap-4 xl:grid-cols-2">
            {generatedPreview.map((item) => (
              <div key={`${item.engine}-${item.endpoint.id}`} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <div className="text-lg font-semibold">{endpointCaption(item)}</div>
                    <div className="mt-1 text-sm text-foreground/80">
                      {item.endpoint.public_host}:{item.endpoint.public_port}
                    </div>
                  </div>
                  <div className="flex items-center gap-3">
                    <Badge tone={item.endpoint.enabled ? "success" : "muted"}>
                      {item.endpoint.enabled ? t("common.enabled") : t("common.disabled")}
                    </Badge>
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={toggleEndpointMutation.isPending}
                      onClick={() => {
                        toggleEndpoint(item);
                      }}
                    >
                      {item.endpoint.enabled ? t("common.turn_off") : t("common.turn_on")}
                    </Button>
                  </div>
                </div>

                <div className="mt-4 grid gap-2 text-sm text-foreground/90">
                  <div>{t("node_endpoints.endpoint_security")}: {item.endpoint.security}</div>
                  <div>SNI: {item.endpoint.server_name ?? "вЂ”"}</div>
                  <div>Host: {item.endpoint.host_header ?? "вЂ”"}</div>
                  <div>Path: {item.endpoint.path ?? "вЂ”"}</div>
                  <div>{t("node_endpoints.service_name")}: {item.endpoint.service_name ?? "вЂ”"}</div>
                  <div>ALPN: {item.endpoint.alpn.join(", ") || "вЂ”"}</div>
                  <div>
                    {t("node_endpoints.endpoint_reality_key")}:{" "}
                    {item.endpoint.reality_public_key
                      ? t("node_endpoints.endpoint_reality_ready")
                      : t("node_endpoints.endpoint_reality_unused")}
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-8 text-sm text-foreground/80">
            {selectedServer ? t("node_endpoints.generated_empty") : t("node_endpoints.server_required")}
          </div>
        )}
      </Card>

      <Dialog
        open={settingsOpen}
        onClose={() => {
          setSettingsOpen(false);
        }}
        title={
          selectedServer
            ? t("node_endpoints.dialog.title", { name: formatNodeName(selectedServer.name) })
            : t("node_endpoints.dialog.title_empty")
        }
        description={t("node_endpoints.dialog.description")}
        className="max-w-6xl"
      >
        <div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
          <div className="space-y-4">
            <div className="flex gap-3">
              <Button
                className="flex-1"
                type="button"
                variant="secondary"
                onClick={() => {
                  addDraft();
                }}
              >
                {t("node_endpoints.dialog.add_rule")}
              </Button>
              <Button className="flex-1" type="button" variant="secondary" onClick={resetPreset}>
                {t("node_endpoints.dialog.reset_preset")}
              </Button>
            </div>

            <div className="rounded-[28px] border border-border bg-[#f8f5f0] p-3">
              <div className="px-2 pb-3 text-xs uppercase tracking-[0.24em] text-foreground/80">
                {t("node_endpoints.dialog.rules")}
              </div>
              <div className="max-h-[58vh] space-y-2 overflow-y-auto pr-1">
                {form.map((draft, index) => {
                  const active = selectedDraft.id === draft.id;
                  return (
                    <button
                      key={draft.id}
                      type="button"
                      onClick={() => {
                        setSelectedDraftId(draft.id);
                      }}
                      className={`w-full rounded-[22px] border px-4 py-3 text-left transition ${
                        active
                          ? "border-accent bg-accent/10 text-foreground"
                          : "border-border bg-card/70 text-foreground/75 hover:bg-muted"
                      }`}
                    >
                      <div className="text-xs uppercase tracking-[0.2em] text-foreground/80">#{index + 1}</div>
                      <div className="mt-2 text-sm font-semibold">
                        {ruleTitle(draft, t("node_endpoints.rule_new"))}
                      </div>
                      <div className="mt-1 text-xs text-foreground/80">{ruleSubtitle(draft, modeMeta)}</div>
                    </button>
                  );
                })}
              </div>
            </div>
          </div>

          {selectedDraft !== null ? (
            <div className="space-y-5">
              <div className="rounded-[28px] border border-border bg-[#f8f5f0] p-5">
                <div className="text-xs uppercase tracking-[0.24em] text-foreground/80">
                  {t("node_endpoints.dialog.mode")}
                </div>
                <Select
                  className="mt-3"
                  value={selectedDraft.mode}
                  onChange={(event) => {
                    updateDraft(selectedDraft.id, "mode", event.target.value as NodeDomainMode);
                  }}
                >
                  {modeOrder.map((mode) => (
                    <option key={mode} value={mode}>
                      {getModeInfo(modeMeta, mode).title}
                    </option>
                  ))}
                </Select>

                <div className="mt-4 rounded-[24px] bg-[#f2efe4] p-4 text-sm text-foreground/75">
                  <div className="font-semibold text-foreground">{getModeInfo(modeMeta, selectedDraft.mode).title}</div>
                  <div className="mt-2">{getModeInfo(modeMeta, selectedDraft.mode).description}</div>
                  <div className="mt-4 flex flex-wrap gap-2">
                    {getModeInfo(modeMeta, selectedDraft.mode).generated.map((item) => (
                      <Badge key={item} tone="muted">
                        {item}
                      </Badge>
                    ))}
                  </div>
                </div>

                <div className="mt-4 grid gap-2 text-sm text-foreground/90">
                  {modeOrder.map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      className={`rounded-[18px] px-3 py-2 text-left transition ${
                        selectedDraft.mode === mode ? "bg-accent/10 text-foreground" : "hover:bg-[#f2efe4]"
                      }`}
                      onClick={() => {
                        updateDraft(selectedDraft.id, "mode", mode);
                      }}
                    >
                      <span className="font-semibold">{getModeInfo(modeMeta, mode).title}:</span>{" "}
                      {getModeInfo(modeMeta, mode).description}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 xl:grid-cols-2">
                <div className="space-y-2">
                  <div className="text-sm font-semibold">{t("node_endpoints.domain")}</div>
                  <Input
                    placeholder="example.com"
                    value={selectedDraft.domain}
                    onChange={(event) => {
                      updateDraft(selectedDraft.id, "domain", event.target.value);
                    }}
                  />
                  <div className="text-sm text-foreground/80">{getModeInfo(modeMeta, selectedDraft.mode).domainHint}</div>
                </div>

                <div className="space-y-2">
                  <div className="text-sm font-semibold">{t("node_endpoints.alias")}</div>
                  <Input
                    placeholder="main"
                    value={selectedDraft.alias}
                    onChange={(event) => {
                      updateDraft(selectedDraft.id, "alias", event.target.value);
                    }}
                  />
                  <div className="text-sm text-foreground/80">{getModeInfo(modeMeta, selectedDraft.mode).aliasHint}</div>
                </div>
              </div>

              <div className="grid gap-4 xl:grid-cols-2">
                <div className="space-y-2">
                  <div className="text-sm font-semibold">{t("node_endpoints.server_names")}</div>
                  <Textarea
                    className="min-h-40"
                    placeholder={"example.com\ncdn.example.com"}
                    value={selectedDraft.server_names_text}
                    onChange={(event) => {
                      updateDraft(selectedDraft.id, "server_names_text", event.target.value);
                    }}
                  />
                  <div className="text-sm text-foreground/80">{getModeInfo(modeMeta, selectedDraft.mode).serverNameHint}</div>
                </div>

                <div className="space-y-2">
                  <div className="text-sm font-semibold">Host header</div>
                  <Textarea
                    className="min-h-40"
                    placeholder={"example.com\nworker.example.com"}
                    value={selectedDraft.host_headers_text}
                    onChange={(event) => {
                      updateDraft(selectedDraft.id, "host_headers_text", event.target.value);
                    }}
                  />
                  <div className="text-sm text-foreground/80">{getModeInfo(modeMeta, selectedDraft.mode).hostHeaderHint}</div>
                </div>
              </div>

              <div className="flex items-center justify-between gap-3 rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-4 py-4">
                <div className="text-sm text-foreground/80">{t("node_endpoints.rule_help")}</div>
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => {
                    removeDraft(selectedDraft.id);
                  }}
                  disabled={form.length === 0}
                >
                  {t("node_endpoints.rule_delete")}
                </Button>
              </div>

              <div className="flex justify-end gap-3">
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => {
                    setSettingsOpen(false);
                  }}
                >
                  {t("common.cancel")}
                </Button>
                <Button
                  type="button"
                  disabled={saveMutation.isPending || !selectedServer}
                  onClick={() => {
                    saveMutation.mutate();
                  }}
                >
                  {saveMutation.isPending ? t("node_endpoints.saving") : t("common.save")}
                </Button>
              </div>
            </div>
          ) : (
            <div className="rounded-[28px] border border-dashed border-border bg-[#f8f5f0] px-5 py-12 text-sm text-foreground/80">
              {t("node_endpoints.rule_empty")}
            </div>
          )}
        </div>
      </Dialog>
    </div>
  );
}

