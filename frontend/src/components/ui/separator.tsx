// Registry boundary: semantic visual separator.
import { type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export function Separator({ orientation = "horizontal", className, ...props }: HTMLAttributes<HTMLDivElement> & { orientation?: "horizontal" | "vertical" }) {
  return <div role="separator" aria-orientation={orientation} className={cn("beui-separator shrink-0 bg-border", orientation === "horizontal" ? "h-px w-full" : "h-full w-px", className)} {...props} />;
}
