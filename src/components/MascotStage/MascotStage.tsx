import { Bee } from "@/components/Bee/Bee";
import { SpeechBubble } from "@/components/SpeechBubble/SpeechBubble";
import type { MascotView } from "@/types/view";
import { metrics } from "./layout";

export interface MascotStageProps {
  view: MascotView;
  /** Lado del sprite (64, 96 o 128); define la geometría de la ventana (ver layout.ts). */
  size?: number;
  /** Fuerza movimiento reducido además de la preferencia del sistema. */
  reducedMotion?: boolean;
}

/**
 * Escena de la ventana: abeja + burbuja, a partir del estado que publica Rust.
 * La abeja se ancla a `anchorY` px del borde superior (el espacio superior es para la burbuja);
 * la fórmula es la misma que usa Rust para dimensionar la ventana (`layout.ts` <-> `window.rs`).
 */
export function MascotStage({ view, size = 96, reducedMotion = false }: MascotStageProps) {
  const m = metrics(size);
  return (
    <div
      className="stage"
      style={{ ["--bee-size" as string]: `${size}px`, ["--bee-anchor-y" as string]: `${m.anchorY}px` }}
      data-paused={view.paused || view.hidden ? "" : undefined}
      data-reduced-motion={reducedMotion ? "" : undefined}
    >
      <div className="sprite">
        <Bee state={view.expression} size={size} />
      </div>
      <SpeechBubble message={view.message} />
    </div>
  );
}
