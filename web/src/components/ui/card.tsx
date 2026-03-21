import { cn } from "@/lib/utils";

type CardProps = React.HTMLAttributes<HTMLDivElement>;

export function Card({ className, ...props }: CardProps) {
  return (
    <div
      className={cn("rounded-3xl border border-[#d9ccb8] bg-white p-5 shadow-[0_8px_30px_rgba(19,34,38,0.08)]", className)}
      {...props}
    />
  );
}
