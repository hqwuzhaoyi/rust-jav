// Registry boundary: native scrolling surface with a visible semantic landmark.
import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export const ScrollArea = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(function ScrollArea(
  { className, tabIndex = 0, ...props },
  ref,
) {
  return <div ref={ref} tabIndex={tabIndex} className={cn("beui-scroll-area overflow-auto overscroll-contain outline-none focus-visible:ring-2 focus-visible:ring-ring", className)} {...props} />;
});
