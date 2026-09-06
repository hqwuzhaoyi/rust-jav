// Registry boundary: shadcn/new-york card primitives using semantic surface tokens.
import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export const Card = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  function Card({ className, ...props }, ref) {
    return <div ref={ref} className={cn("beui-card rounded-[var(--ds-radius-md)] border border-border bg-[var(--ds-surface)] text-foreground shadow-sm", className)} {...props} />;
  },
);
