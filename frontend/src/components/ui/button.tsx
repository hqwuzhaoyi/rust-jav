// Registry boundary: shadcn/new-york button API, themed with the local beUI tokens.
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  /** Keeps the existing app's compact controls explicit and accessible. */
  density?: "touch" | "compact";
  variant?: "default" | "secondary" | "ghost" | "destructive" | "outline";
};

const variants = {
  default: "bg-primary text-primary-foreground hover:bg-primary/90",
  secondary: "bg-[var(--ds-surface-muted)] text-foreground hover:bg-[var(--ds-hover)]",
  ghost: "bg-transparent text-foreground hover:bg-[var(--ds-hover)]",
  destructive: "bg-destructive text-white hover:bg-destructive/90",
  outline: "border border-border bg-transparent text-foreground hover:bg-[var(--ds-hover)]",
} as const;

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    { className, density = "touch", variant = "ghost", type = "button", ...props },
    ref,
  ) {
    return (
      <button
        ref={ref}
        type={type}
        className={cn(
          "beui-button inline-flex items-center justify-center gap-2 rounded-[var(--ds-radius-sm)] font-medium outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
          density === "touch" ? "ui-touch-target" : "min-h-[var(--ds-compact-control)]",
          variants[variant],
          className,
        )}
        {...props}
      />
    );
  },
);
