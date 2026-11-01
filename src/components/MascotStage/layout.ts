/**
 * Geometría de la ventana en función del tamaño del sprite. ESPEJO de `window::metrics` en
 * `src-tauri/src/window.rs` (hay una prueba con los mismos números en cada lado).
 */
export const WINDOW_WIDTH = 260;
/** Espacio sobre el sprite reservado para la burbuja. */
export const BUBBLE_SPACE = 80;
/** Espacio bajo el sprite (patas, flotación y holgura). */
export const BOTTOM_SPACE = 24;

export const ALLOWED_SIZES = [64, 96, 128] as const;
export type PetSize = (typeof ALLOWED_SIZES)[number];

export interface Metrics {
  width: number;
  height: number;
  /** Distancia del centro del sprite al borde superior de la ventana. */
  anchorY: number;
}

export function metrics(size: number): Metrics {
  return { width: WINDOW_WIDTH, height: BUBBLE_SPACE + size + BOTTOM_SPACE, anchorY: BUBBLE_SPACE + size / 2 };
}

/** Acepta solo los tamaños permitidos; cualquier otro valor vuelve a 96. */
export function sanitizeSize(n: unknown): PetSize {
  return (ALLOWED_SIZES as readonly number[]).includes(n as number) ? (n as PetSize) : 96;
}
