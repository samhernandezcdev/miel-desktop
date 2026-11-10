import { describe, expect, it } from "vitest";
import { DEFAULT_LANG, fill, LANGS, pickLanguage, STRINGS } from "./strings";

describe("idioma", () => {
  it("elige el primer idioma soportado de la lista de la persona", () => {
    expect(pickLanguage(["es-MX", "en-US"])).toBe("es");
    expect(pickLanguage(["en-GB", "es"])).toBe("en");
    expect(pickLanguage(["fr-FR", "en-US"])).toBe("en");
    expect(pickLanguage(["EN_us"])).toBe("en");
  });

  it("sin idioma soportado o sin lista, español", () => {
    expect(pickLanguage(["fr", "de-DE"])).toBe("es");
    expect(pickLanguage([])).toBe("es");
    expect(pickLanguage(undefined)).toBe(DEFAULT_LANG);
    expect(DEFAULT_LANG).toBe("es");
  });
});

/** Todas las rutas de texto de un objeto de cadenas. */
function paths(o: unknown, prefix = ""): [string, string][] {
  if (typeof o === "string") return [[prefix, o]];
  if (Array.isArray(o)) return o.flatMap((v, i) => paths(v, `${prefix}[${i}]`));
  return Object.entries(o as Record<string, unknown>).flatMap(([k, v]) => paths(v, prefix ? `${prefix}.${k}` : k));
}

describe("textos es/en", () => {
  it("ambos idiomas tienen exactamente las mismas claves y ninguna está vacía", () => {
    const es = paths(STRINGS.es);
    const en = paths(STRINGS.en);
    expect(en.map(([p]) => p)).toEqual(es.map(([p]) => p));
    for (const [p, v] of [...es, ...en]) expect(v.trim(), p).not.toBe("");
    expect(LANGS).toEqual(["es", "en"]);
  });

  it("los marcadores {…} coinciden entre idiomas", () => {
    const marks = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    expect(marks(STRINGS.en.rejected)).toEqual(marks(STRINGS.es.rejected));
    expect(marks(STRINGS.en.adjusted)).toEqual(marks(STRINGS.es.adjusted));
    expect(marks(STRINGS.es.rejected)).toEqual(["name"]);
    expect(marks(STRINGS.es.adjusted)).toEqual(["name", "value"]);
  });

  it("dicen claramente que los ajustes se aplican al reiniciar (en ambos idiomas)", () => {
    expect(STRINGS.es.appliesOnRestart).toMatch(/reiniciar/i);
    expect(STRINGS.es.restartBanner).toMatch(/próxima vez/i);
    expect(STRINGS.en.appliesOnRestart).toMatch(/restart/i);
    expect(STRINGS.en.restartBanner).toMatch(/next time/i);
  });

  it("la sección de privacidad no afirma funciones que aún no existen", () => {
    const all = [...paths(STRINGS.es.privacy), ...paths(STRINGS.en.privacy)].map(([, v]) => v.toLowerCase()).join("\n");
    for (const future of ["no molestar", "do not disturb", "dormir", "sleep", "proximidad", "proximity", "franja", "time of day", "getlastinputinfo", "hora local", "local time"]) {
      expect(all, future).not.toContain(future);
    }
  });

  it("fill sustituye marcadores y deja los desconocidos", () => {
    expect(fill("a {x} b {y}", { x: 1 })).toBe("a 1 b {y}");
  });
});
