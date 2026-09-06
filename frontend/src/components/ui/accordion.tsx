// Registry boundary: disclosure primitives with button/region ARIA linkage.
import {
  createContext,
  useContext,
  useId,
  useState,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type AccordionContextValue = { open: Set<string>; toggle: (value: string) => void };
type AccordionItemContextValue = { value: string; open: boolean; triggerId: string; contentId: string };
const AccordionContext = createContext<AccordionContextValue | null>(null);
const AccordionItemContext = createContext<AccordionItemContextValue | null>(null);
function useAccordion() { const context = useContext(AccordionContext); if (!context) throw new Error("Accordion items require Accordion"); return context; }
function useAccordionItem() { const context = useContext(AccordionItemContext); if (!context) throw new Error("AccordionTrigger and AccordionContent require AccordionItem"); return context; }

export function Accordion({ type = "single", value, defaultValue, onValueChange, children, className }: { type?: "single" | "multiple"; value?: string | string[]; defaultValue?: string | string[]; onValueChange?: (value: string | string[]) => void; children: ReactNode; className?: string }) {
  const initial = Array.isArray(defaultValue) ? defaultValue : defaultValue ? [defaultValue] : [];
  const [internal, setInternal] = useState(() => new Set(initial));
  const controlled = new Set(Array.isArray(value) ? value : value ? [value] : []);
  const open = value === undefined ? internal : controlled;
  const toggle = (next: string) => {
    const updated = new Set(open);
    if (updated.has(next)) updated.delete(next); else { if (type === "single") updated.clear(); updated.add(next); }
    if (value === undefined) setInternal(updated);
    onValueChange?.(type === "single" ? [...updated][0] ?? "" : [...updated]);
  };
  return <AccordionContext.Provider value={{ open, toggle }}><div className={cn("beui-accordion divide-y divide-border", className)}>{children}</div></AccordionContext.Provider>;
}

export function AccordionItem({ value, className, children }: { value: string; className?: string; children: ReactNode }) {
  const accordion = useAccordion();
  const id = useId();
  return <AccordionItemContext.Provider value={{ value, open: accordion.open.has(value), triggerId: `${id}-trigger`, contentId: `${id}-content` }}><div data-state={accordion.open.has(value) ? "open" : "closed"} className={cn("beui-accordion-item", className)}>{children}</div></AccordionItemContext.Provider>;
}

export function AccordionTrigger({ className, children, onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  const accordion = useAccordion(); const item = useAccordionItem();
  return <Button id={item.triggerId} variant="ghost" className={cn("w-full justify-between px-1 text-left", className)} aria-expanded={item.open} aria-controls={item.contentId} onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) accordion.toggle(item.value); }} {...props}>{children}</Button>;
}

export function AccordionContent({ className, children, ...props }: HTMLAttributes<HTMLDivElement>) {
  const item = useAccordionItem();
  return <div id={item.contentId} role="region" aria-labelledby={item.triggerId} hidden={!item.open} className={cn("beui-accordion-content pb-3", className)} {...props}>{children}</div>;
}
