import { motion, MotionConfig, useReducedMotion } from "motion/react";
import {
  createContext,
  useContext,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";

type TabsContextValue = {
  value: string;
  setValue: (value: string) => void;
  baseId: string;
  layoutId: string;
  variant: "pill" | "underline" | "segment";
};

const TabsContext = createContext<TabsContextValue | null>(null);

function useTabs() {
  const value = useContext(TabsContext);
  if (!value) throw new Error("BeUITabs components require BeUITabs");
  return value;
}

export function BeUITabs({
  defaultValue,
  value,
  onValueChange,
  children,
  className,
  variant = "underline",
}: {
  defaultValue: string;
  value?: string;
  onValueChange?: (value: string) => void;
  children: ReactNode;
  className?: string;
  variant?: "pill" | "underline" | "segment";
}) {
  const [internal, setInternal] = useState(defaultValue);
  const current = value ?? internal;
  const baseId = useId();
  const reduce = useReducedMotion();
  const context = useMemo(
    () => ({
      value: current,
      setValue: (next: string) => {
        if (value === undefined) setInternal(next);
        onValueChange?.(next);
      },
      baseId,
      layoutId: `${baseId}-indicator`,
      variant,
    }),
    [baseId, current, onValueChange, value, variant],
  );
  return (
    <MotionConfig transition={reduce ? { duration: 0 } : { type: "spring", stiffness: 220, damping: 28 }}>
      <TabsContext.Provider value={context}>
        <motion.div layoutRoot className={className}>{children}</motion.div>
      </TabsContext.Provider>
    </MotionConfig>
  );
}

export function BeUITabsList({ children, label }: { children: ReactNode; label: string }) {
  const { variant } = useTabs();
  const ref = useRef<HTMLDivElement>(null);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    const tabs = Array.from(ref.current?.querySelectorAll<HTMLButtonElement>('[role="tab"]') ?? []);
    const active = tabs.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 :
      (active + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
    event.preventDefault();
    tabs[next]?.focus();
    tabs[next]?.click();
  };
  return <div ref={ref} role="tablist" aria-label={label} data-variant={variant} className="beui-tabs-list" onKeyDown={onKeyDown}>{children}</div>;
}

export function BeUITab({ value, children }: { value: string; children: ReactNode }) {
  const tabs = useTabs();
  const selected = tabs.value === value;
  return (
    <button
      id={`${tabs.baseId}-tab-${value}`}
      type="button"
      role="tab"
      tabIndex={selected ? 0 : -1}
      aria-selected={selected}
      aria-controls={`${tabs.baseId}-panel-${value}`}
      onClick={() => tabs.setValue(value)}
    >
      {selected && <motion.span className="beui-tab-indicator" layoutId={tabs.layoutId} />}
      <span>{children}</span>
    </button>
  );
}

export function BeUITabPanel({ value, children }: { value: string; children: ReactNode }) {
  const tabs = useTabs();
  return (
    <motion.div
      id={`${tabs.baseId}-panel-${value}`}
      role="tabpanel"
      aria-labelledby={`${tabs.baseId}-tab-${value}`}
      hidden={tabs.value !== value}
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      className="detail-panel"
    >
      {children}
    </motion.div>
  );
}
