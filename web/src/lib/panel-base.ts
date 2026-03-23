function normalizeBasePath(pathname: string) {
  const normalized = pathname.trim().replace(/\/+$/, "");
  if (normalized === "" || normalized === "/") {
    return "";
  }
  return normalized.startsWith("/") ? normalized : `/${normalized}`;
}

function resolveBaseHref() {
  if (typeof document === "undefined") {
    return new URL("/", "http://localhost");
  }
  const baseElement = document.querySelector("base[href]");
  const href = baseElement?.getAttribute("href") ?? "/";
  try {
    return new URL(href, window.location.origin);
  } catch {
    return new URL("/", window.location.origin);
  }
}

export function panelBasePath() {
  return normalizeBasePath(resolveBaseHref().pathname);
}

export function panelBaseHref() {
  const basePath = panelBasePath();
  return basePath === "" ? "/" : `${basePath}/`;
}

export function panelPublicBaseUrl() {
  return `${window.location.origin}${panelBasePath()}`;
}

export function panelAssetUrl(asset: string) {
  const target = asset.replace(/^\/+/, "");
  return new URL(target, `${window.location.origin}${panelBaseHref()}`).toString();
}
