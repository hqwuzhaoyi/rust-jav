// Registry boundary: adapted from the vendored beUI MorphingModal component.
import { type ReactNode } from "react";
import { MorphingModal, type MorphingModalProps } from "@/components/motion/morphing-modal";

export type DialogProps = Omit<MorphingModalProps, "viewId" | "placement"> & {
  open: boolean;
  children: ReactNode;
};

export function Dialog({ open, ...props }: DialogProps) {
  return <MorphingModal viewId={open ? "dialog" : null} placement="center" {...props} />;
}

export function Sheet({ open, ...props }: DialogProps) {
  return <MorphingModal viewId={open ? "sheet" : null} placement="bottom" {...props} />;
}
