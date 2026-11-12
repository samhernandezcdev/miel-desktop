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

  it("la sección de privacidad no afirma funciones que aún no existen (No molestar existe desde E4b y Sueño desde E5)", () => {
    const all = [...paths(STRINGS.es.privacy), ...paths(STRINGS.en.privacy)].map(([, v]) => v.toLowerCase()).join("\n");
    for (const future of ["proximidad", "proximity", "franja", "time of day", "getlastinputinfo", "hora local", "local time"]) {
      expect(all, future).not.toContain(future);
    }
  });

  // Sustituye a las palabras prohibidas que se quitaron en E4b («no molestar», «do not disturb») y en E5 («dormir», «sleep»):
  // ahora esas funciones existen y su texto verdadero debe poder decirse, pero NO se pueden afirmar cosas falsas sobre ellas.
  // Cada patrón describe una afirmación que Miel NO cumple y exige que no vaya precedida de una negación («no», «ni», «not»).
  const FALSE_CLAIMS_ES: [string, RegExp][] = [
    ["sabe/lee/guarda teclas, aplicaciones, juegos, ventanas o títulos", /(?<!no )(?<!ni )(?<!sin )(sabe|conoce|lee|registra|guarda|envía|detecta|identifica) (qué |cuál |cuáles |las |los |el |la |tu |tus )?(teclas?|aplicaci[oó]n|aplicaciones|juegos?|programas?|ventanas?|t[ií]tulos?)/i],
    ["historial o registro de actividad", /(?<!no )(?<!ni )(guarda|mantiene|lleva|conserva) (un )?(historial|registro) de (lo que|tu|tus|las|los)/i],
    ["envía o sube datos", /(?<!no )(?<!ni )(envía|sube|comparte|transmite) (datos|información|estad[ií]sticas|el estado)/i],
    ["lee el asistente de concentración", /(?<!no )(?<!ni )(lee|detecta|respeta|sigue) el asistente de concentraci[oó]n/i],
  ];
  const FALSE_CLAIMS_EN: [string, RegExp][] = [
    ["knows/reads/stores keys, applications, games, windows or titles", /(?<!not )(?<!nor )(?<!without )(knows|reads|records|stores|sends|detects|identifies) (which |what |the |your )?(keys?|applications?|games?|programs?|windows|titles?)/i],
    ["history or activity log", /(?<!not )(?<!nor )(keeps|stores|saves|maintains) (a )?(history|log|record) of (what|your|the)/i],
    ["sends or uploads data", /(?<!not )(?<!nor )(sends|uploads|shares|transmits) (data|information|statistics|the state)/i],
    ["reads focus assist", /(?<!not )(?<!nor )(reads|detects|respects|follows) (windows' )?focus assist/i],
  ];

  it("la sección de privacidad no afirma NADA falso sobre No molestar y Sueño (afirmaciones precisas, no palabras sueltas)", () => {
    const check = (text: string, claims: [string, RegExp][]) => claims.filter(([, re]) => re.test(text)).map(([name]) => name);
    expect(check(STRINGS.es.privacy.items.join("\n"), FALSE_CLAIMS_ES)).toEqual([]);
    expect(check(STRINGS.en.privacy.items.join("\n"), FALSE_CLAIMS_EN)).toEqual([]);
    // y los textos de las propias funciones tampoco
    const feature = (s: typeof STRINGS.es) => [s.dnd.hint, s.sleep.hint, s.sleep.after.hint].join("\n");
    expect(check(feature(STRINGS.es), FALSE_CLAIMS_ES)).toEqual([]);
    expect(check(feature(STRINGS.en), FALSE_CLAIMS_EN)).toEqual([]);
  });

  it("los patrones de afirmaciones falsas SÍ detectan frases falsas y NO el texto verdadero (control negativo)", () => {
    const hits = (text: string, claims: [string, RegExp][]) => claims.some(([, re]) => re.test(text));
    for (const falso of [
      "Miel sabe qué aplicación está a pantalla completa.",
      "Miel lee las teclas que pulsas para saber si estás inactivo.",
      "Miel guarda un historial de lo que haces.",
      "Miel envía datos a un servidor.",
      "Miel detecta el asistente de concentración de Windows.",
    ]) {
      expect(hits(falso, FALSE_CLAIMS_ES), falso).toBe(true);
    }
    for (const falso of [
      "Miel knows which application is full screen.",
      "Miel reads the keys you press.",
      "Miel keeps a history of what you do.",
      "Miel sends data to a server.",
      "Miel detects Windows' focus assist.",
    ]) {
      expect(hits(falso, FALSE_CLAIMS_EN), falso).toBe(true);
    }
    for (const verdadero of [
      "Miel no sabe cuál es la aplicación ni qué muestra, y no lo guarda.",
      "No lee lo que escribes y no lee los títulos ni el contenido de tus ventanas.",
      "Le pregunta a Windows cuánto tiempo lleva sin usarse el teclado o el ratón (solo ese tiempo: no sabe qué teclas ni qué haces).",
      "No guarda historial de lo que haces.",
    ]) {
      expect(hits(verdadero, FALSE_CLAIMS_ES), verdadero).toBe(false);
    }
    for (const verdadero of [
      "Miel does not know which application it is or what it shows, and does not store it.",
      "It does not read what you type or the titles of your windows.",
      "It keeps no history of what you do.",
      "Only that time: it does not know which keys or what you are doing.",
    ]) {
      expect(hits(verdadero, FALSE_CLAIMS_EN), verdadero).toBe(false);
    }
  });

  it("la privacidad dice lo cierto sobre Sueño: solo cuánto tiempo, sin saber teclas ni qué haces, avisos de bloqueo y suspensión y el límite de las entradas sintéticas", () => {
    const es = STRINGS.es.privacy.items.join("\n");
    const en = STRINGS.en.privacy.items.join("\n");
    expect(es).toMatch(/cuánto tiempo lleva sin usarse el teclado o el ratón/);
    expect(es).toMatch(/no sabe qué teclas ni qué haces/);
    expect(es).toMatch(/se bloquea o se desbloquea/);
    expect(es).toMatch(/se suspende o se reanuda/);
    expect(es).toMatch(/entradas simuladas por otros programas/);
    expect(en).toMatch(/how long the keyboard or the mouse have gone unused/);
    expect(en).toMatch(/does not know which keys or what you are doing/);
    expect(en).toMatch(/locked or unlocked/);
    expect(en).toMatch(/suspends or resumes/);
    expect(en).toMatch(/input simulated by other programs/);
  });

  it("fill sustituye marcadores y deja los desconocidos", () => {
    expect(fill("a {x} b {y}", { x: 1 })).toBe("a 1 b {y}");
  });
});
