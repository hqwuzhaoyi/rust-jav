import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { type ReactNode, useEffect, useRef } from "react";
import { EASE_OUT, SPRING_PANEL } from "@/lib/ease";
import { PresenceGate } from "@/lib/presence-gate";
import { cn } from "@/lib/utils";

export interface MorphingModalProps {
  viewId: string | null;
  onClose: () => void;
  children: ReactNode;
  placement?: "bottom" | "center";
  className?: string;
  initialAnimation?: boolean;
}

const FOCUSABLE = [
  "[autofocus]",
  "button:not([disabled])",
  "a[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

export function MorphingModal({
  viewId,
  onClose,
  children,
  placement = "bottom",
  className,
  initialAnimation = true,
}: MorphingModalProps) {
  const open = viewId !== null;
  const reduce = useReducedMotion();
  const panelRef = useRef<HTMLDivElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const enterY = reduce ? 0 : placement === "bottom" ? 40 : 20;
  const enterScale = reduce ? 1 : 0.97;

  useEffect(() => {
    if (!open) return;
    const previousOverflow = document.body.style.overflow;
    returnFocusRef.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    document.body.style.overflow = "hidden";

    return () => {
      document.body.style.overflow = previousOverflow;
      const returnFocus = returnFocusRef.current;
      returnFocusRef.current = null;
      if (returnFocus?.isConnected) returnFocus.focus();
    };
  }, [open]);

  useEffect(() => {
    if (!open || viewId === null) return;
    const views = Array.from(
      panelRef.current?.querySelectorAll<HTMLElement>("[data-modal-view]") ?? [],
    );
    const currentView = views.find((view) => view.dataset.modalView === viewId);
    const dialog = currentView?.querySelector<HTMLElement>('[role="dialog"]');

    const focusTarget = dialog?.querySelector<HTMLElement>(FOCUSABLE) ?? dialog;
    if (focusTarget === dialog && dialog && !dialog.hasAttribute("tabindex")) {
      dialog.setAttribute("tabindex", "-1");
    }
    focusTarget?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab" || !dialog) return;
      const focusable = Array.from(
        dialog.querySelectorAll<HTMLElement>(FOCUSABLE),
      );
      if (!focusable.length) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && (active === first || !dialog.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, viewId]);

  return (
    <AnimatePresence initial={false}>
      {open ? (
        <PresenceGate key="backdrop">
          {({ gate }) => (
            <motion.button
              type="button"
              aria-label="Close modal"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2, ease: EASE_OUT }}
              {...gate}
              onClick={onClose}
              className="pointer-events-auto fixed inset-0 z-[80] bg-background/5 [backdrop-filter:blur(14px)_saturate(140%)] [-webkit-backdrop-filter:blur(14px)_saturate(140%)]"
            />
          )}
        </PresenceGate>
      ) : null}

      {open ? (
        <PresenceGate key="panel-layer">
          {({ isPresent, gate }) => (
            <div
              inert={!isPresent}
              className={cn(
                "pointer-events-none fixed inset-4 z-[80] flex justify-center",
                placement === "bottom" ? "items-end pb-4" : "items-center",
              )}
            >
              <motion.div
                ref={panelRef}
                key="panel"
                layout
                initial={initialAnimation ? { opacity: 0, y: enterY, scale: enterScale } : false}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{
                  opacity: 0,
                  y: enterY,
                  scale: reduce ? 1 : 0.98,
                  transition: { duration: 0.18, ease: EASE_OUT },
                }}
                transition={SPRING_PANEL}
                {...gate}
                className={cn(
                  "pointer-events-auto relative w-full max-w-sm overflow-hidden rounded-3xl border border-border bg-background shadow-2xl will-change-transform",
                  className,
                )}
              >
                <motion.div layout="position" className="p-5">
                  <AnimatePresence mode="popLayout" initial={false}>
                    <motion.div
                      key={viewId}
                      data-modal-view={viewId ?? undefined}
                      initial={initialAnimation
                        ? reduce
                          ? { opacity: 0 }
                          : { opacity: 0, y: 8, filter: "blur(4px)" }
                        : false}
                      animate={
                        reduce
                          ? { opacity: 1, transition: { duration: 0.18, ease: EASE_OUT } }
                          : {
                              opacity: 1,
                              y: 0,
                              filter: "blur(0px)",
                              transition: { duration: 0.24, ease: EASE_OUT },
                            }
                      }
                      exit={
                        reduce
                          ? { opacity: 0, transition: { duration: 0.14, ease: EASE_OUT } }
                          : {
                              opacity: 0,
                              y: -8,
                              filter: "blur(4px)",
                              transition: { duration: 0.16, ease: EASE_OUT },
                            }
                      }
                    >
                      {isPresent ? children : null}
                    </motion.div>
                  </AnimatePresence>
                </motion.div>
              </motion.div>
            </div>
          )}
        </PresenceGate>
      ) : null}
    </AnimatePresence>
  );
}
