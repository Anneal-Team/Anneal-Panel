import { useState } from "react";
import { cn } from "@/lib/utils";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronUp } from "lucide-react";

export function ExpandableText({ text, className }: { text: string; className?: string }) {
  const [expanded, setExpanded] = useState(false);
  const { t } = useTranslation();

  const isLong = text.length > 200 || (text.match(/\n/g) || []).length > 2;

  if (!isLong) {
    return <div className={cn("whitespace-pre-wrap break-words", className)}>{text}</div>;
  }

  return (
    <div className={className}>
      <div
        className={cn(
          "whitespace-pre-wrap break-words overflow-hidden",
          !expanded && "line-clamp-3"
        )}
      >
        {text}
      </div>
      <button
        type="button"
        onClick={() => {
          setExpanded(!expanded);
        }}
        className="mt-2 flex items-center gap-1 text-xs font-semibold text-[#728468] hover:text-[#384733] transition"
      >
        {expanded ? (
          <>
            <ChevronUp className="h-4 w-4" /> {t("ui.collapse", "Свернуть")}
          </>
        ) : (
          <>
            <ChevronDown className="h-4 w-4" /> {t("ui.expand", "Развернуть полностью")}
          </>
        )}
      </button>
    </div>
  );
}
