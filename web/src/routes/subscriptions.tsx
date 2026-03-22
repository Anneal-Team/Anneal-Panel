import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { AuthRequired } from "@/components/auth-required";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { api, type Subscription } from "@/lib/api";
import { formatBytes, formatDate, formatQuotaState } from "@/lib/format";
import { useNow } from "@/lib/use-now";

type CreateSubscriptionForm = {
  tenant_id: string;
  user_id: string;
  name: string;
  note: string;
  traffic_limit_gb: string;
  package_days: string;
};

type EditSubscriptionForm = {
  subscription_id: string;
  tenant_id: string;
  name: string;
  note: string;
  traffic_limit_gb: string;
  expires_at: string;
  suspended: boolean;
};

function bytesFromGigabytes(value: string) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return 0;
  }
  return Math.round(parsed * 1024 * 1024 * 1024);
}

function gigabytesFromBytes(value: number) {
  return (value / 1024 / 1024 / 1024).toFixed(value >= 100 * 1024 * 1024 * 1024 ? 0 : 1);
}

function daysToExpiresAt(days: string, now: number) {
  const parsed = Number(days);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }
  return new Date(now + parsed * 24 * 60 * 60 * 1000);
}

function toDateTimeLocalValue(value: string) {
  const date = new Date(value);
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
}

function fromDateTimeLocalValue(value: string) {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }
  return parsed.toISOString();
}

function deliveryUrlFor(subscription: { current_token: string | null }) {
  if (!subscription.current_token) {
    return null;
  }
  return `${window.location.origin}/s/${subscription.current_token}`;
}

function createInitialForm(): CreateSubscriptionForm {
  return {
    tenant_id: "",
    user_id: "",
    name: "",
    note: "",
    traffic_limit_gb: "100",
    package_days: "30",
  };
}

function editFormFromSubscription(subscription: Subscription): EditSubscriptionForm {
  return {
    subscription_id: subscription.id,
    tenant_id: subscription.tenant_id,
    name: subscription.name,
    note: subscription.note ?? "",
    traffic_limit_gb: gigabytesFromBytes(subscription.traffic_limit_bytes),
    expires_at: toDateTimeLocalValue(subscription.expires_at),
    suspended: subscription.suspended,
  };
}

export function SubscriptionsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = api.readSession();
  const now = useNow();
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [deliveryUrl, setDeliveryUrl] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Subscription | null>(null);
  const [createForm, setCreateForm] = useState<CreateSubscriptionForm>(createInitialForm);
  const [editForm, setEditForm] = useState<EditSubscriptionForm | null>(null);

  const usersQuery = useQuery({
    queryKey: ["users"],
    queryFn: api.listUsers,
    enabled: Boolean(session.accessToken),
  });
  const subscriptionsQuery = useQuery({
    queryKey: ["subscriptions"],
    queryFn: api.listSubscriptions,
    enabled: Boolean(session.accessToken),
  });

  const userOptions = useMemo(
    () =>
      (usersQuery.data ?? [])
        .filter((user) => user.role === "user" && user.tenant_id)
        .map((user) => ({
          id: user.id,
          tenant_id: user.tenant_id as string,
          label: `${user.display_name} · ${user.email}`,
          display_name: user.display_name,
        })),
    [usersQuery.data],
  );

  const userNames = useMemo(() => {
    return new Map((usersQuery.data ?? []).map((user) => [user.id, user.display_name] as const));
  }, [usersQuery.data]);

  const expiresAt = daysToExpiresAt(createForm.package_days, now);

  const createSubscriptionMutation = useMutation({
    mutationFn: () => {
      if (!expiresAt) {
        throw new Error("Укажи срок пакета в днях");
      }
      return api.createSubscription({
        tenant_id: createForm.tenant_id,
        user_id: createForm.user_id,
        name: createForm.name.trim() || "Подписка",
        note: createForm.note.trim() || null,
        traffic_limit_bytes: bytesFromGigabytes(createForm.traffic_limit_gb),
        expires_at: expiresAt.toISOString(),
      });
    },
    onSuccess: async (result) => {
      setError(null);
      setMessage(`Подписка ${result.subscription.name} создана.`);
      setDeliveryUrl(result.delivery_url);
      setCreateOpen(false);
      setCreateForm(createInitialForm());
      await queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
    },
    onError: (mutationError) => {
      setDeliveryUrl(null);
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const updateSubscriptionMutation = useMutation({
    mutationFn: () => {
      if (!editForm) {
        throw new Error("Не выбрана подписка");
      }
      const expiresAtValue = fromDateTimeLocalValue(editForm.expires_at);
      if (!expiresAtValue) {
        throw new Error("Укажи корректную дату окончания");
      }
      return api.updateSubscription(editForm.subscription_id, {
        name: editForm.name.trim(),
        note: editForm.note.trim() || null,
        traffic_limit_bytes: bytesFromGigabytes(editForm.traffic_limit_gb),
        expires_at: expiresAtValue,
        suspended: editForm.suspended,
      });
    },
    onSuccess: async (subscription) => {
      setError(null);
      setMessage(`Подписка ${subscription.name} обновлена.`);
      setEditOpen(false);
      setEditForm(null);
      await queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const deleteSubscriptionMutation = useMutation({
    mutationFn: (subscription: Subscription) =>
      api.deleteSubscription(subscription.id, subscription.tenant_id),
    onSuccess: async () => {
      setError(null);
      setMessage("Подписка удалена.");
      setDeleteTarget(null);
      await queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const rotateLinkMutation = useMutation({
    mutationFn: async ({
      subscriptionId,
      tenantId,
    }: {
      subscriptionId: string;
      tenantId: string;
    }) => {
      const link = await api.rotateSubscriptionLink(subscriptionId, tenantId);
      return `${window.location.origin}/s/${link.token}`;
    },
    onSuccess: async (url) => {
      setError(null);
      setMessage("Ссылка подписки обновлена.");
      setDeliveryUrl(url);
      await queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
    },
    onError: (mutationError) => {
      setDeliveryUrl(null);
      setMessage(null);
      setError(mutationError.message);
    },
  });

  function openCreateDialog() {
    setDeliveryUrl(null);
    setCreateForm(createInitialForm());
    setCreateOpen(true);
  }

  function openEditDialog(subscription: Subscription) {
    setEditForm(editFormFromSubscription(subscription));
    setEditOpen(true);
  }

  if (!session.accessToken) {
    return <AuthRequired title={t("subscriptions.unauthorized")} />;
  }

  return (
    <div className="space-y-8">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
            {t("nav_group.system")}
          </div>
          <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("subscriptions.title")}</h1>
          <p className="mt-3 max-w-4xl text-base text-[#485644]">
            {t("subscriptions.subtitle")}
          </p>
        </div>
        <Button onClick={openCreateDialog} type="button">
          {t("subscriptions.create.button")}
        </Button>
      </div>

      {message ? <div className="text-sm text-emerald-700">{message}</div> : null}
      {error ? <div className="text-sm text-danger">{error}</div> : null}

      {deliveryUrl ? (
        <Card className="break-all bg-gradient-to-r from-muted to-card text-sm">
          <div className="font-semibold">{t("subscriptions.success_link_label")}</div>
          <div className="mt-3 font-mono text-foreground/75">{deliveryUrl}</div>
        </Card>
      ) : null}

      <Card className="space-y-4 shadow-sm">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
          <div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("subscriptions.list.title")}</h2>
          </div>
          <div className="rounded-2xl bg-muted px-4 py-3 text-sm text-foreground/80">
            Всего: {subscriptionsQuery.data?.length ?? 0}
          </div>
        </div>

        {(subscriptionsQuery.data ?? []).length > 0 ? (
          <div className="space-y-3">
            {(subscriptionsQuery.data ?? []).map((subscription) => {
              const ratio =
                subscription.traffic_limit_bytes > 0
                  ? Math.min(subscription.used_bytes / subscription.traffic_limit_bytes, 1)
                  : 1;
              const expiresSoon = new Date(subscription.expires_at).getTime() - now < 3 * 24 * 60 * 60 * 1000;
              const currentDeliveryUrl = deliveryUrlFor(subscription);

              return (
                <div key={subscription.id} className="rounded-[24px] border border-border bg-[#f8f5f0] p-4">
                  <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-3">
                        <div className="text-lg font-semibold">{subscription.name}</div>
                        <Badge
                          tone={
                            subscription.quota_state === "exhausted"
                              ? "danger"
                              : subscription.quota_state === "normal"
                                ? "success"
                                : "warning"
                          }
                        >
                          {formatQuotaState(subscription.quota_state)}
                        </Badge>
                        <Badge tone={subscription.suspended ? "danger" : "muted"}>
                          {subscription.suspended ? "Приостановлена" : "Активна"}
                        </Badge>
                        <Badge tone={expiresSoon ? "warning" : "muted"}>
                          До {formatDate(subscription.expires_at)}
                        </Badge>
                      </div>

                      <div className="mt-2 text-sm text-foreground/80">
                        Пользователь: {userNames.get(subscription.user_id) ?? subscription.user_id}
                      </div>

                      {subscription.note ? (
                        <div className="mt-3 text-sm text-foreground/80">{subscription.note}</div>
                      ) : null}

                      <div className="mt-4 grid gap-3 md:grid-cols-3">
                        <div className="rounded-[20px] bg-[#f2efe4] px-4 py-3">
                          <div className="text-xs uppercase tracking-[0.2em] text-foreground/80">Лимит</div>
                          <div className="mt-2 text-lg font-semibold">
                            {gigabytesFromBytes(subscription.traffic_limit_bytes)} GB
                          </div>
                        </div>
                        <div className="rounded-[20px] bg-[#f2efe4] px-4 py-3">
                          <div className="text-xs uppercase tracking-[0.2em] text-foreground/80">Использовано</div>
                          <div className="mt-2 text-lg font-semibold">{formatBytes(subscription.used_bytes)}</div>
                        </div>
                        <div className="rounded-[20px] bg-[#f2efe4] px-4 py-3">
                          <div className="text-xs uppercase tracking-[0.2em] text-foreground/80">Доступна до</div>
                          <div className="mt-2 text-lg font-semibold">{formatDate(subscription.expires_at)}</div>
                        </div>
                      </div>

                      <div className="mt-4 h-2 rounded-full bg-muted">
                        <div
                          className="h-2 rounded-full bg-accent transition-all"
                          style={{ width: `${Math.max(ratio * 100, 4)}%` }}
                        />
                      </div>

                      <div className="mt-4 rounded-2xl bg-[#f2efe4] px-4 py-3 text-xs font-mono text-foreground/80 break-all">
                        {currentDeliveryUrl ?? "Ссылка будет показана после первой генерации или ротации."}
                      </div>
                    </div>

                    <div className="flex flex-wrap gap-3">
                      <Button
                        variant="secondary"
                        disabled={rotateLinkMutation.isPending}
                        onClick={() =>
                          rotateLinkMutation.mutate({
                            subscriptionId: subscription.id,
                            tenantId: subscription.tenant_id,
                          })
                        }
                        type="button"
                      >
                        Обновить ссылку
                      </Button>
                      <Button type="button" variant="secondary" onClick={() => openEditDialog(subscription)}>
                        Редактировать
                      </Button>
                      <Button type="button" variant="danger" onClick={() => setDeleteTarget(subscription)}>
                        Удалить
                      </Button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="rounded-[24px] border border-dashed border-border bg-[#f8f5f0] px-5 py-8 text-sm text-foreground/80">
            Пока нет ни одной подписки.
          </div>
        )}
      </Card>

      <Dialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        title="Создать подписку"
        description="Пакет доступа создаётся сразу с лимитом, сроком действия и одной ссылкой выдачи."
      >
        <form
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            createSubscriptionMutation.mutate();
          }}
        >
          <Select
            value={createForm.user_id}
            onChange={(event) => {
              const selected = userOptions.find((user) => user.id === event.target.value);
              setCreateForm((current) => ({
                ...current,
                user_id: event.target.value,
                tenant_id: selected?.tenant_id ?? "",
                name: current.name || (selected ? `${selected.display_name} access` : ""),
              }));
            }}
          >
            <option value="">Выбери пользователя</option>
            {userOptions.map((user) => (
              <option key={user.id} value={user.id}>
                {user.label}
              </option>
            ))}
          </Select>

          <Input
            placeholder="Имя пакета"
            value={createForm.name}
            onChange={(event) =>
              setCreateForm((current) => ({ ...current, name: event.target.value }))
            }
          />

          <Textarea
            placeholder="Примечание администратора"
            value={createForm.note}
            onChange={(event) =>
              setCreateForm((current) => ({ ...current, note: event.target.value }))
            }
          />

          <div className="grid gap-3 md:grid-cols-2">
            <Input
              placeholder="Лимит трафика, GB"
              value={createForm.traffic_limit_gb}
              onChange={(event) =>
                setCreateForm((current) => ({ ...current, traffic_limit_gb: event.target.value }))
              }
            />
            <Input
              placeholder="Дни пакета"
              value={createForm.package_days}
              onChange={(event) =>
                setCreateForm((current) => ({ ...current, package_days: event.target.value }))
              }
            />
          </div>

          <div className="grid gap-3 md:grid-cols-2">
            <div className="rounded-[24px] bg-[#f2efe4] p-4 text-sm text-foreground/90">
              <div>Лимит: {bytesFromGigabytes(createForm.traffic_limit_gb) > 0 ? `${createForm.traffic_limit_gb} GB` : "не задан"}</div>
              <div className="mt-1">Срок: {createForm.package_days || "0"} дней</div>
            </div>
            <div className="rounded-[24px] bg-[#f2efe4] p-4 text-sm text-foreground/90">
              <div>Доступна до</div>
              <div className="mt-2 text-base font-semibold">
                {expiresAt
                  ? new Intl.DateTimeFormat("ru-RU", { dateStyle: "medium", timeStyle: "short" }).format(expiresAt)
                  : "укажи срок пакета"}
              </div>
            </div>
          </div>

          <div className="flex justify-end gap-3">
            <Button type="button" variant="secondary" onClick={() => setCreateOpen(false)}>
              Отмена
            </Button>
            <Button
              disabled={
                createSubscriptionMutation.isPending ||
                !createForm.user_id ||
                !createForm.tenant_id ||
                bytesFromGigabytes(createForm.traffic_limit_gb) <= 0 ||
                !expiresAt
              }
              type="submit"
            >
              {createSubscriptionMutation.isPending ? "Создаю..." : "Создать"}
            </Button>
          </div>
        </form>
      </Dialog>

      <Dialog
        open={editOpen}
        onClose={() => {
          setEditOpen(false);
          setEditForm(null);
        }}
        title="Редактирование подписки"
        description="Измени параметры пакета доступа и сохрани новые ограничения."
      >
        {editForm ? (
          <form
            className="grid gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              updateSubscriptionMutation.mutate();
            }}
          >
            <Input
              placeholder="Имя пакета"
              value={editForm.name}
              onChange={(event) => setEditForm((current) => current ? { ...current, name: event.target.value } : current)}
            />

            <Textarea
              placeholder="Примечание администратора"
              value={editForm.note}
              onChange={(event) => setEditForm((current) => current ? { ...current, note: event.target.value } : current)}
            />

            <div className="grid gap-3 md:grid-cols-2">
              <Input
                placeholder="Лимит трафика, GB"
                value={editForm.traffic_limit_gb}
                onChange={(event) =>
                  setEditForm((current) => current ? { ...current, traffic_limit_gb: event.target.value } : current)
                }
              />
              <Input
                type="datetime-local"
                value={editForm.expires_at}
                onChange={(event) =>
                  setEditForm((current) => current ? { ...current, expires_at: event.target.value } : current)
                }
              />
            </div>

            <label className="flex items-center gap-3 rounded-[24px] bg-[#f2efe4] px-4 py-3 text-sm text-foreground/90">
              <input
                type="checkbox"
                checked={editForm.suspended}
                onChange={(event) =>
                  setEditForm((current) => current ? { ...current, suspended: event.target.checked } : current)
                }
              />
              Приостановить подписку
            </label>

            <div className="flex justify-end gap-3">
              <Button
                type="button"
                variant="secondary"
                onClick={() => {
                  setEditOpen(false);
                  setEditForm(null);
                }}
              >
                Отмена
              </Button>
              <Button
                disabled={
                  updateSubscriptionMutation.isPending ||
                  !editForm.name.trim() ||
                  bytesFromGigabytes(editForm.traffic_limit_gb) <= 0 ||
                  !fromDateTimeLocalValue(editForm.expires_at)
                }
                type="submit"
              >
                {updateSubscriptionMutation.isPending ? "Сохраняю..." : "Сохранить"}
              </Button>
            </div>
          </form>
        ) : null}
      </Dialog>

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        title="Удалить подписку"
        description={
          deleteTarget
            ? `Подписка ${deleteTarget.name} будет удалена. Связанная ссылка выдачи перестанет работать.`
            : ""
        }
        confirmLabel="Удалить"
        pendingLabel="Удаляю..."
        isPending={deleteSubscriptionMutation.isPending}
        onConfirm={() => {
          if (deleteTarget) {
            deleteSubscriptionMutation.mutate(deleteTarget);
          }
        }}
      />
    </div>
  );
}
