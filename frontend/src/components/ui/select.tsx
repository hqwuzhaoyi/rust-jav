// Registry boundary: native Select retains dependable keyboard and mobile behavior.
import { forwardRef, type SelectHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export type SelectProps = SelectHTMLAttributes<HTMLSelectElement>;

export const Select = forwardRef<HTMLSelectElement, SelectProps>(function Select(
  { className, children, ...props },
  ref,
) {
  return <select ref={ref} className={cn("beui-select min-h-[var(--ds-touch-target)] w-full rounded-[var(--ds-radius-sm)] border border-border bg-[var(--ds-surface)] px-3 py-2 text-sm text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50", className)} {...props}>{children}</select>;
});
