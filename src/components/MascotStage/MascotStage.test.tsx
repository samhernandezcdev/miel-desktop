import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MascotStage } from "./MascotStage";
import type { MascotView } from "@/types/view";

const view = (over: Partial<MascotView> = {}): MascotView => ({
  version: 1, behavior: "idle", expression: "idle", paused: false, hidden: false, followEnabled: true, message: null, ...over,
});

describe("MascotStage (abeja + burbuja)", () => {
  it("muestra la abeja con la expresión recibida y sin burbuja", () => {
    const { container } = render(<MascotStage view={view({ expression: "happy" })} />);
    expect(container.querySelector("svg.bee")!.getAttribute("data-state")).toBe("happy");
    expect(container.querySelector(".bubble")).toBeNull();
  });

  it("muestra la burbuja junto a la abeja y conserva el MISMO nodo SVG al cambiar mensaje y expresión", () => {
    const { container, rerender } = render(<MascotStage view={view()} />);
    const svg = container.querySelector("svg.bee");
    rerender(
      <MascotStage
        view={view({ version: 2, expression: "sleepy", message: { id: "rest.little", text: "Voy a descansar un ratito." } })}
      />,
    );
    expect(container.querySelector(".bubble")!.textContent).toBe("Voy a descansar un ratito.");
    expect(container.querySelector("svg.bee")).toBe(svg);
    expect(svg!.getAttribute("data-state")).toBe("sleepy");
    rerender(<MascotStage view={view({ version: 3, expression: "resting" })} />);
    expect(container.querySelector("svg.bee")).toBe(svg);
    expect(svg!.getAttribute("data-state")).toBe("resting");
  });

  it("burbuja y sprite comparten el ancla y el tamaño configurables", () => {
    const { container } = render(<MascotStage view={view()} size={128} />);
    const stage = container.querySelector<HTMLElement>(".stage")!;
    expect(stage.style.getPropertyValue("--bee-size")).toBe("128px");
    expect(container.querySelector("svg.bee")!.getAttribute("width")).toBe("128");
    expect(container.querySelector(".sprite")).not.toBeNull();
    expect(container.querySelector(".bubble-slot")).not.toBeNull();
  });

  it("marca la escena como pausada cuando está en pausa u oculta", () => {
    const a = render(<MascotStage view={view({ paused: true })} />);
    expect(a.container.querySelector(".stage")!.hasAttribute("data-paused")).toBe(true);
    const b = render(<MascotStage view={view({ hidden: true })} />);
    expect(b.container.querySelector(".stage")!.hasAttribute("data-paused")).toBe(true);
    const c = render(<MascotStage view={view()} />);
    expect(c.container.querySelector(".stage")!.hasAttribute("data-paused")).toBe(false);
  });
});

describe("MascotStage: tamaño y movimiento reducido (Etapa 7)", () => {
  it("ancla el sprite según el tamaño (misma fórmula que Rust)", () => {
    for (const [size, anchor] of [[64, 112], [96, 128], [128, 144]] as const) {
      const { container, unmount } = render(<MascotStage view={view()} size={size} />);
      const stage = container.querySelector<HTMLElement>(".stage")!;
      expect(stage.style.getPropertyValue("--bee-anchor-y")).toBe(`${anchor}px`);
      expect(stage.style.getPropertyValue("--bee-size")).toBe(`${size}px`);
      unmount();
    }
  });

  it("marca movimiento reducido solo si la preferencia lo fuerza", () => {
    const off = render(<MascotStage view={view()} />);
    expect(off.container.querySelector(".stage")!.hasAttribute("data-reduced-motion")).toBe(false);
    off.unmount();
    const on = render(<MascotStage view={view()} reducedMotion />);
    expect(on.container.querySelector(".stage")!.hasAttribute("data-reduced-motion")).toBe(true);
  });
});
