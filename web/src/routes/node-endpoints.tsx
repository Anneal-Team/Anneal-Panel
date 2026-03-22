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
  type NodeEndpoint,
  type NodeGroup,
  type NodeGroupDomain,
  type NodeGroupDomainMode,
  type ProxyEngine,
} from "@/lib/api";
import { formatDate, formatNodeStatus } from "@/lib/format";

type ServerNode = {
  id: string;
  tenant_id: string;
  name: string;
  runtimes: Partial<Record<ProxyEngine, Node>>;
};

type DomainDraft = {
  id: string;
  mode: NodeGroupDomainMode;
  domain: string;
  alias: string;
  server_names_text: string;
  host_headers_text: string;
};

type GeneratedPreviewItem = {
  engine: ProxyEngine;
  node_id: string;
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

const modeOrder: NodeGroupDomainMode[] = [
  "direct",
  "legacy_direct",
  "cdn",
  "auto_cdn",
  "relay",
  "worker",
  "reality",
  "fake",
];

const modeMeta: Record<NodeGroupDomainMode, ModeMeta> = {
  direct: {
    title: "На прямую",
    description:
      "Выбирай этот режим, если домен смотрит прямо на сервер и работает без CDN или прокси.",
    generated: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2", "gRPC"],
    domainHint: "Укажи домен, который уже направлен на сервер в режиме DNS only.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Если оставить пустым, панель возьмёт сам домен.",
    hostHeaderHint: "По строкам. Если оставить пустым, Host будет равен домену.",
  },
  legacy_direct: {
    title: "Старый direct",
    description:
      "Прямое подключение без gRPC. Используй, если нужен только классический набор без gRPC-вариантов.",
    generated: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2"],
    domainHint: "Укажи домен, который смотрит прямо на сервер без прокси.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Если оставить пустым, панель возьмёт сам домен.",
    hostHeaderHint: "По строкам. Если оставить пустым, Host будет равен домену.",
  },
  cdn: {
    title: "CDN",
    description:
      "Используй для домена за CDN или любым прокси-режимом. Панель подготовит набор TLS-вариантов.",
    generated: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "gRPC"],
    domainHint: "Укажи домен, который выдаётся пользователям и работает через прокси или CDN.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Если оставить пустым, панель возьмёт сам домен.",
    hostHeaderHint: "По строкам. Если оставить пустым, Host будет равен домену.",
  },
  auto_cdn: {
    title: "Auto CDN",
    description:
      "Работает как CDN-режим, но публичный адрес будет определяться автоматически через DNS домена.",
    generated: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "gRPC", "авто IP"],
    domainHint: "Укажи домен, по которому можно получить актуальный IP через DNS.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Если оставить пустым, панель возьмёт сам домен.",
    hostHeaderHint: "По строкам. Если оставить пустым, Host будет равен домену.",
  },
  relay: {
    title: "Relay",
    description:
      "Подходит для схемы с промежуточным сервером. Панель соберёт обычный набор точек входа с relay-доменом.",
    generated: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2", "gRPC"],
    domainHint: "Укажи внешний домен, через который пользователи заходят на relay-схему.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Если оставить пустым, панель возьмёт сам домен.",
    hostHeaderHint: "По строкам. Если оставить пустым, Host будет равен домену.",
  },
  worker: {
    title: "Worker",
    description:
      "Готовит worker-схему для WS и HTTP Upgrade. Удобно, когда нужен внешний Host и отдельные SNI.",
    generated: ["VLESS WS", "VLESS HTTP Upgrade", "Trojan WS", "VMess WS"],
    domainHint: "Укажи домен, через который будет заходить трафик worker-схемы.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Если оставить пустым, панель возьмёт сам домен.",
    hostHeaderHint: "По строкам. Если строк меньше, панель подставит первый Host для остальных вариантов.",
  },
  reality: {
    title: "Reality",
    description:
      "Для каждого SNI из списка создаётся отдельная точка входа с ключами Reality. Ключи генерируются автоматически.",
    generated: ["VLESS Reality", "отдельный endpoint на каждый SNI"],
    domainHint: "Укажи домен, который будет публичным адресом для Reality-подключения.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Каждая строка создаёт отдельный Reality endpoint.",
    hostHeaderHint: "Для Reality не используется и может быть пустым.",
  },
  fake: {
    title: "Поддельный сайт",
    description:
      "Используй, если нужен маскирующий домен и отдельный SNI для обходных схем.",
    generated: ["VLESS", "Trojan", "VMess", "Shadowsocks 2022", "TUIC", "Hysteria2", "gRPC"],
    domainHint: "Укажи внешний домен, который будет отдаваться клиентам как публичный адрес.",
    aliasHint: "Короткое имя для панели и конфигов. Можно оставить пустым.",
    serverNameHint: "По строкам. Если оставить пустым, панель возьмёт сам домен.",
    hostHeaderHint: "По строкам. Если оставить пустым, Host будет равен домену.",
  },
};

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

function createDraft(mode: NodeGroupDomainMode = "direct"): DomainDraft {
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

function draftFromDomain(domain: NodeGroupDomain): DomainDraft {
  return {
    id: domain.id,
    mode: domain.mode,
    domain: domain.domain,
    alias: domain.alias ?? "",
    server_names_text: domain.server_names.join("\n"),
    host_headers_text: domain.host_headers.join("\n"),
  };
}

function formFromDomains(domains: NodeGroupDomain[] | undefined) {
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

function groupServers(nodeGroups: NodeGroup[] | undefined, nodes: Node[] | undefined) {
  const servers = new Map<string, ServerNode>();
  for (const group of nodeGroups ?? []) {
    servers.set(group.id, {
      id: group.id,
      tenant_id: group.tenant_id,
      name: group.name,
      runtimes: {},
    });
  }
  for (const node of nodes ?? []) {
    const current = servers.get(node.node_group_id) ?? {
      id: node.node_group_id,
      tenant_id: node.tenant_id,
      name: node.name,
      runtimes: {},
    };
    current.runtimes[node.engine] = node;
    servers.set(node.node_group_id, current);
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

function ruleTitle(draft: DomainDraft) {
  if (draft.domain.trim()) {
    return draft.domain.trim();
  }
  if (draft.alias.trim()) {
    return draft.alias.trim();
  }
  return "Новое правило";
}

function ruleSubtitle(draft: DomainDraft) {
  const parts = [modeMeta[draft.mode].title];
  if (draft.alias.trim()) {
    parts.push(draft.alias.trim());
  }
  return parts.join(" / ");
}

export function NodeEndpointsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = api.readSession();
  const [selectedServerId, setSelectedServerId] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [form, setForm] = useState<DomainDraft[]>(createStarterPack);
  const [selectedDraftId, setSelectedDraftId] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  const servers = useMemo(
    () => groupServers(nodeGroupsQuery.data, nodesQuery.data),
    [nodeGroupsQuery.data, nodesQuery.data],
  );

  const selectedServer = useMemo(
    () => servers.find((server) => server.id === selectedServerId) ?? servers[0] ?? null,
    [selectedServerId, servers],
  );

  const selectedDraft = useMemo(
    () => form.find((draft) => draft.id === selectedDraftId) ?? form[0] ?? null,
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
    queryKey: ["node-group-domains", selectedServer?.id],
    enabled: Boolean(session.accessToken && selectedServer),
    queryFn: () => api.listNodeGroupDomains(selectedServer!.id, selectedServer!.tenant_id),
  });

  const endpointsQuery = useQuery({
    queryKey: [
      "server-endpoints",
      selectedServer?.id,
      selectedServer?.runtimes.xray?.id,
      selectedServer?.runtimes.singbox?.id,
    ],
    enabled: Boolean(session.accessToken && selectedServer),
    queryFn: async () => {
      if (!selectedServer) {
        return {} as Partial<Record<ProxyEngine, NodeEndpoint[]>>;
      }
      const entries = await Promise.all(
        (["xray", "singbox"] as const)
          .filter((engine) => selectedServer.runtimes[engine])
          .map(async (engine) => {
            const node = selectedServer.runtimes[engine]!;
            const endpoints = await api.listNodeEndpoints(node.id, node.tenant_id);
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
        throw new Error("Сначала выбери серверную ноду.");
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
      return api.replaceNodeGroupDomains(selectedServer.id, {
        tenant_id: selectedServer.tenant_id,
        domains,
      });
    },
    onSuccess: async (domains) => {
      setError(null);
      setMessage(`Доменные правила сохранены: ${domains.length}. Точки входа пересобраны автоматически.`);
      setSettingsOpen(false);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["node-group-domains"] }),
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
      nodeId,
      tenantId,
      endpoints,
    }: {
      nodeId: string;
      tenantId: string;
      endpoints: NodeEndpoint[];
    }) =>
      api.replaceNodeEndpoints(nodeId, {
        tenant_id: tenantId,
        endpoints: endpoints.map(endpointInput),
      }),
    onSuccess: async () => {
      setError(null);
      setMessage("Состояние точки входа обновлено.");
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
      const runtime = selectedServer?.runtimes[engine];
      if (!runtime) {
        continue;
      }
      for (const endpoint of endpointsQuery.data?.[engine] ?? []) {
        items.push({
          engine,
          node_id: runtime.id,
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

  function addDraft(mode: NodeGroupDomainMode = selectedDraft?.mode ?? "direct") {
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
      nodeId: item.node_id,
      tenantId: item.tenant_id,
      endpoints: next,
    });
  }

  if (!session.accessToken) {
    return <AuthRequired title="Раздел точек входа недоступен без авторизации" />;
  }

  return (
    <div className="space-y-8">
      <div>
        <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
          {t("nav_group.infrastructure")}
        </div>
        <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">Домены и точки входа</h1>
        <p className="mt-3 max-w-4xl text-base text-[#485644]">
          Здесь настраиваются домены, режимы, SNI и Host для выбранного сервера. После сохранения панель сама
          пересобирает полный набор точек входа.
        </p>
      </div>

      {message ? <div className="text-sm text-emerald-700">{message}</div> : null}
      {error ? <div className="text-sm text-danger">{error}</div> : null}

      <Card className="space-y-5 shadow-sm">
        <div className="grid gap-3 xl:grid-cols-[1.1fr_auto]">
          <Select value={selectedServerId} onChange={(event) => setSelectedServerId(event.target.value)}>
            <option value="">Выберите сервер</option>
            {servers.map((server) => (
              <option key={server.id} value={server.id}>
                {server.name} / {server.tenant_id}
              </option>
            ))}
          </Select>
          <Button type="button" disabled={!selectedServer} onClick={() => setSettingsOpen(true)}>
            Настроить домены
          </Button>
        </div>

        {selectedServer ? (
          <div className="grid gap-3 md:grid-cols-2">
            {(["xray", "singbox"] as const).map((engine) => {
              const runtime = selectedServer.runtimes[engine];
              return (
                <div key={engine} className="rounded-[22px] border border-border bg-[#f8f5f0] px-4 py-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="text-sm font-semibold">{engine}</div>
                    <Badge tone={runtimeTone(runtime)}>{runtimeLabel(runtime)}</Badge>
                  </div>
                  <div className="mt-2 text-sm text-foreground/80">
                    {runtime ? `Версия ${runtime.version}` : "Рантайм ещё не зарегистрирован"}
                  </div>
                  <div className="mt-2 text-xs text-foreground/90">
                    Последний сигнал: {formatDate(runtime?.last_seen_at ?? null)}
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
                  {modeMeta[domain.mode].title} / {domain.domain}
                </Badge>
              ))}
            </div>
          ) : (
            <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-6 text-sm text-foreground/80">
              Доменов ещё нет. Открой настройки и заполни нужные режимы в одном окне.
            </div>
          )
        ) : (
          <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-6 text-sm text-foreground/80">
            Сначала выбери сервер.
          </div>
        )}
      </Card>

      <Card className="space-y-4 shadow-sm">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-end xl:justify-between">
          <div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">Сгенерированные точки входа</h2>
          </div>
          <div className="text-sm text-foreground/80">Всего: {generatedPreview.length}</div>
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
                      {item.endpoint.enabled ? "Включена" : "Выключена"}
                    </Badge>
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={toggleEndpointMutation.isPending}
                      onClick={() => toggleEndpoint(item)}
                    >
                      {item.endpoint.enabled ? "Выключить" : "Включить"}
                    </Button>
                  </div>
                </div>

                <div className="mt-4 grid gap-2 text-sm text-foreground/90">
                  <div>Безопасность: {item.endpoint.security}</div>
                  <div>SNI: {item.endpoint.server_name ?? "—"}</div>
                  <div>Host: {item.endpoint.host_header ?? "—"}</div>
                  <div>Path: {item.endpoint.path ?? "—"}</div>
                  <div>Service: {item.endpoint.service_name ?? "—"}</div>
                  <div>ALPN: {item.endpoint.alpn.join(", ") || "—"}</div>
                  <div>Reality ключ: {item.endpoint.reality_public_key ? "готов" : "не нужен"}</div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-8 text-sm text-foreground/80">
            {selectedServer
              ? "Сохрани домены, и панель сразу соберёт точки входа для выбранного сервера."
              : "Сначала выбери сервер."}
          </div>
        )}
      </Card>

      <Dialog
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        title={selectedServer ? `Домены сервера ${selectedServer.name}` : "Настройка доменов"}
        description="Все режимы, SNI и Host настраиваются в одном окне. После сохранения панель сама пересоберёт точки входа."
        className="max-w-6xl"
      >
        <div className="grid gap-6 xl:grid-cols-[320px_minmax(0,1fr)]">
          <div className="space-y-4">
            <div className="flex gap-3">
              <Button className="flex-1" type="button" variant="secondary" onClick={() => addDraft()}>
                Добавить правило
              </Button>
              <Button className="flex-1" type="button" variant="secondary" onClick={resetPreset}>
                Стартовый набор
              </Button>
            </div>

            <div className="rounded-[28px] border border-border bg-[#f8f5f0] p-3">
              <div className="px-2 pb-3 text-xs uppercase tracking-[0.24em] text-foreground/80">Правила</div>
              <div className="max-h-[58vh] space-y-2 overflow-y-auto pr-1">
                {form.map((draft, index) => {
                  const active = selectedDraft?.id === draft.id;
                  return (
                    <button
                      key={draft.id}
                      type="button"
                      onClick={() => setSelectedDraftId(draft.id)}
                      className={`w-full rounded-[22px] border px-4 py-3 text-left transition ${
                        active
                          ? "border-accent bg-accent/10 text-foreground"
                          : "border-border bg-card/70 text-foreground/75 hover:bg-muted"
                      }`}
                    >
                      <div className="text-xs uppercase tracking-[0.2em] text-foreground/80">#{index + 1}</div>
                      <div className="mt-2 text-sm font-semibold">{ruleTitle(draft)}</div>
                      <div className="mt-1 text-xs text-foreground/80">{ruleSubtitle(draft)}</div>
                    </button>
                  );
                })}
              </div>
            </div>
          </div>

          {selectedDraft ? (
            <div className="space-y-5">
              <div className="rounded-[28px] border border-border bg-[#f8f5f0] p-5">
                <div className="text-xs uppercase tracking-[0.24em] text-foreground/80">Режим</div>
                <Select
                  className="mt-3"
                  value={selectedDraft.mode}
                  onChange={(event) =>
                    updateDraft(selectedDraft.id, "mode", event.target.value as NodeGroupDomainMode)
                  }
                >
                  {modeOrder.map((mode) => (
                    <option key={mode} value={mode}>
                      {modeMeta[mode].title}
                    </option>
                  ))}
                </Select>

                <div className="mt-4 rounded-[24px] bg-[#f2efe4] p-4 text-sm text-foreground/75">
                  <div className="font-semibold text-foreground">{modeMeta[selectedDraft.mode].title}</div>
                  <div className="mt-2">{modeMeta[selectedDraft.mode].description}</div>
                  <div className="mt-4 flex flex-wrap gap-2">
                    {modeMeta[selectedDraft.mode].generated.map((item) => (
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
                      onClick={() => updateDraft(selectedDraft.id, "mode", mode)}
                    >
                      <span className="font-semibold">{modeMeta[mode].title}:</span> {modeMeta[mode].description}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 xl:grid-cols-2">
                <div className="space-y-2">
                  <div className="text-sm font-semibold">Домен</div>
                  <Input
                    placeholder="example.com"
                    value={selectedDraft.domain}
                    onChange={(event) => updateDraft(selectedDraft.id, "domain", event.target.value)}
                  />
                  <div className="text-sm text-foreground/80">{modeMeta[selectedDraft.mode].domainHint}</div>
                </div>

                <div className="space-y-2">
                  <div className="text-sm font-semibold">Псевдоним</div>
                  <Input
                    placeholder="main"
                    value={selectedDraft.alias}
                    onChange={(event) => updateDraft(selectedDraft.id, "alias", event.target.value)}
                  />
                  <div className="text-sm text-foreground/80">{modeMeta[selectedDraft.mode].aliasHint}</div>
                </div>
              </div>

              <div className="grid gap-4 xl:grid-cols-2">
                <div className="space-y-2">
                  <div className="text-sm font-semibold">SNI / server_name</div>
                  <Textarea
                    className="min-h-40"
                    placeholder={"example.com\ncdn.example.com"}
                    value={selectedDraft.server_names_text}
                    onChange={(event) => updateDraft(selectedDraft.id, "server_names_text", event.target.value)}
                  />
                  <div className="text-sm text-foreground/80">{modeMeta[selectedDraft.mode].serverNameHint}</div>
                </div>

                <div className="space-y-2">
                  <div className="text-sm font-semibold">Host header</div>
                  <Textarea
                    className="min-h-40"
                    placeholder={"example.com\nworker.example.com"}
                    value={selectedDraft.host_headers_text}
                    onChange={(event) => updateDraft(selectedDraft.id, "host_headers_text", event.target.value)}
                  />
                  <div className="text-sm text-foreground/80">{modeMeta[selectedDraft.mode].hostHeaderHint}</div>
                </div>
              </div>

              <div className="flex items-center justify-between gap-3 rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-4 py-4">
                <div className="text-sm text-foreground/80">
                  Пустые поля SNI и Host не ломают правило. Если их не заполнять, панель автоматически подставит сам
                  домен.
                </div>
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => removeDraft(selectedDraft.id)}
                  disabled={form.length === 0}
                >
                  Удалить правило
                </Button>
              </div>

              <div className="flex justify-end gap-3">
                <Button type="button" variant="secondary" onClick={() => setSettingsOpen(false)}>
                  Отмена
                </Button>
                <Button type="button" disabled={saveMutation.isPending || !selectedServer} onClick={() => saveMutation.mutate()}>
                  {saveMutation.isPending ? "Сохраняю..." : "Сохранить"}
                </Button>
              </div>
            </div>
          ) : (
            <div className="rounded-[28px] border border-dashed border-border bg-[#f8f5f0] px-5 py-12 text-sm text-foreground/80">
              Правило не выбрано. Добавь новое или заполни стартовый набор.
            </div>
          )}
        </div>
      </Dialog>
    </div>
  );
}
