// Registry boundary: minimal shadcn-compatible menu composition for app action menus.
import {
  createContext,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type ReactNode,
} from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type MenuContext = {
  open: boolean;
  setOpen: (open: boolean) => void;
  menuId: string;
  root: React.RefObject<HTMLDivElement | null>;
  trigger: React.RefObject<HTMLButtonElement | null>;
};
const MenuContext = createContext<MenuContext | null>(null);
function useMenu() {
  const context = useContext(MenuContext);
  if (!context) throw new Error("DropdownMenu components require DropdownMenu");
  return context;
}

export function DropdownMenu({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);
  return <MenuContext.Provider value={{ open, setOpen, menuId, root, trigger }}><div ref={root} className="beui-dropdown relative inline-flex">{children}</div></MenuContext.Provider>;
}

export function DropdownMenuTrigger({ className, onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  const { open, setOpen, menuId, trigger } = useMenu();
  return <Button ref={trigger} aria-haspopup="menu" aria-expanded={open} aria-controls={menuId} className={className} onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) setOpen(!open); }} {...props} />;
}

export function DropdownMenuContent({ className, children, ...props }: HTMLAttributes<HTMLDivElement>) {
  const { open, setOpen, menuId, trigger } = useMenu();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => { if (open) ref.current?.querySelector<HTMLElement>("[role=menuitem]")?.focus(); }, [open]);
  if (!open) return null;
  const items = () => Array.from(ref.current?.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not(:disabled)") ?? []);
  const closeAndRestore = () => {
    trigger.current?.focus();
    setOpen(false);
  };
  return <div ref={ref} id={menuId} role="menu" className={cn("absolute right-0 top-full z-50 mt-2 min-w-40 rounded-[var(--ds-radius-sm)] border border-border bg-[var(--ds-surface)] p-1 shadow-lg", className)} onKeyDown={(event) => {
    if (event.key === "Escape") { event.preventDefault(); closeAndRestore(); return; }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const menuItems = items();
    if (!menuItems.length) return;
    const current = menuItems.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? menuItems.length - 1 : (current + (event.key === "ArrowDown" ? 1 : -1) + menuItems.length) % menuItems.length;
    event.preventDefault();
    menuItems[next]?.focus();
  }} {...props}>{children}</div>;
}

export function DropdownMenuItem({ className, density = "compact", onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { density?: "touch" | "compact" }) {
  const { setOpen } = useMenu();
  return <Button role="menuitem" density={density} variant="ghost" className={cn("w-full justify-start px-3 text-left", className)} onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) setOpen(false); }} {...props} />;
}
