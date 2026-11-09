// Comparación visual antes/después SIN capturar tu pantalla: renderiza el rig sin estilo de Miel con
// Chromium/Edge SIN CABEZA (ventana fuera de pantalla, perfil temporal) y obtiene fotogramas en INSTANTES
// EXACTOS: las animaciones CSS se pausan y se fija su `currentTime` (misma t para todas las variantes).
//
// Uso:  node tools/anim_compare/capture.mjs <config.json> <carpeta-de-salida>
// config.json: {
//   "browser": "C:/.../msedge.exe",
//   "rig": "src/assets/bee/bee-rig.nostyle.svg",
//   "size": 96,
//   "variants": [{"name": "antes", "css": "ruta/bee.css", "label": "Antes"}, ...],
//   "states": [{"name": "idle", "fps": 30, "seconds": 4}, ...]
// }
// Salida: <out>/<estado>/<variante>/NNNN.png (+ <out>/<estado>/times.json con los instantes usados).
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [cfgPath, outDir] = process.argv.slice(2);
if (!cfgPath || !outDir) {
  console.error("uso: node tools/anim_compare/capture.mjs <config.json> <salida>");
  process.exit(2);
}
const cfg = JSON.parse(readFileSync(cfgPath, "utf8"));
const PORT = 9333;
const W = 132, H = 140; // píxeles CSS del lienzo por variante
const profile = mkdtempSync(join(tmpdir(), "miel-compare-"));
const browser = spawn(cfg.browser, [
  "--headless=new", `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`,
  "--no-first-run", "--no-default-browser-check", "--hide-scrollbars", "--allow-file-access-from-files",
  "--disable-background-timer-throttling", "--force-color-profile=srgb", "about:blank",
], { stdio: "ignore" });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function waitJson(path) {
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}${path}`);
      if (r.ok) return await r.json();
    } catch { /* aún no */ }
    await sleep(200);
  }
  throw new Error("el navegador sin cabeza no respondió");
}

let ws, nextId = 1;
const pending = new Map();
function send(method, params = {}) {
  const id = nextId++;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((res, rej) => pending.set(id, { res, rej }));
}
async function evalJs(expr) {
  const r = await send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 300));
  return r.result.value;
}

function pageHtml(variant, rigSvg) {
  const cssUrl = pathToFileURL(resolve(variant.css)).href;
  const rig = rigSvg.replace(/ width="128" height="128"/, ` width="${cfg.size}" height="${cfg.size}"`);
  return `<!doctype html><meta charset="utf-8"><link rel="stylesheet" href="${cssUrl}">
<style>html,body{margin:0;background:#f4f1ea;font:12px/1.2 "Segoe UI",Arial,sans-serif;color:#3b2a21}
.label{position:absolute;left:0;right:0;top:6px;text-align:center;font-weight:600}
.sprite{position:absolute;left:${(W - cfg.size) / 2}px;top:34px;width:${cfg.size}px;height:${cfg.size}px}</style>
<div class="label">${variant.label}</div><div class="sprite">${rig}</div>`;
}

async function main() {
  const rigSvg = readFileSync(cfg.rig, "utf8");
  await waitJson("/json/version");
  const pages = await waitJson("/json");
  const page = pages.find((p) => p.type === "page");
  ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      const { res, rej } = pending.get(m.id);
      pending.delete(m.id);
      m.error ? rej(new Error(m.error.message)) : res(m.result);
    }
  };
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 2, mobile: false });

  const tmp = join(profile, "pages");
  mkdirSync(tmp, { recursive: true });
  for (const st of cfg.states) {
    const n = Math.round(st.fps * st.seconds);
    // Instantes en el CENTRO de cada intervalo de 1/fps s (nunca justo en el borde de un paso de steps(): ahí el redondeo
    // de coma flotante de Chromium puede caer a un lado u otro). Idénticos para todas las variantes.
    const times = Array.from({ length: n }, (_, i) => Math.round(((i + 0.5) * 1000) / st.fps * 1000) / 1000);
    mkdirSync(join(outDir, st.name), { recursive: true });
    writeFileSync(join(outDir, st.name, "times.json"), JSON.stringify({ fps: st.fps, times_ms: times }));
    for (const v of cfg.variants) {
      const file = join(tmp, `${v.name}.html`);
      writeFileSync(file, pageHtml(v, rigSvg));
      await send("Page.navigate", { url: pathToFileURL(file).href });
      for (let i = 0; i < 50; i++) { if (await evalJs("document.readyState") === "complete") break; await sleep(100); }
      await evalJs(`(async () => {
        const b = document.querySelector('.bee'); b.setAttribute('data-state', ${JSON.stringify(st.name)});
        await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
        document.getAnimations().filter(a => a instanceof CSSTransition).forEach(a => a.finish());
        await new Promise(r => requestAnimationFrame(r));
        return document.getAnimations().length;
      })()`);
      const dir = join(outDir, st.name, v.name);
      mkdirSync(dir, { recursive: true });
      for (let i = 0; i < n; i++) {
        await evalJs(`document.getAnimations().filter(a => a instanceof CSSAnimation).forEach(a => { a.pause(); a.currentTime = ${times[i]}; }); 1`);
        const shot = await send("Page.captureScreenshot", { format: "png", clip: { x: 0, y: 0, width: W, height: H, scale: 1 } });
        writeFileSync(join(dir, String(i).padStart(4, "0") + ".png"), Buffer.from(shot.data, "base64"));
      }
      console.log(`${st.name} / ${v.name}: ${n} fotogramas`);
    }
  }
}

main()
  .catch((e) => { console.error("ERROR:", e.message); process.exitCode = 1; })
  .finally(() => { try { ws?.close(); } catch { /* */ } browser.kill(); setTimeout(() => { try { rmSync(profile, { recursive: true, force: true }); } catch { /* */ } process.exit(process.exitCode ?? 0); }, 1500); });
