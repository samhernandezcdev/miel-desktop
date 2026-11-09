import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
// @ts-expect-error tools/check_dist.mjs es JavaScript sin declaraciones de tipos
import { checkDist, checkPerfLayer } from "../../tools/check_dist.mjs";

const css = readFileSync(resolve(process.cwd(), "src", "assets", "bee", "bee.css"), "utf8").replace(/\r\n/g, "\n");
const perfAt = css.indexOf("/* ── Capa de rendimiento");

describe("check:dist protege la capa de rendimiento (E0b)", () => {
  it("el CSS actual la cumple y checkDist no reporta errores", () => {
    expect(checkPerfLayer(css)).toEqual([]);
    expect(checkDist(css, "")).toEqual([]);
  });

  it("falla si se pierde la capa completa", () => {
    const errors: string[] = checkPerfLayer(css.slice(0, perfAt));
    expect(errors.length).toBeGreaterThan(10);
  });

  it.each([
    ["rostros inactivos de resting", /\.bee\[data-state="resting"\] \.face:not\(\.face-resting\) \.eyes,/],
    ["flotación de resting", /\.bee\[data-state="resting"\] \.float\{animation:none\}/],
    ["steps del balanceo de resting", /\.bee\[data-state="resting"\] \.sway\{animation-timing-function:steps\(\d+,end\)\}/],
    ["steps del aleteo de idle", /\.bee\[data-state="idle"\] \.flap\{animation-timing-function:steps\(\d+,end\)\}/],
    ["steps de las zzz de sleepy", /\.bee\[data-state="sleepy"\] \.zzz \.z-2\{animation-timing-function:steps\(\d+,end\)\}/],
  ])("falla si falta: %s", (_name, re) => {
    const broken = css.replace(re, "/*quitado*/");
    expect(broken).not.toBe(css);
    expect(checkPerfLayer(broken).length).toBeGreaterThan(0);
  });

  it("falla si la capa queda antes de la tabla base", () => {
    const base = css.slice(0, perfAt);
    const perf = css.slice(perfAt);
    expect(checkPerfLayer(perf + "\n" + base)).toContain("la capa de rendimiento aparece antes de la tabla base: la base la anularía");
  });
});
