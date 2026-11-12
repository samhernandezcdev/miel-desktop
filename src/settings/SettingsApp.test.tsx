import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SettingsApi, SettingsData } from "./api";
import SettingsApp from "./SettingsApp";
import { STRINGS } from "./strings";
import { NOTICE_MS } from "./useSettings";

const base = (over: Partial<SettingsData> = {}): SettingsData => ({
  schema_version: 2,
  follow_enabled: true,
  paused: false,
  messages_enabled: true,
  message_interval_minutes: 12,
  pet_size: 96,
  speed: 260,
  reduced_motion: false,
  dnd_auto_enabled: true,
  sleep_enabled: true,
  sleep_after_minutes: 10,
  proximity_enabled: true,
  time_of_day_enabled: true,
  ...over,
});

/** Frontera simulada de Rust: guarda el estado REAL, valida como Rust y emite el evento de cambio. */
function fakeRust(initial: SettingsData = base()) {
  let state = { ...initial };
  let listener: ((s: SettingsData) => void) | undefined;
  const unlisten = vi.fn();
  const calls: [string, unknown][] = [];
  const api: SettingsApi = {
    get: vi.fn(async () => ({ ...state })),
    set: vi.fn(async (key, value) => {
      calls.push([key, value]);
      const isBool = key === "messages_enabled" || key === "reduced_motion" || key === "dnd_auto_enabled" || key === "sleep_enabled";
      if (isBool ? typeof value !== "boolean" : typeof value !== "number") throw "tipo erróneo";
      let v = value;
      if (key === "speed") v = Math.min(600, Math.max(60, value as number)); // Rust sanea el rango
      if (key === "message_interval_minutes") v = Math.min(1440, Math.max(1, value as number));
      const next = { ...state, [key]: v };
      const changed = JSON.stringify(next) !== JSON.stringify(state);
      state = next;
      if (changed) listener?.({ ...state });
      return { ...state };
    }),
    onChanged: vi.fn(async (cb) => {
      listener = cb;
      return unlisten;
    }),
  };
  return {
    api,
    unlisten,
    calls,
    /** cambio hecho "desde fuera" (p. ej. por Rust): actualiza el estado real y avisa */
    pushFromRust(over: Partial<SettingsData>) {
      state = { ...state, ...over };
      act(() => listener?.({ ...state }));
    },
    real: () => ({ ...state }),
    setReal(over: Partial<SettingsData>) {
      state = { ...state, ...over };
    },
  };
}


// Sin jest-dom (no se añaden dependencias): comprobaciones directas sobre el DOM.
const input = (el: HTMLElement) => el as HTMLInputElement;
const value = (el: HTMLElement) => input(el).value;
const isChecked = (el: HTMLElement) => input(el).checked;
const present = (el: HTMLElement | null) => expect(el).not.toBeNull();

const es = STRINGS.es;
const en = STRINGS.en;

async function ready(r = fakeRust(), lang: "es" | "en" = "es") {
  const view = render(<SettingsApp api={r.api} lang={lang} />);
  await screen.findByLabelText(STRINGS[lang].speed.label);
  return { r, view };
}

beforeEach(() => {
  document.documentElement.lang = "";
});
afterEach(() => {
  vi.useRealTimers();
});

describe("carga", () => {
  it("lee con get_settings y muestra SOLO los ajustes de v0.1.0 de la lista cerrada", async () => {
    const { r } = await ready(fakeRust(base({ pet_size: 128, speed: 300, message_interval_minutes: 45, reduced_motion: true, messages_enabled: false })));
    expect(r.api.get).toHaveBeenCalledTimes(1);
    expect(value(screen.getByLabelText(es.speed.label))).toBe("300");
    expect(value(screen.getByLabelText(es.messageInterval.label))).toBe("45");
    expect(isChecked(screen.getByLabelText(es.messagesEnabled.label))).toBe(false);
    expect(isChecked(screen.getByLabelText(es.reducedMotion.label))).toBe(true);
    expect(isChecked(screen.getByLabelText(es.petSize.options["128"]))).toBe(true);
    expect(isChecked(screen.getByLabelText(es.petSize.options["96"]))).toBe(false);
  });

  it("NO muestra seguimiento ni pausa (solo bandeja) ni los interruptores de v0.2.0 que aún no existen (solo No molestar, E4b, y Sueño, E5)", async () => {
    await ready();
    const text = document.body.textContent ?? "";
    // los nombres de esos ajustes no aparecen como controles: los únicos interruptores son los 2 de v0.1.0 + No molestar + Sueño
    const switches = screen.getAllByRole("switch").map((e) => e.getAttribute("id"));
    expect(switches).toHaveLength(4);
    expect(screen.getAllByRole("combobox")).toHaveLength(1); // el tiempo de sueño
    expect(screen.getAllByRole("radio")).toHaveLength(3);
    expect(screen.getAllByRole("spinbutton")).toHaveLength(2);
    expect(screen.queryByRole("button")).toBeNull();
    for (const banned of ["Proximidad", "Hora del día", "dnd", "proximity"]) {
      expect(text).not.toContain(banned);
    }
  });

  it("registra el listener ANTES de leer y lo elimina al cerrar", async () => {
    const r = fakeRust();
    const order: string[] = [];
    (r.api.onChanged as ReturnType<typeof vi.fn>).mockImplementationOnce(async (cb) => {
      order.push("listen");
      await r.api.onChanged(cb);
      return r.unlisten;
    });
    (r.api.get as ReturnType<typeof vi.fn>).mockImplementationOnce(async () => {
      order.push("get");
      return r.real();
    });
    const { view } = await ready(r);
    expect(order).toEqual(["listen", "get"]);
    view.unmount();
    expect(r.unlisten).toHaveBeenCalled();
  });

  it("si la lectura inicial falla, avisa y permite reintentar", async () => {
    const r = fakeRust();
    (r.api.get as ReturnType<typeof vi.fn>).mockRejectedValueOnce("fallo");
    render(<SettingsApp api={r.api} lang="es" />);
    present(await screen.findByText(es.loadError));
    fireEvent.click(screen.getByRole("button", { name: es.retry }));
    present(await screen.findByLabelText(es.speed.label));
    expect(screen.queryByText(es.loadError)).toBeNull();
  });

  it("mientras carga muestra un estado de carga accesible", () => {
    const r = fakeRust();
    (r.api.get as ReturnType<typeof vi.fn>).mockReturnValue(new Promise(() => {}));
    render(<SettingsApp api={r.api} lang="es" />);
        present(screen.getByText(es.loading));
  });

  it("un evento que llega antes de la lectura inicial gana a la lectura (más antigua)", async () => {
    const r = fakeRust(base({ speed: 100 }));
    let release!: (s: SettingsData) => void;
    (r.api.get as ReturnType<typeof vi.fn>).mockReturnValueOnce(new Promise<SettingsData>((res) => (release = res)));
    render(<SettingsApp api={r.api} lang="es" />);
    await waitFor(() => expect(r.api.onChanged).toHaveBeenCalled());
    r.pushFromRust({ speed: 500 });
    await screen.findByLabelText(es.speed.label);
    await act(async () => release(base({ speed: 100 })));
    expect(value(screen.getByLabelText(es.speed.label))).toBe("500");
  });
});

describe("escritura con set_setting", () => {
  it("un interruptor envía el booleano y refleja el valor aplicado", async () => {
    const { r } = await ready();
    fireEvent.click(screen.getByLabelText(es.messagesEnabled.label));
    await waitFor(() => expect(isChecked(screen.getByLabelText(es.messagesEnabled.label))).toBe(false));
    expect(r.calls).toEqual([["messages_enabled", false]]);
  });

  it("el tamaño envía el número elegido", async () => {
    const { r } = await ready();
    fireEvent.click(screen.getByLabelText(es.petSize.options["64"]));
    await waitFor(() => expect(isChecked(screen.getByLabelText(es.petSize.options["64"]))).toBe(true));
    expect(r.calls).toEqual([["pet_size", 64]]);
  });

  it("un número se envía al salir del campo o con Enter, no en cada tecla", async () => {
    const { r } = await ready();
    const speed = screen.getByLabelText(es.speed.label);
    fireEvent.change(speed, { target: { value: "3" } });
    fireEvent.change(speed, { target: { value: "30" } });
    fireEvent.change(speed, { target: { value: "300" } });
    expect(r.calls).toEqual([]);
    fireEvent.keyDown(speed, { key: "Enter" });
    await waitFor(() => expect(r.calls).toEqual([["speed", 300]]));
    const interval = screen.getByLabelText(es.messageInterval.label);
    fireEvent.change(interval, { target: { value: "20" } });
    fireEvent.blur(interval);
    await waitFor(() => expect(r.calls).toEqual([["speed", 300], ["message_interval_minutes", 20]]));
  });

  it("no envía nada si el valor no cambió, y Escape descarta lo escrito", async () => {
    const { r } = await ready();
    const speed = screen.getByLabelText(es.speed.label);
    fireEvent.blur(speed);
    fireEvent.change(speed, { target: { value: "260" } });
    fireEvent.blur(speed);
    fireEvent.change(speed, { target: { value: "999" } });
    fireEvent.keyDown(speed, { key: "Escape" });
    expect(value(speed)).toBe("260");
    expect(r.calls).toEqual([]);
  });

  it("el frontend NO corrige: envía lo escrito y muestra lo que Rust aplicó, con un aviso", async () => {
    const { r } = await ready();
    const speed = screen.getByLabelText(es.speed.label);
    fireEvent.change(speed, { target: { value: "99999" } });
    fireEvent.blur(speed);
    await waitFor(() => expect(value(speed)).toBe("600"));
    expect(r.calls).toEqual([["speed", 99999]]); // sin recortar en la página
    const status = screen.getAllByRole("status").find((e) => e.classList.contains("notice-slot"))!;
    present(within(status).getByText(/600/));
    expect(status.textContent).toContain(es.speed.label);
  });
});

describe("rechazo de Rust", () => {
  it("muestra un aviso breve y restaura el control al valor REAL", async () => {
    const { r } = await ready();
    const interval = screen.getByLabelText(es.messageInterval.label);
    fireEvent.change(interval, { target: { value: "" } }); // vacío → se envía el texto; Rust lo rechaza
    fireEvent.blur(interval);
    await waitFor(() => expect(r.calls).toEqual([["message_interval_minutes", ""]]));
    await waitFor(() => expect(value(screen.getByLabelText(es.messageInterval.label))).toBe("12"));
    const slot = document.querySelector(".notice-slot")!;
    expect(slot.textContent).toContain(es.messageInterval.label);
    expect(slot.textContent).toMatch(/no es válido/);
    expect(r.real().message_interval_minutes).toBe(12);
  });

  it("restaura el valor real aunque este haya cambiado en Rust mientras tanto", async () => {
    const { r } = await ready();
    (r.api.set as ReturnType<typeof vi.fn>).mockImplementationOnce(async () => {
      r.setReal({ speed: 333 }); // el estado real cambió por otra vía
      throw "rechazado";
    });
    const speed = screen.getByLabelText(es.speed.label);
    fireEvent.change(speed, { target: { value: "400" } });
    fireEvent.blur(speed);
    await waitFor(() => expect(value(speed)).toBe("333"));
  });

  it("un interruptor rechazado vuelve a su estado real", async () => {
    const { r } = await ready();
    (r.api.set as ReturnType<typeof vi.fn>).mockRejectedValueOnce("no se pudo guardar");
    fireEvent.click(screen.getByLabelText(es.reducedMotion.label));
    await waitFor(() => expect(document.querySelector(".notice")).not.toBeNull());
    expect(isChecked(screen.getByLabelText(es.reducedMotion.label))).toBe(false);
    expect(r.real().reduced_motion).toBe(false);
  });

  it("el aviso es breve: desaparece solo", async () => {
    const { r } = await ready();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    (r.api.set as ReturnType<typeof vi.fn>).mockRejectedValueOnce("x");
    fireEvent.click(screen.getByLabelText(es.messagesEnabled.label));
    await waitFor(() => expect(document.querySelector(".notice")).not.toBeNull());
    await act(async () => {
      vi.advanceTimersByTime(NOTICE_MS + 50);
    });
    expect(document.querySelector(".notice")).toBeNull();
  });

  it("el aviso se anuncia en una región viva (sin mover el foco)", async () => {
    await ready();
    const slot = document.querySelector(".notice-slot")!;
    expect(slot.getAttribute("role")).toBe("status");
    expect(slot.getAttribute("aria-live")).toBe("polite");
  });
});

describe("evento settings://changed", () => {
  it("actualiza los controles cuando Rust cambia un ajuste", async () => {
    const { r } = await ready();
    r.pushFromRust({ pet_size: 128, reduced_motion: true, speed: 450, messages_enabled: false });
    await waitFor(() => expect(value(screen.getByLabelText(es.speed.label))).toBe("450"));
    expect(isChecked(screen.getByLabelText(es.petSize.options["128"]))).toBe(true);
    expect(isChecked(screen.getByLabelText(es.reducedMotion.label))).toBe(true);
    expect(isChecked(screen.getByLabelText(es.messagesEnabled.label))).toBe(false);
  });

  it("no pisa lo que la persona está escribiendo", async () => {
    const { r } = await ready();
    const speed = screen.getByLabelText(es.speed.label);
    fireEvent.change(speed, { target: { value: "123" } });
    r.pushFromRust({ messages_enabled: false });
    expect(value(speed)).toBe("123");
    expect(isChecked(screen.getByLabelText(es.messagesEnabled.label))).toBe(false);
  });
});

describe("idioma y accesibilidad", () => {
  it("en inglés todos los textos cambian y se fija lang y el título del documento", async () => {
    await ready(fakeRust(), "en");
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(en.heading);
    present(screen.getByLabelText(en.speed.label));
    expect(document.documentElement.lang).toBe("en");
    expect(document.title).toBe(en.windowTitle);
    expect(screen.queryByText(es.heading)).toBeNull();
  });

  it("en español fija lang=es", async () => {
    await ready();
    expect(document.documentElement.lang).toBe("es");
    expect(document.title).toBe(es.windowTitle);
  });

  it("cada control tiene etiqueta asociada y descripción (ayuda + «se aplica al reiniciar»)", async () => {
    await ready();
    for (const label of [es.speed.label, es.messageInterval.label, es.messagesEnabled.label, es.reducedMotion.label]) {
      const el = screen.getByLabelText(label);
      const ids = (el.getAttribute("aria-describedby") ?? "").split(" ").filter(Boolean);
      expect(ids.length, label).toBe(2);
      const text = ids.map((id) => document.getElementById(id)?.textContent ?? "").join(" ");
      expect(text, label).toContain(es.appliesOnRestart);
    }
    const group = screen.getByRole("radiogroup", { name: es.petSize.label });
    expect(group.getAttribute("aria-describedby")).toBeTruthy();
  });

  it("todos los controles son nativos, alcanzables por teclado y sin tabindex positivo", async () => {
    await ready();
    const controls = [...document.querySelectorAll("input, button, select, textarea")] as HTMLElement[];
    expect(controls.length).toBe(10);
    for (const c of controls) {
      expect(Number(c.getAttribute("tabindex") ?? 0), c.outerHTML).toBeLessThanOrEqual(0);
      expect(c.hasAttribute("disabled")).toBe(false);
    }
    expect(document.querySelectorAll("[onclick], [style]").length).toBe(0);
    // un solo h1 y secciones con título
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(screen.getAllByRole("heading", { level: 2 }).length).toBe(6);
  });

  it("dice qué ajustes se aplican al reiniciar (banner + una nota por ajuste) y que seguimiento/pausa están en la bandeja", async () => {
    await ready();
    present(screen.getByText(es.restartBanner));
    expect(screen.getAllByText(es.appliesOnRestart)).toHaveLength(5);
    present(screen.getByText(es.trayNote));
  });

  it("incluye la sección de privacidad estática", async () => {
    await ready();
    const h = screen.getByRole("heading", { name: es.sections.privacy });
    const section = h.closest("section")!;
    for (const item of es.privacy.items) present(within(section).getByText(item));
    present(within(section).getByText(es.privacy.storageNote));
  });
});

describe("No molestar automático (E4b)", () => {
  it("lee el valor real y lo muestra con su etiqueta, ayuda y la nota «se aplica de inmediato»", async () => {
    await ready(fakeRust(base({ dnd_auto_enabled: false })));
    const sw = screen.getByLabelText(es.dnd.label);
    expect(sw.getAttribute("role")).toBe("switch");
    expect(isChecked(sw)).toBe(false);
    const ids = (sw.getAttribute("aria-describedby") ?? "").split(" ").filter(Boolean);
    expect(ids).toHaveLength(2);
    const text = ids.map((id) => document.getElementById(id)?.textContent ?? "").join(" ");
    expect(text).toContain(es.dnd.hint);
    expect(text).toContain(es.appliesNow);
    expect(text).not.toContain(es.appliesOnRestart);
  });

  it("envía el booleano con set_setting y refleja el valor aplicado", async () => {
    const { r } = await ready();
    const sw = screen.getByLabelText(es.dnd.label);
    expect(isChecked(sw)).toBe(true);
    fireEvent.click(sw);
    await waitFor(() => expect(isChecked(screen.getByLabelText(es.dnd.label))).toBe(false));
    expect(r.calls).toEqual([["dnd_auto_enabled", false]]);
    expect(r.real().dnd_auto_enabled).toBe(false);
  });

  it("si Rust lo rechaza, avisa con el nombre del ajuste y restaura el valor real", async () => {
    const r = fakeRust();
    (r.api.set as ReturnType<typeof vi.fn>).mockImplementationOnce(async () => {
      throw "rechazado";
    });
    await ready(r);
    fireEvent.click(screen.getByLabelText(es.dnd.label));
    await screen.findByText(`No se pudo cambiar «${es.dnd.label}»: el valor no es válido. Se restauró el valor actual.`);
    expect(isChecked(screen.getByLabelText(es.dnd.label))).toBe(true);
  });

  it("sigue a Rust cuando el cambio llega por el evento", async () => {
    const { r } = await ready();
    r.pushFromRust({ dnd_auto_enabled: false });
    await waitFor(() => expect(isChecked(screen.getByLabelText(es.dnd.label))).toBe(false));
  });

  it("está en una sección propia con su título y en inglés cambia el texto", async () => {
    await ready(fakeRust(), "en");
    const h = screen.getByRole("heading", { name: en.sections.dnd });
    const section = h.closest("section")!;
    present(within(section).getByLabelText(en.dnd.label));
    present(within(section).getByText(en.appliesNow));
    expect(en.dnd.label).not.toBe(es.dnd.label);
  });

  it("el texto dice qué hace, que no es el asistente de concentración y que se aplica de inmediato (es + en)", () => {
    for (const s of [es, en]) {
      expect(s.dnd.hint.length).toBeGreaterThan(80);
      expect(s.appliesNow.length).toBeGreaterThan(5);
    }
    expect(es.dnd.hint).toMatch(/pantalla completa/i);
    expect(es.dnd.hint).toMatch(/asistente de concentración de Windows/i);
    expect(es.dnd.hint).toMatch(/Mostrar de todos modos/);
    expect(es.appliesNow).toMatch(/inmediato/i);
    expect(en.dnd.hint).toMatch(/full-screen/i);
    expect(en.dnd.hint).toMatch(/focus assist/i);
    expect(en.appliesNow).toMatch(/immediately/i);
  });

  it("la privacidad dice lo cierto: solo pregunta si hay pantalla completa, juego o presentación; no sabe cuál ni qué muestra", () => {
    const joined = (s: typeof es) => s.privacy.items.join("\n");
    expect(joined(es)).toMatch(/pantalla completa, un juego o una presentación/);
    expect(joined(es)).toMatch(/no sabe cuál es la aplicación ni qué muestra/);
    expect(joined(es)).toMatch(/no lo guarda/);
    expect(joined(en)).toMatch(/full-screen application, a game or a presentation/);
    expect(joined(en)).toMatch(/does not know which application it is or what it shows/);
    expect(joined(en)).toMatch(/does not store it/);
    // y lo que ya existía sigue diciéndose
    expect(joined(es)).toMatch(/no hace capturas de pantalla/);
    expect(joined(es)).toMatch(/no lee los títulos/);
  });

  it("el aviso de reinicio excluye explícitamente este ajuste, que actúa al momento", () => {
    expect(es.restartBanner).toMatch(/salvo «No molestar automático»/);
    expect(en.restartBanner).toMatch(/except “No molestar automático”/);
  });
});

describe("Sueño (E5)", () => {
  const sleepSwitch = () => screen.getByRole("switch", { name: es.sleep.label });
  const sleepSelect = () => screen.getByLabelText(es.sleep.after.label) as HTMLSelectElement;
  const describedText = (el: HTMLElement) =>
    (el.getAttribute("aria-describedby") ?? "")
      .split(" ")
      .filter(Boolean)
      .map((id) => document.getElementById(id)?.textContent ?? "")
      .join(" ");

  it("lee los valores reales: interruptor, tiempo, ayuda y la nota «se aplica de inmediato»", async () => {
    await ready(fakeRust(base({ sleep_enabled: false, sleep_after_minutes: 30 })));
    expect(sleepSwitch().getAttribute("role")).toBe("switch");
    expect(isChecked(sleepSwitch())).toBe(false);
    expect(sleepSelect().value).toBe("30");
    for (const el of [sleepSwitch(), sleepSelect()]) {
      const text = describedText(el);
      expect(text).toContain(el === sleepSwitch() ? es.sleep.hint : es.sleep.after.hint);
      expect(text).toContain(es.appliesNow);
      expect(text).not.toContain(es.appliesOnRestart);
    }
  });

  it("el selector ofrece exactamente 5, 10, 15, 30 y 60 minutos", async () => {
    await ready();
    expect(within(sleepSelect()).getAllByRole("option").map((o) => (o as HTMLOptionElement).value)).toEqual(["5", "10", "15", "30", "60"]);
    expect(within(sleepSelect()).getAllByRole("option").map((o) => o.textContent)).toEqual(["5 minutos", "10 minutos", "15 minutos", "30 minutos", "60 minutos"]);
  });

  it("envía el booleano y el número con set_setting y refleja el valor aplicado", async () => {
    const { r } = await ready();
    expect(isChecked(sleepSwitch())).toBe(true);
    fireEvent.click(sleepSwitch());
    await waitFor(() => expect(isChecked(sleepSwitch())).toBe(false));
    fireEvent.change(sleepSelect(), { target: { value: "15" } });
    await waitFor(() => expect(sleepSelect().value).toBe("15"));
    expect(r.calls).toEqual([
      ["sleep_enabled", false],
      ["sleep_after_minutes", 15],
    ]);
    expect(r.real().sleep_enabled).toBe(false);
    expect(r.real().sleep_after_minutes).toBe(15);
  });

  it("si Rust lo rechaza, avisa con el nombre del ajuste y restaura el valor real", async () => {
    const r = fakeRust();
    (r.api.set as ReturnType<typeof vi.fn>).mockImplementationOnce(async () => {
      throw "rechazado";
    });
    await ready(r);
    fireEvent.change(sleepSelect(), { target: { value: "60" } });
    await screen.findByText(`No se pudo cambiar «${es.sleep.after.label}»: el valor no es válido. Se restauró el valor actual.`);
    expect(sleepSelect().value).toBe("10");
  });

  it("sigue a Rust cuando el cambio llega por el evento", async () => {
    const { r } = await ready();
    r.pushFromRust({ sleep_enabled: false, sleep_after_minutes: 5 });
    await waitFor(() => expect(isChecked(sleepSwitch())).toBe(false));
    expect(sleepSelect().value).toBe("5");
  });

  it("está en una sección propia y en inglés cambia el texto", async () => {
    await ready(fakeRust(), "en");
    const section = screen.getByRole("heading", { name: en.sections.sleep }).closest("section")!;
    present(within(section).getByRole("switch", { name: en.sleep.label }));
    present(within(section).getByLabelText(en.sleep.after.label));
    expect(en.sleep.label).not.toBe(es.sleep.label);
    expect(within(section).getAllByText(en.appliesNow).length).toBe(2);
  });

  it("el texto dice qué hace, lo del bloqueo y la suspensión, y que se aplica de inmediato (es + en)", () => {
    expect(es.sleep.hint).toMatch(/teclado ni el ratón/);
    expect(es.sleep.hint).toMatch(/bloqueas la sesión/);
    expect(es.sleep.hint).toMatch(/suspende/);
    expect(en.sleep.hint).toMatch(/keyboard or the mouse/);
    expect(en.sleep.hint).toMatch(/lock the session/);
    expect(es.restartBanner).toMatch(/«Sueño»/);
    expect(en.restartBanner).toMatch(/“Sueño”/);
  });
});

