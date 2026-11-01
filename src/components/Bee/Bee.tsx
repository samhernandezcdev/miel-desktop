// El rig SIN <style> (derivado reproducible: tools/make_nostyle_rig.py) + su CSS como archivo propio.
// En producción Tauri añade un nonce a `style-src` y el navegador IGNORA 'unsafe-inline': un <style>
// embebido en el SVG quedaría bloqueado (8 rostros apilados y sin animación). El CSS empaquetado por
// Vite como archivo se sirve desde 'self', que sí está permitido.
import BeeRig from "@/assets/bee/bee-rig.nostyle.svg?react";
import "../../assets/bee/bee.css";
import type { BeeState } from "@/types/view";

export interface BeeProps {
  /** Expresión visual. Cambiar solo este valor; nunca desmontar el componente. */
  state?: BeeState;
  /** Lado del sprite en px (recomendado: 64, 96 o 128). */
  size?: number;
}

/**
 * Renderiza el rig maestro como SVG inline (sin <style>; ver arriba): las animaciones y la tabla de
 * estados viven en `bee.css` y se controlan con `data-state`.
 * Los ids del rig son fijos: solo debe haber una instancia montada.
 */
export function Bee({ state = "idle", size = 96 }: BeeProps) {
  return (
    <BeeRig
      data-state={state}
      width={size}
      height={size}
      aria-label={`Miel, ${state}`}
    />
  );
}
