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
type AccordionSharedProps = {
  children: ReactNode;
  className?: string;
};
type SingleAccordionProps = AccordionSharedProps & {
  type?: "single";
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
};
type MultipleAccordionProps = AccordionSharedProps & {
  type: "multiple";
  value?: string[];
  defaultValue?: string[];
  onValueChange?: (value: string[]) => void;
};
export type AccordionProps = SingleAccordionProps | MultipleAccordionProps;
const AccordionContext = createContext<AccordionContextValue | null>(null);
const AccordionItemContext = createContext<AccordionItemContextValue | null>(null);
function useAccordion() { const context = useContext(AccordionContext); if (!context) throw new Error("Accordion items require Accordion"); return context; }
function useAccordionItem() { const context = useContext(AccordionItemContext); if (!context) throw new Error("AccordionTrigger and AccordionContent require AccordionItem"); return context; }

export function Accordion(props: AccordionProps) {
  const { children, className } = props;
  const multiple = props.type === "multiple";
  const initial = multiple
    ? props.defaultValue ?? []
    : props.defaultValue
      ? [props.defaultValue]
      : [];
  const [internal, setInternal] = useState(() => new Set(initial));
  const controlled = new Set(
    multiple ? props.value ?? [] : props.value ? [props.value] : [],
  );
  const open = props.value === undefined ? internal : controlled;
  const toggle = (next: string) => {
    const updated = new Set(open);
    if (updated.has(next)) updated.delete(next);
    else {
      if (!multiple) updated.clear();
      updated.add(next);
    }
    if (props.value === undefined) setInternal(updated);
    if (multiple) props.onValueChange?.([...updated]);
    else props.onValueChange?.([...updated][0] ?? "");
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
