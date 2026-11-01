import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { clampText, MAX_BUBBLE_CHARS, SpeechBubble } from "./SpeechBubble";

const msg = (id: string, text: string) => ({ id, text });

afterEach(() => vi.unstubAllGlobals());

describe("SpeechBubble", () => {
  it("sin mensaje renderiza solo la región accesible vacía", () => {
    const { container } = render(<SpeechBubble message={null} />);
    const region = container.querySelector(".bubble-slot")!;
    expect(region.getAttribute("role")).toBe("status");
    expect(region.getAttribute("aria-live")).toBe("polite");
    expect(container.querySelector(".bubble")).toBeNull();
  });

  it("con mensaje muestra el texto y el id", () => {
    render(<SpeechBubble message={msg("presence.here", "Aquí sigo.")} />);
    const bubble = screen.getByText("Aquí sigo.");
    expect(bubble.getAttribute("data-message-id")).toBe("presence.here");
    expect(bubble.getAttribute("data-state")).toBe("visible");
  });

  it("al quitar el mensaje inicia la salida y se desmonta al terminar la animación", () => {
    const { container, rerender } = render(<SpeechBubble message={msg("a", "Hola")} />);
    rerender(<SpeechBubble message={null} />);
    const bubble = container.querySelector(".bubble")!;
    expect(bubble.getAttribute("data-state")).toBe("leaving");
    expect(container.querySelector(".bubble")).not.toBeNull(); // sigue montada durante la salida
    fireEvent.animationEnd(bubble);
    expect(container.querySelector(".bubble")).toBeNull();
  });

  it("si llega un mensaje durante la salida, se cancela la salida", () => {
    const { container, rerender } = render(<SpeechBubble message={msg("a", "Uno")} />);
    rerender(<SpeechBubble message={null} />);
    rerender(<SpeechBubble message={msg("b", "Dos")} />);
    const bubble = container.querySelector(".bubble")!;
    expect(bubble.textContent).toBe("Dos");
    expect(bubble.getAttribute("data-state")).toBe("visible");
  });

  it("con movimiento reducido se desmonta al instante", () => {
    vi.stubGlobal("matchMedia", (q: string) => ({ matches: q.includes("reduce"), media: q }));
    const { container, rerender } = render(<SpeechBubble message={msg("a", "Hola")} />);
    rerender(<SpeechBubble message={null} />);
    expect(container.querySelector(".bubble")).toBeNull();
  });

  it("conserva saltos de línea explícitos (texto multilínea) sin tocar el contenido", () => {
    render(<SpeechBubble message={msg("m", "Primera línea\nSegunda línea")} />);
    const bubble = document.querySelector(".bubble")!;
    expect(bubble.textContent).toBe("Primera línea\nSegunda línea");
  });

  it("recorta textos demasiado largos con una elipsis", () => {
    const long = "palabra ".repeat(40);
    render(<SpeechBubble message={msg("l", long)} />);
    const text = document.querySelector(".bubble")!.textContent!;
    expect(Array.from(text).length).toBeLessThanOrEqual(MAX_BUBBLE_CHARS);
    expect(text.endsWith("…")).toBe(true);
  });

  it("no usa temporizadores propios (no hay setTimeout/setInterval al aparecer)", () => {
    const t = vi.spyOn(globalThis, "setTimeout");
    const i = vi.spyOn(globalThis, "setInterval");
    const { rerender } = render(<SpeechBubble message={null} />);
    rerender(<SpeechBubble message={msg("a", "Hola")} />);
    rerender(<SpeechBubble message={null} />);
    expect(i).not.toHaveBeenCalled();
    expect(t.mock.calls.filter(([, ms]) => ms !== undefined && (ms as number) > 0)).toHaveLength(0);
    t.mockRestore();
    i.mockRestore();
  });
});

describe("clampText", () => {
  it("no toca textos cortos y cuenta caracteres Unicode, no unidades UTF-16", () => {
    expect(clampText("  ¡Qué bonito día!  ")).toBe("¡Qué bonito día!");
    expect(clampText("🐝".repeat(10), 5)).toBe("🐝🐝🐝🐝…");
  });
});
