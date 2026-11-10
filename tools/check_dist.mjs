// Comprobación automática del frontend COMPILADO (dist/). Se ejecuta al final de `npm run build`
// (y por tanto en `npm run tauri build`) y no depende del ojo humano.
//
// Contexto: en producción Tauri añade un nonce a `style-src`; con un nonce presente el navegador IGNORA
// 'unsafe-inline', así que un <style> embebido en el SVG de la abeja queda bloqueado y se pierden
// `.face{opacity:0}`, los selectores [data-state] y los @keyframes (8 rostros apilados, sin animación).
// Las pruebas con jsdom y Edge headless no aplican la CSP final: esta comprobación es la red de seguridad.
//
// Reglas:
//  1. El CSS compilado contiene la regla que oculta los rostros inactivos, los 8 selectores de estado,
//     los @keyframes del rig y prefers-reduced-motion (el CSS viaja como archivo propio, permitido por 'self').
//  2. El JS compilado NO incluye CSS del rig embebido (ni @keyframes ni selectores de rostro): significa
//     que el SVG no lleva un <style> dentro.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const STATES = ["idle", "happy", "curious", "thinking", "sleepy", "surprised", "talking", "resting"];

// Capa de rendimiento (E0b, docs/ANIMATION_PERFORMANCE.md): el costo en reposo lo marca la tasa de fotogramas que
// produce alguna animación activa. Si se pierde alguna de estas reglas, Miel vuelve a gastar ~10 % de un núcleo.
const rule = (state, tail) => new RegExp(`\\[data-state=["']?${state}["']?\\]\\s*${tail}`);
const STEPS = "\\{\\s*animation-timing-function\\s*:\\s*steps\\(";
const NONE = "\\{\\s*animation\\s*:\\s*none";

export function checkPerfLayer(css) {
  const errors = [];
  // 1) los rostros inactivos no animan (los 8 estados)
  for (const s of STATES) {
    if (!rule(s, `\\.face:not\\(\\.face-${s}\\)\\s*\\.eyes`).test(css)) {
      errors.push(`falta la regla que apaga las animaciones de los rostros inactivos en «${s}»`);
    }
  }
  if (!/\.zzz\s*\.z-2\s*\{\s*animation\s*:\s*none/.test(css)) errors.push("la regla de rostros inactivos no apaga las «zzz» ocultas");
  // 2) resting: solo se animan balanceo y antenas, con steps(); flotación y aleteo (amplitud 0) apagados
  if (!rule("resting", `\\.float\\s*${NONE}`).test(css)) errors.push("resting: la flotación (amplitud 0) sigue animada");
  if (!rule("resting", `\\.flap\\s*${NONE}`).test(css)) errors.push("resting: el aleteo (amplitud 0) sigue animado");
  for (const [name, t] of [["balanceo", "\\.sway"], ["antena izquierda", "\\.antenna-l\\s*\\.wiggle"], ["antena derecha", "\\.antenna-r\\s*\\.wiggle"]]) {
    if (!rule("resting", `${t}\\s*${STEPS}`).test(css)) errors.push(`resting: falta steps() en ${name}`);
  }
  // 3) idle y sleepy cuantizados (el aleteo de idle puede ser estándar o en ráfagas)
  const idleFlap = rule("idle", `\\.flap\\s*${STEPS}`).test(css) || rule("idle", "\\.wing-l\\s*\\.flap\\s*\\{\\s*animation\\s*:\\s*bee-flap-burst-l").test(css);
  if (!idleFlap) errors.push("idle: el aleteo no está cuantizado con steps()");
  if (!rule("idle", `\\.sway\\s*${STEPS}`).test(css)) errors.push("idle: falta steps() en el balanceo");
  if (!rule("sleepy", `\\.zzz\\s*\\.z-2\\s*${STEPS}`).test(css)) errors.push("sleepy: faltan los steps() de las «zzz» visibles");
  if (rule("sleepy", `\\.flap\\s*${NONE}`).test(css)) errors.push("sleepy: el aleteo visible no debe apagarse");
  // 4) orden: la capa debe ir DESPUÉS de la tabla base (si no, la base la pisa)
  const baseAt = css.search(/\.bee\s+\.eyes\s*\{[^}]*animation\s*:\s*bee-blink/);
  const perfAt = css.search(rule("resting", "\\.sway\\s*\\{\\s*animation-timing-function"));
  if (baseAt >= 0 && perfAt >= 0 && perfAt < baseAt) errors.push("la capa de rendimiento aparece antes de la tabla base: la base la anularía");
  return errors;
}

export function checkDist(css, js) {
  const errors = [];
  if (!/\.bee\s+\.face\s*\{[^}]*opacity\s*:\s*0/.test(css)) {
    errors.push("el CSS compilado no contiene la regla que oculta los rostros inactivos (`.bee .face{opacity:0…}`)");
  }
  for (const s of STATES) {
    const re = new RegExp(`\\[data-state=["']?${s}["']?\\]\\s*\\.face-${s}`);
    if (!re.test(css)) errors.push(`falta el selector del rostro «${s}» en el CSS compilado`);
  }
  for (const k of ["bee-flap-l", "bee-flap-r", "bee-float", "bee-blink", "bee-talk-a", "bee-zz"]) {
    if (!css.includes(`@keyframes ${k}`)) errors.push(`falta @keyframes ${k} en el CSS compilado`);
  }
  if (!/prefers-reduced-motion/.test(css)) errors.push("falta prefers-reduced-motion en el CSS compilado");
  errors.push(...checkPerfLayer(css));
  // CSS del rig embebido en el JS = <style> dentro del SVG
  for (const needle of ["@keyframes bee-", ".face-idle", "bee-flap-l"]) {
    if (js.includes(needle)) errors.push(`el JS compilado incluye CSS del rig («${needle}»): el SVG lleva un <style> embebido`);
  }
  return errors;
}

// ---- Entradas de build (E3): la mascota (index.html) y la ventana de ajustes (settings.html) ----------------------
// La CSP de producción es `default-src 'self'; script-src 'self'; style-src 'self'; ...`: sin <style> ni estilos en línea,
// sin scripts en línea y sin recursos remotos. `index.html` conserva UN <style> en línea (evita el destello opaco; Tauri le
// añade el nonce); `settings.html` no puede llevar ninguno.
const REMOTE = /^(?:[a-z][a-z0-9+.-]*:)?\/\//i; // http:, https:, ws:, ftp:, ... o //host

/** Referencias de un HTML: scripts (con src), hojas de estilo y otros `href`/`src`. */
export function parseHtml(html) {
  const scripts = [];
  const links = [];
  const inlineScripts = [];
  for (const m of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
    const src = /\ssrc\s*=\s*["']([^"']+)["']/i.exec(m[1])?.[1];
    if (src) scripts.push(src);
    else inlineScripts.push(m[2]);
  }
  for (const m of html.matchAll(/<link\b([^>]*)>/gi)) {
    const href = /\shref\s*=\s*["']([^"']+)["']/i.exec(m[1])?.[1];
    const rel = /\srel\s*=\s*["']([^"']+)["']/i.exec(m[1])?.[1] ?? "";
    if (href) links.push({ href, rel: rel.toLowerCase() });
  }
  const inlineStyles = [...html.matchAll(/<style\b[^>]*>/gi)].length;
  const styleAttrs = [...html.matchAll(/\sstyle\s*=/gi)].length;
  const handlers = [...html.matchAll(/\son[a-z]+\s*=/gi)].length;
  const otherUrls = [...html.matchAll(/\s(?:src|href|action|data|poster|srcset)\s*=\s*["']([^"']+)["']/gi)].map((m) => m[1]);
  return { scripts, links, inlineScripts, inlineStyles, styleAttrs, handlers, otherUrls };
}

const assetName = (url) => (url.startsWith("/assets/") ? url.slice("/assets/".length) : null);

/**
 * Comprueba UNA entrada HTML: recursos locales y existentes, sin script/estilo en línea (salvo los permitidos),
 * sin manejadores `on*=` ni atributos `style=`, y sin URL remotas. Devuelve `{ errors, css, js }` con los archivos
 * enlazados (nombres dentro de `assets/`).
 */
export function checkEntry(name, html, files, { allowInlineStyle }) {
  const errors = [];
  const h = parseHtml(html);
  if (h.scripts.length === 0) errors.push(`${name}: no enlaza ningún script`);
  if (h.inlineScripts.length) errors.push(`${name}: contiene <script> en línea (la CSP lo bloquea)`);
  if (h.inlineStyles && !allowInlineStyle) errors.push(`${name}: contiene <style> en línea (la CSP lo bloquea; usa CSS externo)`);
  if (h.styleAttrs) errors.push(`${name}: contiene atributos style="" (la CSP los bloquea)`);
  if (h.handlers) errors.push(`${name}: contiene manejadores de eventos en línea (onclick=…)`);
  for (const u of h.otherUrls) if (REMOTE.test(u)) errors.push(`${name}: recurso remoto «${u}»`);
  const css = [];
  const js = [];
  const refs = [...h.scripts.map((u) => ({ u, kind: "script" })), ...h.links.map((l) => ({ u: l.href, kind: l.rel }))];
  for (const { u, kind } of refs) {
    if (REMOTE.test(u)) continue; // ya notificado arriba
    const a = assetName(u);
    if (a === null) {
      errors.push(`${name}: «${u}» no es un recurso local de /assets/`);
      continue;
    }
    if (!files.has(a)) {
      errors.push(`${name}: referencia «${u}» pero el archivo no existe en dist/assets`);
      continue;
    }
    if (kind === "stylesheet") css.push(a);
    else if (a.endsWith(".js")) js.push(a);
  }
  return { errors, css, js, scripts: h.scripts.map(assetName).filter(Boolean), stylesheets: css };
}

/** CSS sin recursos remotos (`@import`, `url()`, fuentes). */
export function checkCssLocal(name, css) {
  const errors = [];
  if (/@import\s+(?:url\(\s*)?["']?\s*(?:[a-z][a-z0-9+.-]*:)?\/\//i.test(css)) errors.push(`${name}: @import remoto`);
  if (/url\(\s*["']?\s*(?:[a-z][a-z0-9+.-]*:)?\/\//i.test(css)) errors.push(`${name}: url() remoto`);
  return errors;
}

/** El JS propio de la entrada de ajustes no crea ni inyecta estilos (la CSP lo bloquearía). */
export function checkNoInjectedStyles(name, js) {
  const errors = [];
  for (const [needle, re] of [
    ['createElement("style")', /createElement\(\s*["']style["']\s*\)/],
    ["<style", /<style[\s>]/],
    ["cssText", /\.cssText\b/],
    ['setAttribute("style")', /setAttribute\(\s*["']style["']/],
    ["adoptedStyleSheets", /adoptedStyleSheets/],
    ["insertRule", /insertRule\s*\(/],
  ]) {
    if (re.test(js)) errors.push(`${name}: el JS inyecta estilos (${needle}); la CSP de producción lo bloquea`);
  }
  return errors;
}

/**
 * Comprobación de las DOS entradas. `files` = Map(nombre dentro de assets/ → contenido de texto).
 * Conserva toda la guardia de la mascota (`checkDist`: ocho rostros, @keyframes, capa de rendimiento) sobre el CSS de
 * `index.html` y el JS compilado completo.
 */
export function checkEntries(indexHtml, settingsHtml, files) {
  const errors = [];
  if (indexHtml == null) return ["falta dist/index.html"];
  if (settingsHtml == null) return ["falta dist/settings.html (la entrada de la ventana de ajustes)"];
  const main = checkEntry("index.html", indexHtml, files, { allowInlineStyle: true });
  const sett = checkEntry("settings.html", settingsHtml, files, { allowInlineStyle: false });
  errors.push(...main.errors, ...sett.errors);
  if (main.stylesheets.length === 0) errors.push("index.html: no enlaza ninguna hoja de estilos (el CSS de la mascota debe ser externo)");
  if (sett.stylesheets.length === 0) errors.push("settings.html: no enlaza ninguna hoja de estilos (el CSS de la ventana debe ser externo)");
  for (const c of new Set([...main.css, ...sett.css])) errors.push(...checkCssLocal(c, files.get(c) ?? ""));
  for (const j of sett.scripts) errors.push(...checkNoInjectedStyles(j, files.get(j) ?? ""));
  // guardia de la mascota: CSS enlazado por index.html + TODO el JS compilado (cualquiera podría embeber el rig)
  const mainCss = main.css.map((f) => files.get(f)).join("\n");
  const allJs = [...files].filter(([f]) => f.endsWith(".js")).map(([, v]) => v).join("\n");
  errors.push(...checkDist(mainCss, allJs));
  return errors;
}

function main() {
  const root = join(process.cwd(), "dist");
  const dir = join(root, "assets");
  const read = (p) => (existsSync(p) ? readFileSync(p, "utf8") : null);
  const files = new Map(readdirSync(dir).filter((f) => /\.(css|js)$/.test(f)).map((f) => [f, readFileSync(join(dir, f), "utf8")]));
  const errors = checkEntries(read(join(root, "index.html")), read(join(root, "settings.html")), files);
  if (errors.length) {
    console.error("✗ check:dist — el frontend compilado NO es seguro para la CSP de producción:");
    for (const e of errors) console.error("  - " + e);
    process.exit(1);
  }
  const css = [...files].filter(([f]) => f.endsWith(".css")).map(([, v]) => v).join("\n");
  console.log(`✓ check:dist — 2 entradas (index.html y settings.html) sin scripts en línea ni recursos remotos y settings.html sin <style>; CSS (${css.length} B) con los 8 estados y animaciones; JS sin CSS del rig embebido`);
}

if (import.meta.url === `file://${process.argv[1].replace(/\\/g, "/")}` || process.argv[1]?.endsWith("check_dist.mjs")) main();
