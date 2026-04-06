import * as React from "react";

import { cn } from "@/lib/utils";

type BadgeTone = "default" | "success" | "warning" | "danger" | "muted";

const toneClasses: Record<BadgeTone, string> = {
  default: "bg-accent/10 text-accent",
  success: "bg-emerald-500/10 text-emerald-700",
  warning: "bg-amber-500/15 text-amber-800",
  danger: "bg-danger/10 text-danger",
  muted: "bg-muted text-foreground/90",
};

type BadgeProps = React.HTMLAttributes<HTMLSpanElement> & {
  tone?: BadgeTone;
};

export function Badge({ className, tone = "default", ...props }: BadgeProps) {
  const toneClass = Object.hasOwn(toneClasses, tone) ? toneClasses[tone] : toneClasses.default;

  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-3 py-1 text-xs font-semibold uppercase tracking-[0.18em]",
        toneClass,
        className,
      )}
      {...props}
    />
  );
}
