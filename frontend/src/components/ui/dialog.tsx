// Registry boundary: adapted from the vendored beUI MorphingModal component.
import { type ReactNode, useId } from "react";
import { MorphingModal, type MorphingModalProps } from "@/components/motion/morphing-modal";
import { cn } from "@/lib/utils";

type AccessibleName =
  | { title: NonNullable<ReactNode>; "aria-label"?: never }
  | { title?: never; "aria-label": string };

type SharedModalSurfaceProps = Omit<
  MorphingModalProps,
  "viewId" | "placement" | "children" | "dismissible"
> & {
  open: boolean;
  description?: ReactNode;
  "aria-describedby"?: string;
  contentClassName?: string;
  children: ReactNode;
};

type ModalSurfaceProps = SharedModalSurfaceProps &
  AccessibleName & {
    dismissible?: boolean;
  };

export type DialogProps = ModalSurfaceProps;
export type SheetProps = ModalSurfaceProps;
export type AlertDialogProps = SharedModalSurfaceProps &
  AccessibleName & {
  /** Defaults to false so destructive confirmation must use an explicit action. */
  dismissible?: boolean;
};

function ModalSurface({
  role,
  open,
  title,
  description,
  children,
  className,
  contentClassName,
  placement,
  ...props
}: ModalSurfaceProps & { role: "dialog" | "alertdialog"; placement: "bottom" | "center" }) {
  const titleId = useId();
  const descriptionId = useId();
  const hasTitle = title !== undefined;
  const hasDescription = description !== undefined;
  return (
    <MorphingModal
      viewId={open ? role : null}
      placement={placement}
      className={className}
      {...props}
    >
      <section
        role={role}
        aria-modal="true"
        aria-label={hasTitle ? undefined : props["aria-label"]}
        aria-labelledby={hasTitle ? titleId : undefined}
        aria-describedby={hasDescription ? descriptionId : props["aria-describedby"]}
        className={cn("beui-dialog space-y-3", contentClassName)}
      >
        {hasTitle ? <h2 id={titleId} className="text-lg font-semibold text-foreground">{title}</h2> : null}
        {hasDescription ? <p id={descriptionId} className="text-sm text-muted-foreground">{description}</p> : null}
        {children}
      </section>
    </MorphingModal>
  );
}

export function Dialog(props: DialogProps) {
  return <ModalSurface role="dialog" placement="center" {...props} />;
}

export function Sheet(props: SheetProps) {
  return <ModalSurface role="dialog" placement="bottom" {...props} />;
}

export function AlertDialog({ dismissible = false, ...props }: AlertDialogProps) {
  return <ModalSurface role="alertdialog" placement="center" dismissible={dismissible} {...props} />;
}
