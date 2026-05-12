import { BrowserRouter, Navigate, NavLink, Route, Routes, useLocation, useNavigate } from "react-router";
import {
  Bell,
  LayoutDashboard,
  LogIn,
  RadioTower,
  Users,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { LanguageSwitcher } from "@/components/language-switcher";
import { api } from "@/lib/api";
import { panelAssetUrl, panelBasePath } from "@/lib/panel-base";
import { DashboardPage } from "@/routes/dashboard";
import { DevicesPage } from "@/routes/devices";
import { LoginPage } from "@/routes/login";
import { NotificationsPage } from "@/routes/notifications";
import { PublicSubscriptionPage } from "@/routes/public-subscription";
import { SubscriptionsPage } from "@/routes/subscriptions";
import { UsersPage } from "@/routes/users";

const navigationGroups = [
  {
    titleKey: "nav_group.overview",
    items: [
      { to: "/", labelKey: "nav.dashboard", icon: LayoutDashboard },
      { to: "/users", labelKey: "nav.users", icon: Users },
    ],
  },
  {
    titleKey: "nav_group.infrastructure",
    items: [
      { to: "/subscriptions", labelKey: "nav.subscriptions", icon: RadioTower },
    ],
  },
  {
    titleKey: "nav_group.system",
    items: [
      { to: "/notifications", labelKey: "nav.notifications", icon: Bell },
    ],
  },
];

function normalizeBasePath() {
  const basePath = panelBasePath();
  if (!basePath || basePath === "/") {
    return undefined;
  }
  return basePath.endsWith("/") ? basePath.slice(0, -1) : basePath;
}

function routeLinkClassName(isActive: boolean) {
  const baseClassName =
    "flex items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium transition hover:bg-[#1f2d1e] hover:text-[#a4d872]";
  return isActive ? `${baseClassName} bg-[#21321d] text-[#a4d872]` : baseClassName;
}

function Shell() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const hasAccessSession = Boolean(api.readSession().accessToken);
  const isLoginPage = pathname === "/login";
  const isPublicImportPage = pathname.startsWith("/import/");
  const { t } = useTranslation();

  async function handleLogout() {
    await api.logout();
    void navigate("/login");
  }

  if (!hasAccessSession && !isLoginPage && !isPublicImportPage) {
    return <Navigate to="/login" replace />;
  }

  const sidebarAssetUrl = panelAssetUrl("anneal-sidebar.svg");

  return (
    <div className="min-h-screen bg-[#f8f5f0] text-foreground">
      <div className="mx-auto flex min-h-screen w-full flex-col px-4 py-4 md:flex-row md:items-stretch md:gap-6 xl:px-8 xl:py-6">
        {!isLoginPage && !isPublicImportPage ? (
          <aside className="mb-4 flex flex-col rounded-[24px] bg-[#141813] text-[#aebda4] py-8 px-4 shadow-panel md:sticky md:top-6 md:mb-0 md:h-[calc(100vh-3rem)] md:w-72 md:shrink-0 md:overflow-y-auto">
            <div className="mb-8 flex items-center justify-center">
              <img src={sidebarAssetUrl} alt="Anneal Logo" className="block h-[52px] w-auto object-contain" />
            </div>

            <nav className="flex-1 space-y-8">
              {navigationGroups.map((group) => (
                <div key={group.titleKey}>
                  <div className="mb-3 px-4 text-xs font-bold uppercase tracking-widest text-[#728468]">
                    {t(group.titleKey)}
                  </div>
                  <div className="space-y-1">
                    {group.items.map(({ to, labelKey, icon: Icon }) => (
                      <NavLink
                        key={to}
                        to={to}
                        end={to === "/"}
                        className={({ isActive }) => routeLinkClassName(isActive)}
                      >
                        <Icon className="h-5 w-5" />
                        {t(labelKey)}
                      </NavLink>
                    ))}
                  </div>
                </div>
              ))}
            </nav>

            <div className="mt-8 flex flex-col gap-2">
              <LanguageSwitcher className="w-full justify-start text-[#aebda4] hover:bg-[#1f2d1e] hover:text-[#a4d872]" />
              {hasAccessSession ? (
                <button
                  className="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium transition hover:bg-[#1f2d1e] hover:text-[#a4d872]"
                  onClick={() => {
                    void handleLogout();
                  }}
                >
                  <LogIn className="h-5 w-5" />
                  {t("nav.logout")}
                </button>
              ) : (
                <NavLink
                  to="/login"
                  className="flex w-full items-center justify-center gap-2 rounded-xl bg-[#21321d] px-4 py-3 text-sm font-semibold text-[#a4d872] transition hover:opacity-90"
                >
                  <LogIn className="h-5 w-5" />
                  {t("nav.login")}
                </NavLink>
              )}
            </div>
          </aside>
        ) : null}

        <main className="flex-1 rounded-[32px] border border-white/40 bg-[#f4efe6] p-5 shadow-panel md:p-10">
          <Routes>
            <Route path="/" element={<DashboardPage />} />
            <Route path="/login" element={<LoginPage />} />
            <Route path="/import/:token" element={<PublicSubscriptionPage />} />
            <Route path="/users" element={<UsersPage />} />
            <Route path="/devices" element={<DevicesPage />} />
            <Route path="/subscriptions" element={<SubscriptionsPage />} />
            <Route path="/notifications" element={<NotificationsPage />} />
            <Route path="*" element={<DashboardPage />} />
          </Routes>
        </main>
      </div>
    </div>
  );
}

export function AppRouter() {
  return (
    <BrowserRouter basename={normalizeBasePath()}>
      <Shell />
    </BrowserRouter>
  );
}
