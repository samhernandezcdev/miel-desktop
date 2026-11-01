import { beforeEach, describe, expect, it, vi } from "vitest";

// Doble manual de `invoke` (sin vi.fn): con vi.fn() vitest 5 deja sin manejar la promesa derivada que
// rastrea cuando el mock rechaza, y el fallo se atribuye a la prueba aunque el código lo capture.
const calls: unknown[][] = [];
let mode: "ok" | "fail" = "ok";
let result: unknown = undefined;
vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (...a: unknown[]) => {
    calls.push(a);
    if (mode === "fail") throw new Error("sin canal");
    return result;
  },
}));

import { compact, installGlobalErrorReporting, reportError } from "./errorReport";
import { loadDisplayPrefs } from "./displayPrefs";

beforeEach(() => {
  calls.length = 0;
  mode = "ok";
  result = undefined;
});

describe("errorReport", () => {
  it("compacta espacios y limita la longitud", () => {
    expect(compact("  a \n\n  b\tc  ")).toBe("a b c");
    expect(compact("x".repeat(1000)).length).toBe(400);
  });

  it("informa a Rust con el comando restringido", async () => {
    await reportError("render", "boom");
    expect(calls).toEqual([["report_frontend_error", { kind: "render", message: "boom" }]]);
  });

  it("nunca lanza aunque el informe falle", async () => {
    mode = "fail";
    await expect(reportError("render", "boom")).resolves.toBeUndefined();
    expect(calls).toHaveLength(1);
  });

  it("captura errores globales y promesas rechazadas, y se desinstala", () => {
    const off = installGlobalErrorReporting();
    window.dispatchEvent(new ErrorEvent("error", { message: "fuera del render", lineno: 3, colno: 4 }));
    const rej = new Event("unhandledrejection") as Event & { reason: unknown };
    rej.reason = new Error("promesa");
    window.dispatchEvent(rej);
    expect(calls).toHaveLength(2);
    expect(calls[0]?.[1]).toMatchObject({ kind: "error" });
    expect(calls[1]?.[1]).toMatchObject({ kind: "unhandledrejection" });
    off();
    window.dispatchEvent(new ErrorEvent("error", { message: "otra" }));
    expect(calls).toHaveLength(2);
  });
});

describe("loadDisplayPrefs", () => {
  it("acepta los tamaños permitidos y el movimiento reducido", async () => {
    result = { petSize: 128, reducedMotion: true };
    expect(await loadDisplayPrefs()).toEqual({ petSize: 128, reducedMotion: true });
  });

  it("valores inesperados vuelven a los estables (96 px, sin forzar movimiento reducido)", async () => {
    result = { petSize: 777, reducedMotion: "sí" };
    expect(await loadDisplayPrefs()).toEqual({ petSize: 96, reducedMotion: false });
  });

  it("si Rust no responde usa los valores por defecto", async () => {
    mode = "fail";
    expect(await loadDisplayPrefs()).toEqual({ petSize: 96, reducedMotion: false });
  });
});
