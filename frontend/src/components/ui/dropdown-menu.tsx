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

type MenuContext = { open: boolean; setOpen: (open: boolean) => void; menuId: string; root: React.RefObject<HTMLDivElement | null> };
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
  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);
  return <MenuContext.Provider value={{ open, setOpen, menuId, root }}><div ref={root} className="beui-dropdown relative inline-flex">{children}</div></MenuContext.Provider>;
}

export function DropdownMenuTrigger({ className, onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  const { open, setOpen, menuId } = useMenu();
  return <Button aria-haspopup="menu" aria-expanded={open} aria-controls={menuId} className={className} onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) setOpen(!open); }} {...props} />;
}

export function DropdownMenuContent({ className, children, ...props }: HTMLAttributes<HTMLDivElement>) {
  const { open, setOpen, menuId } = useMenu();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => { if (open) ref.current?.querySelector<HTMLElement>("[role=menuitem]")?.focus(); }, [open]);
  if (!open) return null;
  return <div ref={ref} id={menuId} role="menu" className={cn("absolute right-0 top-full z-50 mt-2 min-w-40 rounded-[var(--ds-radius-sm)] border border-border bg-[var(--ds-surface)] p-1 shadow-lg", className)} onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); setOpen(false); } }} {...props}>{children}</div>;
}

export function DropdownMenuItem({ className, density = "compact", onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { density?: "touch" | "compact" }) {
  const { setOpen } = useMenu();
  return <Button role="menuitem" density={density} variant="ghost" className={cn("w-full justify-start px-3 text-left", className)} onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) setOpen(false); }} {...props} />;
}
