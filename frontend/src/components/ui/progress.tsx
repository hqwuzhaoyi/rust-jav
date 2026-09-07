// Registry boundary: shadcn/new-york progress semantics without a Radix runtime dependency.
import { type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export function Progress({ value = 0, className, ...props }: HTMLAttributes<HTMLDivElement> & { value?: number }) {
  const percentage = Math.max(0, Math.min(100, value));
  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percentage}
      className={cn("beui-progress relative h-1.5 w-full overflow-hidden rounded-full bg-[var(--ds-muted)]", className)}
      {...props}
    >
      <div className="h-full rounded-full bg-primary transition-transform motion-reduce:transition-none" style={{ transform: `translateX(${percentage - 100}%)` }} />
    </div>
  );
}
