import * as React from "react";

import { cn } from "@/lib/utils";

type BadgeTone = "default" | "success" | "warning" | "danger" | "muted";

function toneClass(tone: BadgeTone) {
  switch (tone) {
    case "success":
      return "bg-emerald-500/10 text-emerald-700";
    case "warning":
      return "bg-amber-500/15 text-amber-800";
    case "danger":
      return "bg-danger/10 text-danger";
    case "muted":
      return "bg-muted text-foreground/90";
    case "default":
    default:
      return "bg-accent/10 text-accent";
  }
}

type BadgeProps = React.HTMLAttributes<HTMLSpanElement> & {
  tone?: BadgeTone;
};

export function Badge({ className, tone = "default", ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-3 py-1 text-xs font-semibold uppercase tracking-[0.18em]",
        toneClass(tone),
        className,
      )}
      {...props}
    />
  );
}
