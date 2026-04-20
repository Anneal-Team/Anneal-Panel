import {
  Link,
  Outlet,
  createRootRoute,
  createRoute,
  createRouter,
  useNavigate,
  useRouterState,
} from "@tanstack/react-router";
import {
  Bell,
  LayoutDashboard,
  LogIn,
  Network,
  RadioTower,
  Settings2,
  Users,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { LanguageSwitcher } from "@/components/language-switcher";
import { api } from "@/lib/api";
import { panelAssetUrl, panelBasePath } from "@/lib/panel-base";
import { DashboardPage } from "@/routes/dashboard";
import { DevicesPage } from "@/routes/devices";
import { LoginPage } from "@/routes/login";
import { NodeEndpointsPage } from "@/routes/node-endpoints";
import { NodesPage } from "@/routes/nodes";
import { NotificationsPage } from "@/routes/notifications";
import { PublicSubscriptionPage } from "@/routes/public-subscription";
import { RolloutsPage } from "@/routes/rollouts";
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
      { to: "/nodes", labelKey: "nav.nodes", icon: Network },
      { to: "/subscriptions", labelKey: "nav.subscriptions", icon: RadioTower },
      { to: "/node-endpoints", labelKey: "nav.node_endpoints", icon: Settings2 },
    ],
  },
  {
    titleKey: "nav_group.system",
    items: [
      { to: "/notifications", labelKey: "nav.notifications", icon: Bell },
    ],
  },
];

function Shell() {
  const navigate = useNavigate();
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const hasAccessSession = Boolean(api.readSession().accessToken);
  const isLoginPage = pathname === "/login";
  const isPublicImportPage = pathname.startsWith("/import/");
  const { t } = useTranslation();

  async function handleLogout() {
    await api.logout();
    await navigate({ to: "/login" });
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
                      <Link
                        key={to}
                        to={to}
                        className="flex items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium transition hover:bg-[#1f2d1e] hover:text-[#a4d872]"
                        activeProps={{ className: "bg-[#21321d] text-[#a4d872]" }}
                      >
                        <Icon className="h-5 w-5" />
                        {t(labelKey)}
                      </Link>
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
                <Link
                  to="/login"
                  className="flex w-full items-center justify-center gap-2 rounded-xl bg-[#21321d] px-4 py-3 text-sm font-semibold text-[#a4d872] transition hover:opacity-90"
                >
                  <LogIn className="h-5 w-5" />
                  {t("nav.login")}
                </Link>
              )}
            </div>
          </aside>
        ) : null}

        <main className="flex-1 rounded-[32px] border border-white/40 bg-[#f4efe6] p-5 shadow-panel md:p-10">
          <Outlet />
        </main>
      </div>
    </div>
  );
}

const rootRoute = createRootRoute({
  component: Shell,
});

const loginRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/login",
  component: LoginPage,
});

const dashboardRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});

const publicSubscriptionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/import/$token",
  component: PublicSubscriptionPage,
});

const usersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/users",
  component: UsersPage,
});

const devicesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/devices",
  component: DevicesPage,
});

const nodesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/nodes",
  component: NodesPage,
});

const nodeEndpointsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/node-endpoints",
  component: NodeEndpointsPage,
});

const subscriptionsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/subscriptions",
  component: SubscriptionsPage,
});

const rolloutsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/rollouts",
  component: RolloutsPage,
});

const notificationsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/notifications",
  component: NotificationsPage,
});

const routeTree = rootRoute.addChildren([
  loginRoute,
  dashboardRoute,
  publicSubscriptionRoute,
  usersRoute,
  devicesRoute,
  nodesRoute,
  nodeEndpointsRoute,
  subscriptionsRoute,
  rolloutsRoute,
  notificationsRoute,
]);

export const router = createRouter({
  routeTree,
  basepath: panelBasePath() || "/",
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
