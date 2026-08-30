import {
  motion,
  useMotionTemplate,
  useMotionValue,
  useReducedMotion,
  useSpring,
} from "motion/react";
import { useRef, type ReactNode } from "react";
import { SPRING_MOUSE } from "../../lib/ease";
import { useHoverCapable } from "../../lib/hooks/use-hover-capable";

type TiltCardProps = {
  children: ReactNode;
  className?: string;
  max?: number;
};

export function TiltCard({ children, className, max = 2.5 }: TiltCardProps) {
  const ref = useRef<HTMLDivElement>(null);
  const reduce = useReducedMotion();
  const canHover = useHoverCapable();
  const rotateX = useMotionValue(0);
  const rotateY = useMotionValue(0);
  const springX = useSpring(rotateX, SPRING_MOUSE);
  const springY = useSpring(rotateY, SPRING_MOUSE);
  const transform = useMotionTemplate`perspective(1000px) rotateX(${springX}deg) rotateY(${springY}deg)`;
  const enabled = !reduce && canHover;

  function reset() {
    rotateX.set(0);
    rotateY.set(0);
  }

  return (
    <motion.div
      ref={ref}
      className={className}
      data-motion={enabled ? "tilt" : "static"}
      style={enabled ? { transform, transformStyle: "preserve-3d" } : undefined}
      onPointerMove={(event) => {
        const element = ref.current;
        if (!element || !enabled) return;
        const bounds = element.getBoundingClientRect();
        rotateY.set(((event.clientX - bounds.left) / bounds.width - 0.5) * max);
        rotateX.set((0.5 - (event.clientY - bounds.top) / bounds.height) * max);
      }}
      onPointerLeave={reset}
    >
      {children}
    </motion.div>
  );
}
