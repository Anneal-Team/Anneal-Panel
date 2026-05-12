import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Globe, Check, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";
import { availableLocales } from "@/lib/i18n";

const locales = Object.entries(availableLocales);

export function LanguageSwitcher({ className }: { className?: string }) {
  const { i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const currentCode = i18n.language;
  const currentLabel = availableLocales[currentCode]?.label ?? currentCode.toUpperCase();

  const handleClickOutside = useCallback((e: MouseEvent) => {
    if (ref.current && !ref.current.contains(e.target as Node)) {
      setOpen(false);
    }
  }, []);

  useEffect(() => {
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [handleClickOutside]);

  const handleSelect = useCallback((code: string) => {
    void i18n.changeLanguage(code);
    setOpen(false);
  }, [i18n]);

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        className={cn(
          "flex items-center gap-3 rounded-xl px-4 py-3 text-sm font-medium transition",
          "hover:bg-black/10 dark:hover:bg-white/10",
          className
        )}
        aria-haspopup="listbox"
        aria-expanded={open}
        title="Change language / Сменить язык"
      >
        <Globe className="h-5 w-5 shrink-0" />
        <span className="flex-1 text-left">{currentLabel}</span>
        <ChevronDown
          className={cn(
            "h-4 w-4 shrink-0 opacity-50 transition-transform duration-200",
            open && "rotate-180"
          )}
        />
      </button>

      {open && (
        <div
          role="listbox"
          className={cn(
            "absolute bottom-full left-0 mb-2 w-full min-w-[160px]",
            "rounded-xl border border-white/10 bg-[#1a2218] py-1 shadow-xl",
            "z-50 overflow-hidden"
          )}
        >
          {locales.map(([code, def]) => {
            const isActive = code === currentCode;
            return (
              <button
                key={code}
                role="option"
                aria-selected={isActive}
                onClick={() => handleSelect(code)}
                className={cn(
                  "flex w-full items-center gap-3 px-4 py-2.5 text-sm font-medium transition",
                  isActive
                    ? "text-[#a4d872]"
                    : "text-[#aebda4] hover:bg-white/5 hover:text-[#a4d872]"
                )}
              >
                <span className="flex-1 text-left">{def.label}</span>
                {isActive && <Check className="h-4 w-4 shrink-0" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
