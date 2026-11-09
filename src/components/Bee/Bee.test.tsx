import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Bee } from "./Bee";
import { BEE_STATES } from "@/types/view";

/** Lee un archivo de assets/bee del disco (jsdom no aplica CSS ni la CSP: se comprueba el contenido). */
const root = (...p: string[]): string => resolve(process.cwd(), ...p); // vitest se ejecuta desde la raíz del proyecto
const asset = (name: string): string => readFileSync(root("src", "assets", "bee", name), "utf8").replace(/\r\n/g, "\n");

describe("Bee (rig inline sin <style>)", () => {
  it("renderiza el rig inline SIN ningún <style> (la CSP de producción lo bloquearía) y con los 8 rostros", () => {
    const { container } = render(<Bee />);
    const svg = container.querySelector("svg.bee");
    expect(svg).not.toBeNull();
    expect(svg!.getAttribute("viewBox")).toBe("0 0 128 128");
    expect(svg!.querySelector("style")).toBeNull();
    expect(container.innerHTML).not.toContain("<style");
    expect(container.querySelectorAll("[style]").length).toBe(0); // ni atributos style en el SVG
    for (const s of BEE_STATES) expect(svg!.querySelector(`#face-${s}`)).not.toBeNull();
  });

  it.each(BEE_STATES)("aplica data-state=%s", (s) => {
    const { container } = render(<Bee state={s} />);
    expect(container.querySelector("svg")!.getAttribute("data-state")).toBe(s);
  });

  it("cambia de estado sin remontar el nodo SVG", () => {
    const { container, rerender } = render(<Bee state="idle" />);
    const before = container.querySelector("svg");
    rerender(<Bee state="happy" />);
    const after = container.querySelector("svg");
    expect(after).toBe(before);
    expect(after!.getAttribute("data-state")).toBe("happy");
  });

  it.each([64, 96, 128])("admite tamaño %i px manteniendo proporción", (n) => {
    const { container } = render(<Bee size={n} />);
    const svg = container.querySelector("svg")!;
    expect(svg.getAttribute("width")).toBe(String(n));
    expect(svg.getAttribute("height")).toBe(String(n));
  });
});

describe("bee.css (el CSS externo que sustituye al <style> embebido)", () => {
  const css = asset("bee.css");

  it("oculta los rostros inactivos y muestra solo el del estado activo (los 8 selectores)", () => {
    expect(css).toMatch(/\.bee \.face\{opacity:0;/);
    for (const s of BEE_STATES) {
      expect(css).toContain(`.bee[data-state="${s}"] .face-${s}`);
      expect(css).toContain(`.bee[data-state="${s}"]{--dx:`); // y su fila de la tabla de estados
    }
  });

  it("contiene los @keyframes de las animaciones originales", () => {
    for (const k of ["bee-float", "bee-sway", "bee-flap-l", "bee-flap-r", "bee-wig", "bee-blink", "bee-talk-a", "bee-talk-b", "bee-zz"]) {
      expect(css).toContain(`@keyframes ${k}`);
    }
  });

  it("respeta prefers-reduced-motion", () => {
    expect(css).toContain("@media (prefers-reduced-motion:reduce)");
    expect(css).toMatch(/animation:none!important/);
  });
});

describe("bee.css: capa de rendimiento (docs/ANIMATION_PERFORMANCE.md)", () => {
  const css = asset("bee.css");
  const perf = css.slice(css.indexOf("Capa de rendimiento"));

  it("los rostros inactivos no animan (los 8 estados)", () => {
    for (const s of BEE_STATES) {
      expect(perf).toContain(`.bee[data-state="${s}"] .face:not(.face-${s}) .eyes`);
      expect(perf).toContain(`.bee[data-state="${s}"] .face:not(.face-${s}) .zzz .z-1`);
    }
    expect(perf).toMatch(/\.z-2\{animation:none\}/);
  });

  it("resting solo anima balanceo y antenas, con steps(); flotación y aleteo (amplitud 0) no se animan", () => {
    expect(perf).toContain('.bee[data-state="resting"] .float{animation:none}');
    expect(perf).toContain('.bee[data-state="resting"] .flap{animation:none}');
    expect(perf).toMatch(/\.bee\[data-state="resting"\] \.sway\{animation-timing-function:steps\(\d+,end\)\}/);
    expect(perf).toMatch(/\.bee\[data-state="resting"\] \.antenna-l \.wiggle\{animation-timing-function:steps\(\d+,end\)\}/);
  });

  it("idle y sleepy cuantizan su movimiento con steps(), y sleepy conserva sus zzz visibles", () => {
    expect(perf).toMatch(/\.bee\[data-state="idle"\] \.flap\{animation-timing-function:steps\(\d+,end\)\}/);
    expect(perf).toMatch(/\.bee\[data-state="sleepy"\] \.zzz \.z-2\{animation-timing-function:steps\(\d+,end\)\}/);
    expect(perf).not.toMatch(/\.bee\[data-state="sleepy"\] \.flap\{animation:none\}/); // el aleteo de sleepy sí se ve
  });

  it("la capa no toca Paused/Hidden ni movimiento reducido (siguen en la tabla base y en app.css)", () => {
    expect(perf).not.toContain("animation-play-state");
    expect(perf).not.toContain("prefers-reduced-motion");
    expect(perf).not.toContain("!important");
  });
});

describe("derivación del rig sin estilo (tools/make_nostyle_rig.py)", () => {
  it("bee.css empieza exactamente por el <style> que traía el rig original (nada se perdió ni se cambió) y añade la capa de rendimiento", () => {
    const original = asset("bee-rig.svg");
    const m = /<style>\n([\s\S]*?)\n<\/style>/.exec(original);
    expect(m).not.toBeNull();
    const base = m![1]!.trim();
    const css = asset("bee.css").trim();
    expect(css.startsWith(base)).toBe(true);
    expect(css.slice(base.length)).toContain("Capa de rendimiento");
  });

  it("la ÚNICA diferencia entre bee-rig.svg y bee-rig.nostyle.svg es la ausencia del <style>", () => {
    const withoutStyle = asset("bee-rig.svg").replace(/<style>[\s\S]*?<\/style>\n/, "");
    expect(asset("bee-rig.nostyle.svg")).toBe(withoutStyle);
    expect(asset("bee-rig.nostyle.svg")).not.toContain("<style");
  });

  it("Bee.tsx importa el rig sin estilo y bee.css como archivo propio", () => {
    const src = readFileSync(root("src", "components", "Bee", "Bee.tsx"), "utf8");
    expect(src).toContain('bee-rig.nostyle.svg?react');
    expect(src).toContain('import "../../assets/bee/bee.css"');
    expect(src).not.toMatch(/bee-rig\.svg\?react/);
  });
});

describe("política CSP (tauri.conf.json)", () => {
  const conf = JSON.parse(
    readFileSync(root("src-tauri", "tauri.conf.json"), "utf8"),
  ) as { app: { security: { csp: string; devCsp: string } } };

  it("la CSP de producción NO permite estilos en línea ('unsafe-inline') ni conexiones externas", () => {
    const csp = conf.app.security.csp;
    const styleSrc = /style-src ([^;]*)/.exec(csp)?.[1] ?? "";
    expect(styleSrc).toBe("'self'");
    expect(csp).not.toContain("unsafe-inline");
    expect(csp).not.toContain("unsafe-eval");
    expect(csp).not.toMatch(/https?:\/\/(?!ipc\.localhost)/);
  });
});
