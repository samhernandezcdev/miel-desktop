import { useEffect, useRef, useState } from "react";
import type { BubbleMessage } from "@/types/view";
import "./SpeechBubble.css";

/** Tope defensivo: la burbuja cabe en ≤3 líneas dentro del espacio sobre la abeja. */
export const MAX_BUBBLE_CHARS = 64;

export function clampText(text: string, max = MAX_BUBBLE_CHARS): string {
  const chars = Array.from(text.trim());
  return chars.length <= max ? chars.join("") : `${chars.slice(0, max - 1).join("").trimEnd()}…`;
}

function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export interface SpeechBubbleProps {
  /** Mensaje actual; `null` cuando Rust decide que ya no hay burbuja. */
  message: BubbleMessage | null;
}

/**
 * Burbuja de diálogo puramente presentacional. No tiene temporizadores de negocio: aparece y
 * desaparece cuando Rust lo indica. La salida es una animación CSS; al terminar (`animationend`,
 * única animación en curso mientras `leaving`) se desmonta. Con movimiento reducido se desmonta al instante.
 * Región `role="status"` siempre montada para que los lectores de pantalla anuncien los cambios.
 */
export function SpeechBubble({ message }: SpeechBubbleProps) {
  const [shown, setShown] = useState<BubbleMessage | null>(message);
  const [leaving, setLeaving] = useState(false);
  const bubbleRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (message) {
      setShown(message);
      setLeaving(false);
    } else if (prefersReducedMotion()) {
      setShown(null);
      setLeaving(false);
    } else {
      setLeaving(true);
    }
  }, [message]);

  // Fin de la animación de salida: listener nativo (más fiable que el sintético en entornos sin
  // AnimationEvent) que se registra solo mientras `leaving` y se limpia siempre.
  useEffect(() => {
    const el = bubbleRef.current;
    if (!leaving || !el) return;
    const done = () => {
      setShown(null);
      setLeaving(false);
    };
    el.addEventListener("animationend", done, { once: true });
    return () => el.removeEventListener("animationend", done);
  }, [leaving, shown]);

  return (
    <div className="bubble-slot" role="status" aria-live="polite" aria-atomic="true">
      {shown && (
        <div
          key={shown.id}
          ref={bubbleRef}
          className="bubble"
          data-state={leaving ? "leaving" : "visible"}
          data-message-id={shown.id}
        >
          {clampText(shown.text)}
        </div>
      )}
    </div>
  );
}
