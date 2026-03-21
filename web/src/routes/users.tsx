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
import { api, type User, type UserRole, type UserStatus } from "@/lib/api";
import { formatDate, formatRole } from "@/lib/format";

const createUserRoles: UserRole[] = ["admin", "user"];
const editableUserRoles: UserRole[] = ["superadmin", "admin", "user"];
const userStatuses: UserStatus[] = ["active", "suspended"];

type CreateMode = "user" | "reseller";

type UserForm = {
  user_id: string;
  target_tenant_id: string;
  email: string;
  display_name: string;
  role: UserRole;
  status: UserStatus;
  password: string;
};

type ResellerForm = {
  user_id: string;
  tenant_name: string;
  email: string;
  display_name: string;
  status: UserStatus;
  password: string;
};

function createInitialUserForm(): UserForm {
  return {
    user_id: "",
    target_tenant_id: "",
    email: "",
    display_name: "",
    role: "user",
    status: "active",
    password: "",
  };
}

function createInitialResellerForm(): ResellerForm {
  return {
    user_id: "",
    tenant_name: "",
    email: "",
    display_name: "",
    status: "active",
    password: "",
  };
}

function userFormFromUser(user: User): UserForm {
  return {
    user_id: user.id,
    target_tenant_id: user.tenant_id ?? "",
    email: user.email,
    display_name: user.display_name,
    role: user.role,
    status: user.status,
    password: "",
  };
}

function resellerFormFromUser(user: User): ResellerForm {
  return {
    user_id: user.id,
    tenant_name: user.tenant_name ?? "",
    email: user.email,
    display_name: user.display_name,
    status: user.status,
    password: "",
  };
}

function formatStatus(value: UserStatus) {
  return value === "active" ? "Активен" : "Отключён";
}

function statusTone(value: UserStatus) {
  return value === "active" ? "success" : "danger";
}

export function UsersPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = api.readSession();
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [createMode, setCreateMode] = useState<CreateMode>("user");
  const [createOpen, setCreateOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<User | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<User | null>(null);
  const [resellerForm, setResellerForm] = useState<ResellerForm>(createInitialResellerForm);
  const [userForm, setUserForm] = useState<UserForm>(createInitialUserForm);

  const usersQuery = useQuery({
    queryKey: ["users"],
    queryFn: api.listUsers,
    enabled: Boolean(session.accessToken),
  });
  const resellersQuery = useQuery({
    queryKey: ["resellers"],
    queryFn: api.listResellers,
    enabled: Boolean(session.accessToken),
  });

  const resellerOptions = useMemo(
    () =>
      (resellersQuery.data ?? [])
        .filter((user) => user.tenant_id)
        .map((user) => ({
          user_id: user.id,
          tenant_id: user.tenant_id as string,
          label: `${user.tenant_name ?? user.display_name} · ${user.email}`,
        })),
    [resellersQuery.data],
  );

  const createResellerMutation = useMutation({
    mutationFn: () =>
      api.createReseller({
        tenant_name: resellerForm.tenant_name.trim(),
        email: resellerForm.email.trim(),
        display_name: resellerForm.display_name.trim(),
        password: resellerForm.password,
      }),
    onSuccess: async (created) => {
      setError(null);
      setMessage(`Реселлер ${created.email} создан.`);
      setCreateOpen(false);
      setResellerForm(createInitialResellerForm());
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["users"] }),
        queryClient.invalidateQueries({ queryKey: ["resellers"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const createUserMutation = useMutation({
    mutationFn: () =>
      api.createUser({
        target_tenant_id: userForm.target_tenant_id || undefined,
        email: userForm.email.trim(),
        display_name: userForm.display_name.trim(),
        role: userForm.role,
        password: userForm.password,
      }),
    onSuccess: async (created) => {
      setError(null);
      setMessage(`Пользователь ${created.email} создан.`);
      setCreateOpen(false);
      setUserForm(createInitialUserForm());
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["users"] }),
        queryClient.invalidateQueries({ queryKey: ["resellers"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const updateUserMutation = useMutation({
    mutationFn: () =>
      api.updateUser(userForm.user_id, {
        email: userForm.email.trim(),
        display_name: userForm.display_name.trim(),
        role: userForm.role,
        status: userForm.status,
        password: userForm.password.trim() ? userForm.password : null,
      }),
    onSuccess: async (updated) => {
      setError(null);
      setMessage(`Аккаунт ${updated.email} обновлён.`);
      setEditOpen(false);
      setEditTarget(null);
      setUserForm(createInitialUserForm());
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["users"] }),
        queryClient.invalidateQueries({ queryKey: ["resellers"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const updateResellerMutation = useMutation({
    mutationFn: () =>
      api.updateReseller(resellerForm.user_id, {
        tenant_name: resellerForm.tenant_name.trim(),
        email: resellerForm.email.trim(),
        display_name: resellerForm.display_name.trim(),
        status: resellerForm.status,
        password: resellerForm.password.trim() ? resellerForm.password : null,
      }),
    onSuccess: async (updated) => {
      setError(null);
      setMessage(`Реселлер ${updated.email} обновлён.`);
      setEditOpen(false);
      setEditTarget(null);
      setResellerForm(createInitialResellerForm());
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["users"] }),
        queryClient.invalidateQueries({ queryKey: ["resellers"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  const deleteUserMutation = useMutation({
    mutationFn: (user: User) => {
      if (user.role === "reseller") {
        return api.deleteReseller(user.id);
      }
      return api.deleteUser(user.id);
    },
    onSuccess: async () => {
      setError(null);
      setMessage("Аккаунт удалён.");
      setDeleteTarget(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["users"] }),
        queryClient.invalidateQueries({ queryKey: ["resellers"] }),
      ]);
    },
    onError: (mutationError) => {
      setMessage(null);
      setError(mutationError.message);
    },
  });

  function openCreateDialog() {
    setCreateMode("user");
    setUserForm(createInitialUserForm());
    setResellerForm(createInitialResellerForm());
    setCreateOpen(true);
  }

  function openEditDialog(user: User) {
    setEditTarget(user);
    if (user.role === "reseller") {
      setResellerForm(resellerFormFromUser(user));
    } else {
      setUserForm(userFormFromUser(user));
    }
    setEditOpen(true);
  }

  function submitEdit() {
    if (!editTarget) {
      return;
    }
    if (editTarget.role === "reseller") {
      updateResellerMutation.mutate();
      return;
    }
    updateUserMutation.mutate();
  }

  if (!session.accessToken) {
    return <AuthRequired title={t("users.unauthorized")} />;
  }

  return (
    <div className="space-y-8">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <div className="inline-block rounded-md bg-[#e2efca] px-3 py-1 text-xs font-bold uppercase tracking-widest text-[#384733]">
            {t("nav_group.overview")}
          </div>
          <h1 className="mt-4 text-4xl font-bold text-[#1d271a]">{t("users.title")}</h1>
          <p className="mt-3 max-w-4xl text-base text-[#485644]">
            {t("users.subtitle")}
          </p>
        </div>
        <Button onClick={openCreateDialog} type="button">
          {t("users.create.button")}
        </Button>
      </div>

      {message ? <div className="text-sm text-emerald-700">{message}</div> : null}
      {error ? <div className="text-sm text-danger">{error}</div> : null}

      <Card className="space-y-4 shadow-sm">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
          <div>
            <h2 className="mt-2 text-xl font-bold text-[#1d271a]">{t("users.list.title")}</h2>
          </div>
          <div className="rounded-2xl bg-muted px-4 py-3 text-sm text-foreground/80">
            Всего: {usersQuery.data?.length ?? 0}
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="min-w-full text-left text-sm">
            <thead className="text-foreground/80">
              <tr>
                <th className="px-3 py-2">Аккаунт</th>
                <th className="px-3 py-2">Роль</th>
                <th className="px-3 py-2">Тенант</th>
                <th className="px-3 py-2">Статус</th>
                <th className="px-3 py-2">2FA</th>
                <th className="px-3 py-2">Создан</th>
                <th className="px-3 py-2">Действия</th>
              </tr>
            </thead>
            <tbody>
              {(usersQuery.data ?? []).map((user) => (
                <tr key={user.id} className="border-t border-border/70">
                  <td className="px-3 py-3">
                    <div className="font-semibold">{user.display_name}</div>
                    <div className="text-foreground/80">{user.email}</div>
                  </td>
                  <td className="px-3 py-3">
                    <Badge tone={user.role === "user" ? "muted" : "default"}>
                      {formatRole(user.role)}
                    </Badge>
                  </td>
                  <td className="px-3 py-3 text-foreground/80">
                    {user.tenant_name ?? user.tenant_id ?? "Глобальный"}
                  </td>
                  <td className="px-3 py-3">
                    <Badge tone={statusTone(user.status)}>{formatStatus(user.status)}</Badge>
                  </td>
                  <td className="px-3 py-3">{user.totp_confirmed ? "Подключена" : "Ожидает"}</td>
                  <td className="px-3 py-3 text-foreground/80">{formatDate(user.created_at)}</td>
                  <td className="px-3 py-3">
                    <div className="flex flex-wrap gap-2">
                      <Button type="button" variant="secondary" onClick={() => openEditDialog(user)}>
                        Редактировать
                      </Button>
                      {user.role !== "superadmin" ? (
                        <Button type="button" variant="danger" onClick={() => setDeleteTarget(user)}>
                          Удалить
                        </Button>
                      ) : null}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>

      <Dialog
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        title={createMode === "user" ? "Новый пользователь" : "Новый реселлер"}
        description="Выбери тип аккаунта и заполни только нужные поля."
      >
        <div className="space-y-6">
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant={createMode === "user" ? "default" : "secondary"}
              onClick={() => setCreateMode("user")}
            >
              Пользователь
            </Button>
            <Button
              type="button"
              variant={createMode === "reseller" ? "default" : "secondary"}
              onClick={() => setCreateMode("reseller")}
            >
              Реселлер
            </Button>
          </div>

          {createMode === "reseller" ? (
            <form
              className="grid gap-3"
              onSubmit={(event) => {
                event.preventDefault();
                createResellerMutation.mutate();
              }}
            >
              <Input
                placeholder="Название тенанта"
                value={resellerForm.tenant_name}
                onChange={(event) =>
                  setResellerForm((current) => ({ ...current, tenant_name: event.target.value }))
                }
              />
              <Input
                placeholder="Имя"
                value={resellerForm.display_name}
                onChange={(event) =>
                  setResellerForm((current) => ({ ...current, display_name: event.target.value }))
                }
              />
              <Input
                placeholder="Email"
                value={resellerForm.email}
                onChange={(event) =>
                  setResellerForm((current) => ({ ...current, email: event.target.value }))
                }
              />
              <Input
                placeholder="Пароль"
                type="password"
                value={resellerForm.password}
                onChange={(event) =>
                  setResellerForm((current) => ({ ...current, password: event.target.value }))
                }
              />
              <div className="flex justify-end gap-3">
                <Button type="button" variant="secondary" onClick={() => setCreateOpen(false)}>
                  Отмена
                </Button>
                <Button
                  disabled={
                    createResellerMutation.isPending ||
                    !resellerForm.tenant_name.trim() ||
                    !resellerForm.display_name.trim() ||
                    !resellerForm.email.trim() ||
                    !resellerForm.password
                  }
                  type="submit"
                >
                  {createResellerMutation.isPending ? "Создаю..." : "Создать"}
                </Button>
              </div>
            </form>
          ) : (
            <form
              className="grid gap-3"
              onSubmit={(event) => {
                event.preventDefault();
                createUserMutation.mutate();
              }}
            >
              <Select
                value={userForm.role}
                onChange={(event) =>
                  setUserForm((current) => ({ ...current, role: event.target.value as UserRole }))
                }
              >
                {createUserRoles.map((role) => (
                  <option key={role} value={role}>
                    {formatRole(role)}
                  </option>
                ))}
              </Select>

              <Select
                value={userForm.target_tenant_id}
                onChange={(event) =>
                  setUserForm((current) => ({
                    ...current,
                    target_tenant_id: event.target.value,
                  }))
                }
              >
                <option value="">Глобальный аккаунт или текущий тенант</option>
                {resellerOptions.map((option) => (
                  <option key={option.tenant_id} value={option.tenant_id}>
                    {option.label}
                  </option>
                ))}
              </Select>

              <Input
                placeholder="Имя"
                value={userForm.display_name}
                onChange={(event) =>
                  setUserForm((current) => ({ ...current, display_name: event.target.value }))
                }
              />
              <Input
                placeholder="Email"
                value={userForm.email}
                onChange={(event) =>
                  setUserForm((current) => ({ ...current, email: event.target.value }))
                }
              />
              <Input
                placeholder="Пароль"
                type="password"
                value={userForm.password}
                onChange={(event) =>
                  setUserForm((current) => ({ ...current, password: event.target.value }))
                }
              />
              <div className="flex justify-end gap-3">
                <Button type="button" variant="secondary" onClick={() => setCreateOpen(false)}>
                  Отмена
                </Button>
                <Button
                  disabled={
                    createUserMutation.isPending ||
                    !userForm.display_name.trim() ||
                    !userForm.email.trim() ||
                    !userForm.password
                  }
                  type="submit"
                >
                  {createUserMutation.isPending ? "Создаю..." : "Создать"}
                </Button>
              </div>
            </form>
          )}
        </div>
      </Dialog>

      <Dialog
        open={editOpen}
        onClose={() => {
          setEditOpen(false);
          setEditTarget(null);
        }}
        title={editTarget?.role === "reseller" ? "Редактирование реселлера" : "Редактирование аккаунта"}
        description="Измени данные аккаунта и сохрани обновлённые параметры."
      >
        {editTarget?.role === "reseller" ? (
          <form
            className="grid gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              submitEdit();
            }}
          >
            <Input
              placeholder="Название тенанта"
              value={resellerForm.tenant_name}
              onChange={(event) =>
                setResellerForm((current) => ({ ...current, tenant_name: event.target.value }))
              }
            />
            <Input
              placeholder="Имя"
              value={resellerForm.display_name}
              onChange={(event) =>
                setResellerForm((current) => ({ ...current, display_name: event.target.value }))
              }
            />
            <Input
              placeholder="Email"
              value={resellerForm.email}
              onChange={(event) =>
                setResellerForm((current) => ({ ...current, email: event.target.value }))
              }
            />
            <Select
              value={resellerForm.status}
              onChange={(event) =>
                setResellerForm((current) => ({ ...current, status: event.target.value as UserStatus }))
              }
            >
              {userStatuses.map((status) => (
                <option key={status} value={status}>
                  {formatStatus(status)}
                </option>
              ))}
            </Select>
            <Input
              placeholder="Новый пароль"
              type="password"
              value={resellerForm.password}
              onChange={(event) =>
                setResellerForm((current) => ({ ...current, password: event.target.value }))
              }
            />
            <div className="flex justify-end gap-3">
              <Button
                type="button"
                variant="secondary"
                onClick={() => {
                  setEditOpen(false);
                  setEditTarget(null);
                }}
              >
                Отмена
              </Button>
              <Button
                disabled={
                  updateResellerMutation.isPending ||
                  !resellerForm.tenant_name.trim() ||
                  !resellerForm.display_name.trim() ||
                  !resellerForm.email.trim()
                }
                type="submit"
              >
                {updateResellerMutation.isPending ? "Сохраняю..." : "Сохранить"}
              </Button>
            </div>
          </form>
        ) : editTarget ? (
          <form
            className="grid gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              submitEdit();
            }}
          >
            <div className="rounded-[24px] bg-[#f2efe4] px-4 py-3 text-sm text-foreground/90">
              Тенант: {editTarget.tenant_name ?? editTarget.tenant_id ?? "Глобальный"}
            </div>
            <Input
              placeholder="Имя"
              value={userForm.display_name}
              onChange={(event) =>
                setUserForm((current) => ({ ...current, display_name: event.target.value }))
              }
            />
            <Input
              placeholder="Email"
              value={userForm.email}
              onChange={(event) =>
                setUserForm((current) => ({ ...current, email: event.target.value }))
              }
            />
            <Select
              value={userForm.role}
              onChange={(event) =>
                setUserForm((current) => ({ ...current, role: event.target.value as UserRole }))
              }
            >
              {(editTarget.role === "superadmin" ? ["superadmin"] : editableUserRoles.filter((role) => role !== "superadmin")) .map((role) => (
                <option key={role} value={role}>
                  {formatRole(role)}
                </option>
              ))}
            </Select>
            <Select
              value={userForm.status}
              onChange={(event) =>
                setUserForm((current) => ({ ...current, status: event.target.value as UserStatus }))
              }
            >
              {userStatuses.map((status) => (
                <option key={status} value={status}>
                  {formatStatus(status)}
                </option>
              ))}
            </Select>
            <Input
              placeholder="Новый пароль"
              type="password"
              value={userForm.password}
              onChange={(event) =>
                setUserForm((current) => ({ ...current, password: event.target.value }))
              }
            />
            <div className="flex justify-end gap-3">
              <Button
                type="button"
                variant="secondary"
                onClick={() => {
                  setEditOpen(false);
                  setEditTarget(null);
                }}
              >
                Отмена
              </Button>
              <Button
                disabled={
                  updateUserMutation.isPending ||
                  !userForm.display_name.trim() ||
                  !userForm.email.trim()
                }
                type="submit"
              >
                {updateUserMutation.isPending ? "Сохраняю..." : "Сохранить"}
              </Button>
            </div>
          </form>
        ) : null}
      </Dialog>

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        onClose={() => setDeleteTarget(null)}
        title={deleteTarget?.role === "reseller" ? "Удалить реселлера" : "Удалить аккаунт"}
        description={
          deleteTarget
            ? deleteTarget.role === "reseller"
              ? `Реселлер ${deleteTarget.display_name} будет удалён вместе со своим тенантом и связанными данными.`
              : `Аккаунт ${deleteTarget.display_name} будет удалён.`
            : ""
        }
        confirmLabel="Удалить"
        pendingLabel="Удаляю..."
        isPending={deleteUserMutation.isPending}
        onConfirm={() => {
          if (deleteTarget) {
            deleteUserMutation.mutate(deleteTarget);
          }
        }}
      />
    </div>
  );
}
