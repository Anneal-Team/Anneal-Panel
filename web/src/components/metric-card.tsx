import { forwardRef } from "react";
import { cn } from "@/lib/utils";

type MetricCardProps = React.HTMLAttributes<HTMLDivElement> & {
  label: string;
  value: string;
  hint?: string;
};

export const MetricCard = forwardRef<HTMLDivElement, MetricCardProps>(
  ({ label, value, hint, className, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          "flex flex-col justify-center rounded-[18px] border border-[#d9ccb8] bg-white p-5 shadow-sm",
          className
        )}
        {...props}
      >
        <div className="text-3xl font-bold text-[#1d271a]">{value}</div>
        <div className="mt-1 text-sm font-medium uppercase tracking-wide text-[#485644]">
          {label}
        </div>
        {hint && <div className="mt-2 text-xs text-[#485644]/70">{hint}</div>}
      </div>
    );
  }
);
MetricCard.displayName = "MetricCard";
