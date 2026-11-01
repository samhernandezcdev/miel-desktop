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
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const STATES = ["idle", "happy", "curious", "thinking", "sleepy", "surprised", "talking", "resting"];

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
  // CSS del rig embebido en el JS = <style> dentro del SVG
  for (const needle of ["@keyframes bee-", ".face-idle", "bee-flap-l"]) {
    if (js.includes(needle)) errors.push(`el JS compilado incluye CSS del rig («${needle}»): el SVG lleva un <style> embebido`);
  }
  return errors;
}

function main() {
  const dir = join(process.cwd(), "dist", "assets");
  const files = readdirSync(dir);
  const css = files.filter((f) => f.endsWith(".css")).map((f) => readFileSync(join(dir, f), "utf8")).join("\n");
  const js = files.filter((f) => f.endsWith(".js")).map((f) => readFileSync(join(dir, f), "utf8")).join("\n");
  const errors = checkDist(css, js);
  if (errors.length) {
    console.error("✗ check:dist — el frontend compilado NO es seguro para la CSP de producción:");
    for (const e of errors) console.error("  - " + e);
    process.exit(1);
  }
  console.log(`✓ check:dist — CSS (${css.length} B) con los 8 estados y animaciones; JS sin CSS del rig embebido`);
}

if (import.meta.url === `file://${process.argv[1].replace(/\\/g, "/")}` || process.argv[1]?.endsWith("check_dist.mjs")) main();
