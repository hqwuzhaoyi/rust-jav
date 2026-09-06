// Registry boundary: compact status label variants.
import { type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export function Badge({ variant = "secondary", className, ...props }: HTMLAttributes<HTMLSpanElement> & { variant?: "default" | "secondary" | "success" | "warning" | "destructive" }) {
  const variants = { default: "bg-primary text-primary-foreground", secondary: "bg-[var(--ds-muted)] text-foreground", success: "bg-[var(--ds-success-soft)] text-[var(--ds-success)]", warning: "bg-[var(--ds-warning-soft)] text-[var(--ds-warning)]", destructive: "bg-[var(--ds-danger-soft)] text-[var(--ds-danger)]" };
  return <span className={cn("beui-badge inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium", variants[variant], className)} {...props} />;
}
