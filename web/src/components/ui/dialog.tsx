import * as React from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type DialogProps = {
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  children: React.ReactNode;
  className?: string;
};

export function Dialog({ open, title, description, onClose, children, className }: DialogProps) {
  const { t } = useTranslation();

  React.useEffect(() => {
    if (!open) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, onClose]);

  if (!open) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4 backdrop-blur-md pb-safe">
      <div className="absolute inset-0" onClick={onClose} />
      <div
        className={cn(
          "relative z-10 w-full max-w-3xl max-h-[90vh] overflow-y-auto rounded-3xl border border-border bg-[#fbf7ef] p-6 shadow-2xl",
          className,
        )}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="inline-block rounded-md bg-[#e2efca] px-2 py-1 text-[10px] font-bold uppercase tracking-widest text-[#384733]">
              {t("dialog.management")}
            </div>
            <h2 className="mt-3 text-2xl font-bold text-[#1d271a]">{title}</h2>
            {description ? <p className="mt-2 text-sm text-[#485644]">{description}</p> : null}
          </div>
          <Button variant="secondary" onClick={onClose} type="button">
            {t("dialog.close")}
          </Button>
        </div>
        <div className="mt-6 text-[#132226]">{children}</div>
      </div>
    </div>
  );
}
