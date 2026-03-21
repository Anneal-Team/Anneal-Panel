import { useTranslation } from "react-i18next";
import { Globe } from "lucide-react";
import { cn } from "@/lib/utils";

export function LanguageSwitcher({ className }: { className?: string }) {
  const { i18n } = useTranslation();

  const toggleLanguage = () => {
    const newLang = i18n.language === "ru" ? "en" : "ru";
    i18n.changeLanguage(newLang);
  };

  return (
    <button
      onClick={toggleLanguage}
      className={cn("flex items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium transition hover:bg-black/10 dark:hover:bg-white/10", className)}
      title="Change language / Сменить язык"
    >
      <Globe className="h-5 w-5" />
      <span className="uppercase tracking-widest">{i18n.language || "ru"}</span>
    </button>
  );
}
