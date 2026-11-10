import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MascotView } from "@/types/view";

const v = (version: number, expression: MascotView["expression"] = "idle"): MascotView => ({
  version, behavior: "idle", expression, paused: false, hidden: false, followEnabled: true, message: null,
});

type Handler = (e: { payload: MascotView }) => void;
let handler: Handler | undefined;
const unlisten = vi.fn();
let snapshot: MascotView = v(0);
let resolveListen: (() => void) | undefined;

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_n: string, h: Handler) => {
    handler = h;
    return new Promise<() => void>((res) => { resolveListen = () => res(unlisten); });
  }),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => snapshot) }));

import { reduceView, subscribeView } from "./mascotEvents";

describe("reduceView", () => {
  it("acepta la primera y las más nuevas; descarta iguales y antiguas", () => {
    expect(reduceView(null, v(3))?.version).toBe(3);
    expect(reduceView(v(3), v(4))?.version).toBe(4);
    expect(reduceView(v(4), v(4, "happy"))?.expression).toBe("idle");
    expect(reduceView(v(4), v(2, "happy"))?.version).toBe(4);
  });

  it("una vista de v0.1.0 (sin suppressed ni sleeping) se acepta y equivale a false", () => {
    const old = JSON.parse(
      '{"version":5,"behavior":"rest","expression":"sleepy","paused":false,"hidden":false,"followEnabled":true,"message":null}',
    ) as MascotView;
    expect(reduceView(null, old)).toBe(old);
    expect(old.suppressed ?? false).toBe(false);
    expect(old.sleeping ?? false).toBe(false);
  });

  it("los campos nuevos no alteran el orden por versión", () => {
    const neu: MascotView = { ...v(6), suppressed: true, sleeping: true };
    expect(reduceView(v(5), neu)).toBe(neu);
    expect(reduceView(neu, v(5))).toBe(neu);
  });
});

describe("subscribeView", () => {
  beforeEach(() => { handler = undefined; unlisten.mockClear(); snapshot = v(0); });

  it("pide el snapshot tras suscribirse y no deja que un snapshot viejo pise un evento nuevo", async () => {
    const got: number[] = [];
    subscribeView((x) => got.push(x.version));
    resolveListen!();
    await vi.waitFor(() => expect(got).toContain(0));
    handler!({ payload: v(5, "happy") });
    handler!({ payload: v(3, "sleepy") }); // llega tarde
    expect(got).toEqual([0, 5]);
  });

  it("un evento más nuevo que el snapshot gana aunque llegue antes que él", async () => {
    snapshot = v(2);
    const got: number[] = [];
    subscribeView((x) => got.push(x.version));
    handler!({ payload: v(7) }); // evento antes de que termine la suscripción
    resolveListen!();
    await vi.waitFor(() => expect(got).toEqual([7]));
  });

  it("limpia el listener al desmontar, incluso antes de terminar la suscripción", async () => {
    const stop = subscribeView(() => {});
    stop();
    resolveListen!();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });

  it("limpia el listener al desmontar después de suscrito", async () => {
    const stop = subscribeView(() => {});
    resolveListen!();
    await new Promise((r) => setTimeout(r, 10));
    stop();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
