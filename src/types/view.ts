/** Los ocho estados visuales de Miel (ver docs/BEE_GUIDE.md §4). */
export const BEE_STATES = [
  "idle",
  "happy",
  "curious",
  "thinking",
  "sleepy",
  "surprised",
  "talking",
  "resting",
] as const;

export type BeeState = (typeof BEE_STATES)[number];

export type Behavior = "idle" | "follow" | "rest" | "reaction";

/** Mensaje visible decidido por Rust. React solo lo representa. */
export interface BubbleMessage {
  id: string;
  text: string;
}

/** Estado publicado por Rust (fuente única de verdad). `version` crece con cada cambio. */
export interface MascotView {
  version: number;
  behavior: Behavior;
  expression: BeeState;
  paused: boolean;
  hidden: boolean;
  /** ¿Se permite el seguimiento del cursor? (lo controla la bandeja; React no lo usa) */
  followEnabled: boolean;
  /** Mensaje visible (o null). Rust decide cuándo aparece y cuándo termina. */
  message: BubbleMessage | null;
  /**
   * Supresión automática en vigor (p. ej. No molestar). Aditivo (v0.2.0): opcional, ausente = `false`
   * (una vista de v0.1.0 no lo trae). React no lo usa todavía.
   */
  suppressed?: boolean;
  /** Miel duerme. Aditivo (v0.2.0): opcional, ausente = `false`. React no lo usa todavía. */
  sleeping?: boolean;
}
