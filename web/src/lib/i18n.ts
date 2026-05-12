import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { LocaleDefinition } from "@/lib/i18n-types";

const localeModules = import.meta.glob<{ default: LocaleDefinition }>(
  "../locales/*.ts",
  { eager: true }
);

const localeEntries = Object.entries(localeModules).map(([path, mod]) => {
  const code = path.replace(/^.*\/([^/]+)\.ts$/, "$1");
  return [code, mod.default] as const;
});

export const availableLocales: Record<string, LocaleDefinition> = Object.fromEntries(localeEntries);

const resources = Object.fromEntries(
  localeEntries.map(([code, def]) => [code, { translation: def.translations }])
);

const defaultLng = "ru" in availableLocales ? "ru" : Object.keys(availableLocales)[0] ?? "en";

i18n.use(initReactI18next).init({
  resources,
  lng: defaultLng,
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
