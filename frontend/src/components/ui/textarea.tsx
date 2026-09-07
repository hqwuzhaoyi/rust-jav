// Registry boundary: shadcn/new-york textarea API using the local token layer.
import { forwardRef, type TextareaHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export type TextareaProps = TextareaHTMLAttributes<HTMLTextAreaElement>;

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea(
  { className, ...props },
  ref,
) {
  return <textarea ref={ref} className={cn("beui-textarea flex min-h-24 w-full rounded-[var(--ds-radius-sm)] border border-border bg-[var(--ds-surface)] px-3 py-2 text-sm text-foreground outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50", className)} {...props} />;
});
