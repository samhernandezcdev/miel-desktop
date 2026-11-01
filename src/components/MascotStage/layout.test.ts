import { describe, expect, it } from "vitest";
import { ALLOWED_SIZES, BOTTOM_SPACE, BUBBLE_SPACE, metrics, sanitizeSize } from "./layout";

describe("layout (espejo de window.rs)", () => {
  it("el tamaño de referencia coincide con la ventana original 260×200 y ancla a 128 px", () => {
    expect(metrics(96)).toEqual({ width: 260, height: 200, anchorY: 128 });
  });

  it("mismos números que las pruebas de Rust para 64 y 128", () => {
    expect(metrics(64).height).toBe(168);
    expect(metrics(128).height).toBe(232);
  });

  it("siempre deja el mismo espacio para la burbuja arriba y la holgura abajo", () => {
    for (const s of ALLOWED_SIZES) {
      const m = metrics(s);
      expect(m.anchorY - s / 2).toBe(BUBBLE_SPACE);
      expect(m.height - (m.anchorY + s / 2)).toBe(BOTTOM_SPACE);
      expect(m.width).toBeGreaterThanOrEqual(220 + 40); // burbuja de 220 px con margen
    }
  });

  it("solo admite 64, 96 y 128; cualquier otra cosa vuelve a 96", () => {
    expect(sanitizeSize(64)).toBe(64);
    expect(sanitizeSize(128)).toBe(128);
    for (const bad of [0, 100, "96", null, undefined, NaN, 4096]) expect(sanitizeSize(bad)).toBe(96);
  });
});
