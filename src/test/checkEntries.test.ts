import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
// @ts-expect-error tools/check_dist.mjs es JavaScript sin declaraciones de tipos
import { checkCssLocal, checkEntries, checkNoInjectedStyles, parseHtml } from "../../tools/check_dist.mjs";

// CSS REAL de la mascota (la misma fuente que usa checkDist.test.ts) para la guardia de los ocho rostros.
const rigCss = (): string => readFileSync(resolve(process.cwd(), "src", "assets", "bee", "bee.css"), "utf8").replace(/\r\n/g, "\n");

const INDEX = `<!doctype html><html><head><title>Miel</title><style>html,body{margin:0}</style>
<script type="module" crossorigin src="/assets/main-1.js"></script>
<link rel="modulepreload" crossorigin href="/assets/shared-1.js">
<link rel="stylesheet" crossorigin href="/assets/main-1.css"></head><body><div id="root"></div></body></html>`;
const SETTINGS = `<!doctype html><html><head><title>Ajustes</title>
<script type="module" crossorigin src="/assets/settings-1.js"></script>
<link rel="modulepreload" crossorigin href="/assets/shared-1.js">
<link rel="stylesheet" crossorigin href="/assets/settings-1.css"></head><body><div id="root"></div></body></html>`;

const files = (): Map<string, string> =>
  new Map([
    ["main-1.js", "console.log(1)"],
    ["shared-1.js", "export const x=1"],
    ["settings-1.js", "export const y=2"],
    ["main-1.css", rigCss()],
    ["settings-1.css", ".page{margin:0}"],
  ]);

describe("check:dist con las dos entradas (E3)", () => {
  it("una compilación correcta pasa", () => {
    expect(checkEntries(INDEX, SETTINGS, files())).toEqual([]);
  });

  it("falla si falta cualquiera de los dos HTML", () => {
    expect(checkEntries(null, SETTINGS, files())[0]).toMatch(/index\.html/);
    expect(checkEntries(INDEX, null, files())[0]).toMatch(/settings\.html/);
  });

  it("settings.html no admite <style> en línea, pero index.html conserva el suyo", () => {
    expect(checkEntries(INDEX, SETTINGS.replace("</head>", "<style>a{b:c}</style></head>"), files()).join()).toMatch(/settings\.html: contiene <style>/);
    expect(checkEntries(INDEX, SETTINGS, files())).toEqual([]); // index.html con <style> en línea: permitido
  });

  it.each([
    ["script en línea", (h: string) => h.replace("</body>", "<script>alert(1)</script></body>"), /<script> en línea/],
    ["atributo style", (h: string) => h.replace('<div id="root">', '<div style="color:red" id="root">'), /atributos style/],
    ["manejador onclick", (h: string) => h.replace('<div id="root">', '<div onclick="x()" id="root">'), /manejadores/],
    ["script remoto", (h: string) => h.replace("/assets/settings-1.js", "https://cdn.example.com/a.js"), /remoto/],
    ["hoja de estilos remota", (h: string) => h.replace("/assets/settings-1.css", "//fonts.example.com/a.css"), /remoto/],
    ["recurso fuera de /assets/", (h: string) => h.replace("/assets/settings-1.css", "/otro/a.css"), /no es un recurso local/],
    ["archivo enlazado inexistente", (h: string) => h.replace("settings-1.css", "no-existe.css"), /no existe/],
  ])("settings.html: %s → falla", (_n, mutate, re) => {
    const errors: string[] = checkEntries(INDEX, mutate(SETTINGS), files());
    expect(errors.join("\n")).toMatch(re as RegExp);
  });

  it("las mismas reglas valen para index.html", () => {
    const errors: string[] = checkEntries(INDEX.replace("</body>", "<script>x()</script></body>"), SETTINGS, files());
    expect(errors.join()).toMatch(/index\.html: contiene <script> en línea/);
  });

  it("settings.html sin hoja de estilos o sin script falla", () => {
    expect(checkEntries(INDEX, SETTINGS.replace(/<link rel="stylesheet"[^>]*>/, ""), files()).join()).toMatch(/settings\.html: no enlaza ninguna hoja/);
    expect(checkEntries(INDEX, SETTINGS.replace(/<script[^>]*><\/script>/, ""), files()).join()).toMatch(/settings\.html: no enlaza ningún script/);
  });

  it("CSS con recursos remotos falla", () => {
    for (const bad of ['@import "https://x.example/a.css";', "@import url(//x.example/a.css);", "a{background:url(https://x.example/i.png)}", '@font-face{src:url("http://x.example/f.woff2")}']) {
      const f = files();
      f.set("settings-1.css", bad);
      expect(checkEntries(INDEX, SETTINGS, f).join()).toMatch(/remoto/);
    }
    expect(checkCssLocal("a.css", 'a{background:url("data:image/png;base64,AAAA")} @import "local.css";')).toEqual([]);
  });

  it("el JS de la entrada de ajustes no puede inyectar estilos", () => {
    for (const bad of ['document.createElement("style")', "x.style.cssText='a:b'", 'el.setAttribute("style","a:b")', "document.adoptedStyleSheets=[]", "sheet.insertRule('a{}')", "<style>a{}</style>"]) {
      const f = files();
      f.set("settings-1.js", bad);
      expect(checkEntries(INDEX, SETTINGS, f).join(), bad).toMatch(/inyecta estilos/);
    }
    expect(checkNoInjectedStyles("x.js", "const a = 1")).toEqual([]);
  });

  it("conserva la guardia de la mascota: sin los ocho rostros o con el rig embebido en el JS falla", () => {
    const noFaces = files();
    noFaces.set("main-1.css", ".bee{}");
    expect(checkEntries(INDEX, SETTINGS, noFaces).join()).toMatch(/rostros inactivos/);
    const embedded = files();
    embedded.set("shared-1.js", "const css='@keyframes bee-x{}'");
    expect(checkEntries(INDEX, SETTINGS, embedded).join()).toMatch(/JS compilado incluye CSS del rig/);
  });

  it("parseHtml distingue scripts con src de los en línea", () => {
    const p = parseHtml('<script src="/assets/a.js"></script><script>1</script><link rel="stylesheet" href="/assets/a.css">');
    expect(p.scripts).toEqual(["/assets/a.js"]);
    expect(p.inlineScripts).toHaveLength(1);
    expect(p.links).toEqual([{ href: "/assets/a.css", rel: "stylesheet" }]);
  });
});
