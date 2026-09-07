// Registry boundary: shadcn/new-york input API, themed with the local beUI tokens.
import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export type InputProps = InputHTMLAttributes<HTMLInputElement>;

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className, type = "text", ...props },
  ref,
) {
  // Checkboxes and radios retain their native geometry. The registry boundary
  // still owns their focus and class hook, but must not turn them into text fields.
  const isNativeToggle = type === "checkbox" || type === "radio";
  return (
    <input
      ref={ref}
      type={type}
      className={cn(
        "beui-input outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
        !isNativeToggle && "flex min-h-[var(--ds-touch-target)] w-full rounded-[var(--ds-radius-sm)] border border-border bg-[var(--ds-surface)] px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
});
