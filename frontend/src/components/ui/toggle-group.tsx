// Registry boundary: accessible segmented toggles with roving arrow-key navigation.
import {
  createContext,
  useContext,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type ToggleContext = { value: string; setValue: (value: string) => void; root: React.RefObject<HTMLDivElement | null> };
const ToggleContext = createContext<ToggleContext | null>(null);
function useToggleGroup() {
  const context = useContext(ToggleContext);
  if (!context) throw new Error("ToggleGroupItem requires ToggleGroup");
  return context;
}

export function ToggleGroup({ value, defaultValue, onValueChange, children, className, "aria-label": ariaLabel }: { value?: string; defaultValue?: string; onValueChange?: (value: string) => void; children: ReactNode; className?: string; "aria-label": string }) {
  const [internal, setInternal] = useState(defaultValue ?? "");
  const root = useRef<HTMLDivElement>(null);
  const current = value ?? internal;
  const setValue = (next: string) => { if (value === undefined) setInternal(next); onValueChange?.(next); };
  return <ToggleContext.Provider value={{ value: current, setValue, root }}><div ref={root} role="group" aria-label={ariaLabel} className={cn("beui-toggle-group inline-flex rounded-[var(--ds-radius-sm)] border border-border p-1", className)}>{children}</div></ToggleContext.Provider>;
}

export function ToggleGroupItem({ value, className, onKeyDown, onClick, ...props }: Omit<ButtonHTMLAttributes<HTMLButtonElement>, "value"> & { value: string }) {
  const group = useToggleGroup();
  const pressed = group.value === value;
  const id = useId();
  const moveFocus = (event: KeyboardEvent<HTMLButtonElement>) => {
    onKeyDown?.(event);
    if (event.defaultPrevented || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    const items = Array.from(group.root.current?.querySelectorAll<HTMLButtonElement>("[data-toggle-item]") ?? []);
    const current = items.indexOf(event.currentTarget);
    const index = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (current + (event.key === "ArrowRight" ? 1 : -1) + items.length) % items.length;
    event.preventDefault();
    items[index]?.focus();
    items[index]?.click();
  };
  return <Button id={id} data-toggle-item density="compact" variant={pressed ? "secondary" : "ghost"} aria-pressed={pressed} className={cn("px-3", className)} onKeyDown={moveFocus} onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) group.setValue(value); }} {...props} />;
}
