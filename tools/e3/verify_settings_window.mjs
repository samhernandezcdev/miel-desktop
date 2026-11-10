// E3 · Verificación con la APLICACIÓN REAL (binario de producción con `diag`): ventana de ajustes de verdad,
// WebView2 de verdad. SOLO diagnóstico; no forma parte del producto.
//
// Uso:  node tools/e3/verify_settings_window.mjs <miel.exe de diag> <salida.json>
//
// Cómo funciona (sin tocar el cursor ni el teclado físicos):
//  - Lanza miel.exe con `MIEL_DIAG_UI_SCRIPT` (abre/cierra la ventana y llama a `set_setting` desde Rust a horas fijas)
//    y con el puerto de depuración de WebView2 en 127.0.0.1 (solo local) para leer el DOM de la página y enviarle
//    eventos de teclado SINTÉTICOS dentro de la página (CDP `Input.dispatchKeyEvent`: no pasan por el teclado del sistema).
//  - `settings://changed` se verifica así: Rust cambia un ajuste y se comprueba que el valor del control CAMBIÓ sin que la
//    página haya llamado a `get_settings` ni a `set_setting` (se cuenta cada `invoke` de la página).
//  - Respalda y restaura `settings.json` (y elimina la carpeta si la creó esta ejecución).
//  - Mide procesos (miel.exe + árbol WebView2) y memoria privada antes de abrir, con la ventana abierta y tras cerrar.
import { execFileSync, spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [exe, outFile] = process.argv.slice(2);
if (!exe || !outFile) {
  console.error("uso: node verify_settings_window.mjs <miel.exe> <salida.json>");
  process.exit(2);
}
const HERE = dirname(fileURLToPath(import.meta.url));
const PORT = 9333;
const APPDATA = process.env.APPDATA;
const settingsDir = join(APPDATA, "lat.bkode.miel");
const settingsFile = join(settingsDir, "settings.json");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const report = { exe, checks: [], stats: {}, notes: [] };
const check = (name, ok, detail = "") => {
  report.checks.push({ name, ok: !!ok, detail });
  console.log(`${ok ? "OK  " : "FALLO"} ${name}${detail ? " — " + detail : ""}`);
};

// ---- utilidades -------------------------------------------------------------------------------------------------
async function getJson(url) {
  const r = await fetch(url);
  return r.json();
}
async function waitFor(fn, ms, step = 250) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) return null;
    await sleep(step);
  }
}
const targets = async () => {
  try {
    return await getJson(`http://127.0.0.1:${PORT}/json`);
  } catch {
    return [];
  }
};
const settingsTarget = async () => (await targets()).find((t) => t.type === "page" && /settings\.html/.test(t.url));

function ps(script) {
  return execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", script], { encoding: "utf8" }).trim();
}
function procStats(rootPid) {
  const out = ps(`
$all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
$set = [System.Collections.Generic.HashSet[int]]::new(); [void]$set.Add(${rootPid})
do { $n = $set.Count; foreach ($p in $all) { if ($set.Contains([int]$p.ParentProcessId)) { [void]$set.Add([int]$p.ProcessId) } } } while ($set.Count -ne $n)
$priv = 0.0; $ws = 0.0; $c = 0
foreach ($id in $set) { try { $q = Get-Process -Id $id -ErrorAction Stop; $priv += $q.PrivateMemorySize64; $ws += $q.WorkingSet64; $c++ } catch {} }
'{0} {1} {2}' -f $c, [math]::Round($priv/1MB,1), [math]::Round($ws/1MB,1)`);
  const [procs, privateMb, workingSetMb] = out.split(/\s+/).map(Number);
  return { procs, privateMb, workingSetMb };
}
const winAttrs = (pid) => {
  // -ExecutionPolicy Bypass solo para ESTE proceso (no cambia la configuración del sistema)
  const raw = execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", join(HERE, "winattrs.ps1"), "-RootPid", String(pid)], { encoding: "utf8" }).trim();
  if (!raw) return [];
  const v = JSON.parse(raw);
  return Array.isArray(v) ? v : [v];
};

function cdp(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const pending = new Map();
  const events = [];
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) {
      const { res, rej } = pending.get(m.id);
      pending.delete(m.id);
      m.error ? rej(new Error(JSON.stringify(m.error))) : res(m.result);
    } else events.push(m);
  };
  const opened = new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = rej;
  });
  const send = (method, params = {}) =>
    new Promise((res, rej) => {
      const i = ++id;
      pending.set(i, { res, rej });
      ws.send(JSON.stringify({ id: i, method, params }));
    });
  const ev = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails.exception?.description ?? r.exceptionDetails));
    return r.result.value;
  };
  const key = async (k, extra = {}) => {
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: k.key, code: k.code, windowsVirtualKeyCode: k.vk, text: k.text, ...extra });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: k.key, code: k.code, windowsVirtualKeyCode: k.vk, ...extra });
  };
  return { opened, send, ev, key, events, close: () => ws.close() };
}
const K = {
  Tab: { key: "Tab", code: "Tab", vk: 9 },
  Enter: { key: "Enter", code: "Enter", vk: 13, text: "\r" },
  Space: { key: " ", code: "Space", vk: 32, text: " " },
  Right: { key: "ArrowRight", code: "ArrowRight", vk: 39 },
  Backspace: { key: "Backspace", code: "Backspace", vk: 8 },
};

// ---- preparación ------------------------------------------------------------------------------------------------
if (ps("(Get-Process miel -ErrorAction SilentlyContinue | Measure-Object).Count") !== "0") {
  console.error("Hay un miel.exe en ejecución; ciérralo (no se toca).");
  process.exit(2);
}
const existed = existsSync(settingsFile);
const dirExisted = existsSync(settingsDir);
const bak = join(process.env.TEMP, "miel-settings-e3-verify.bak");
if (existed) copyFileSync(settingsFile, bak);
const logFile = outFile.replace(/\.json$/, "") + "-engine.log";
mkdirSync(dirname(resolve(outFile)), { recursive: true });
const logFd = openSync(logFile, "w");

const UI_SCRIPT = "open_settings@6,set:message_interval_minutes=33@14,set:pet_size=64@16,open_settings@20,close_settings@75,open_settings@95,close_settings@115";
const child = spawn(exe, [], {
  stdio: ["ignore", logFd, logFd],
  env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${PORT}`, MIEL_DIAG_UI_SCRIPT: UI_SCRIPT },
});
const t0 = Date.now();
const rel = () => ((Date.now() - t0) / 1000).toFixed(1);
let page;
try {
  // 1) línea base: la mascota sola
  await waitFor(async () => (await targets()).some((t) => /index\.html|tauri\.localhost\/?$|localhost\/?$/.test(t.url) || t.type === "page"), 20000);
  await sleep(1500);
  report.stats.baseline = procStats(child.pid);
  const main0 = winAttrs(child.pid).filter((w) => w.title === "Miel");
  report.stats.mainWindow = main0;
  check("línea base: solo la ventana de la mascota (sin ventana de ajustes)", (await settingsTarget()) === undefined && main0.length >= 1, JSON.stringify(report.stats.baseline));
  check("la mascota sigue click-through, sin foco y siempre encima", main0.some((w) => w.topmost && w.transparent && w.noactivate), JSON.stringify(main0));

  // 2) la ventana se abre desde Rust
  const tgt = await waitFor(settingsTarget, 25000);
  check("la ventana de ajustes se abre (etiqueta settings, desde Rust)", !!tgt, `a los ${rel()} s`);
  if (!tgt) throw new Error("sin ventana de ajustes");
  await sleep(2500);
  page = cdp(tgt.webSocketDebuggerUrl);
  await page.opened;
  for (const d of ["Runtime", "Log", "Page", "Network"]) await page.send(`${d}.enable`);
  await page.send("Emulation.setFocusEmulationEnabled", { enabled: true });
  await page.send("Page.reload"); // para capturar CSP y peticiones desde la carga
  await sleep(2500);
  // cuenta de `invoke` de la página: demuestra que un cambio llega por el evento y no por una lectura
  await page.ev(`(() => { window.__calls = []; const o = window.__TAURI_INTERNALS__.invoke; window.__TAURI_INTERNALS__.invoke = (c, a, op) => { window.__calls.push(c); return o.call(window.__TAURI_INTERNALS__, c, a, op); }; return true; })()`);

  const w1 = winAttrs(child.pid).filter((w) => /Ajustes/.test(w.title));
  report.stats.settingsWindow = w1;
  check("ventana de ajustes: con marco, redimensionable, en la barra de tareas, SIN siempre-encima, activable", w1.length === 1 && w1[0].caption && w1[0].thickframe && !w1[0].topmost && !w1[0].toolwindow && !w1[0].noactivate && !w1[0].transparent, JSON.stringify(w1));
  report.stats.open = procStats(child.pid);

  // 3) página: CSP, recursos, DOM
  const dom = await page.ev(`(() => ({
    title: document.title, lang: document.documentElement.lang, langs: navigator.languages,
    h1: document.querySelector('h1')?.textContent,
    styleEls: document.querySelectorAll('style').length, sheets: document.styleSheets.length,
    inlineStyleAttrs: document.querySelectorAll('[style]').length,
    bg: getComputedStyle(document.body).backgroundColor, font: getComputedStyle(document.body).fontFamily,
    controls: [...document.querySelectorAll('input,button,select,textarea')].map(e => e.type + ':' + (e.labels?.[0]?.textContent ?? '')),
    radios: [...document.querySelectorAll('input[type=radio]')].map(e => e.value + (e.checked ? '*' : '')),
    interval: document.querySelector('input[type=number][id$=interval]')?.value,
    speed: document.querySelector('input[type=number][id$=speed]')?.value,
  }))()`);
  report.stats.dom = dom;
  check("la página carga con CSS externo (hoja de estilos aplicada, sin <style> ni style=\"\")", dom.sheets >= 1 && dom.styleEls === 0 && dom.inlineStyleAttrs === 0 && dom.bg !== "rgba(0, 0, 0, 0)", `fondo ${dom.bg}`);
  check("7 controles nativos (3 radios, 2 interruptores, 2 numéricos)", dom.controls.length === 7, dom.controls.join(" | "));
  const csp = page.events.filter((e) => e.method === "Log.entryAdded" && /Content Security Policy|CSP|Refused/i.test(JSON.stringify(e.params.entry)));
  const consoleErr = page.events.filter((e) => (e.method === "Log.entryAdded" && e.params.entry.level === "error") || e.method === "Runtime.exceptionThrown");
  check("cero violaciones de CSP en la carga", csp.length === 0, csp.map((e) => e.params.entry.text).join(" / "));
  check("sin errores de consola ni excepciones", consoleErr.length === 0, consoleErr.map((e) => JSON.stringify(e.params).slice(0, 160)).join(" / "));
  const urls = page.events.filter((e) => e.method === "Network.requestWillBeSent").map((e) => e.params.request.url);
  const remote = urls.filter((u) => !/^(https?:\/\/(tauri|ipc)\.localhost|tauri:\/\/localhost|ipc:|data:|about:)/.test(u));
  report.stats.requests = urls;
  check("sin recursos remotos: todas las peticiones de la página son locales", remote.length === 0 && urls.length > 0, remote.join(" ") || `${urls.length} locales`);

  // 4) settings://changed llega a la ventana real (Rust cambia el ajuste; la página no llama a nada)
  const beforeCalls = await page.ev("window.__calls.length");
  const got33 = await waitFor(async () => (await page.ev(`document.querySelector('input[type=number][id$=interval]').value`)) === "33", 30000, 300);
  const calls = await page.ev("window.__calls");
  check("settings://changed llega a la ventana REAL: el intervalo pasó a 33 por un cambio hecho desde Rust", !!got33, `a los ${rel()} s; antes ${dom.interval}`);
  check("…sin que la página llamara a get_settings ni a set_setting", !calls.some((c) => /get_settings|set_setting/.test(c)), JSON.stringify(calls.slice(beforeCalls)));
  const got64 = await waitFor(async () => (await page.ev(`document.querySelector('input[type=radio][value="64"]').checked`)) === true, 15000, 300);
  check("segundo evento: el tamaño pasó a 64 (radio)", !!got64);

  // 5) instancia única: la segunda petición enfoca
  await waitFor(() => /ajustes Focused/.test(readFileSync(logFile, "utf8")), 20000, 500);
  const log1 = readFileSync(logFile, "utf8");
  check("instancia única: la segunda petición solo enfoca (Focused) y hay una sola ventana", /ajustes Focused/.test(log1) && (await targets()).filter((t) => /settings\.html/.test(t.url)).length === 1, "");

  // 6) teclado (eventos sintéticos DENTRO de la página)
  await page.ev(`document.activeElement.blur(); document.body.focus(); true`);
  const order = [];
  for (let i = 0; i < 6; i++) {
    await page.key(K.Tab);
    order.push(await page.ev(`(() => { const e = document.activeElement; return e === document.body ? 'body' : (e.type + ':' + (e.labels?.[0]?.textContent ?? e.name ?? '') + ':' + (e.matches(':focus-visible') ? 'vis' : 'novis') + ':' + (getComputedStyle(e).outlineStyle !== 'none' && parseFloat(getComputedStyle(e).outlineWidth) > 0 ? 'outline' : 'sinoutline')); })()`));
  }
  report.stats.tabOrder = order;
  const stops = order.slice(0, 5);
  check("Tab recorre los 5 puntos de parada en orden (tamaño, mov. reducido, velocidad, mensajes, intervalo)", /radio/.test(stops[0]) && /checkbox:Movimiento reducido|checkbox:Reduced motion/.test(stops[1]) && /number:(Velocidad|Speed)/.test(stops[2]) && /checkbox:(Mostrar mensajes|Show messages)/.test(stops[3]) && /number:(Tiempo entre mensajes|Time between)/.test(stops[4]), stops.join(" → "));
  check("el foco es visible con teclado en cada parada (:focus-visible + contorno)", stops.every((s) => /:vis:outline$/.test(s)), stops.join(" → "));

  // radio con flechas: cambia el tamaño y Rust lo acepta
  await page.ev(`document.querySelector('input[type=radio][value="64"]').focus(); true`);
  await page.key(K.Right); // 64 → 96
  const sz = await waitFor(async () => (await page.ev(`window.__TAURI_INTERNALS__.invoke('get_settings')`)).pet_size === 96, 8000, 250);
  check("teclado: flecha derecha en el grupo de tamaño → Rust aplica 96", !!sz);

  // espacio sobre un interruptor
  await page.ev(`document.querySelector('input[type=checkbox][id$=motion]').focus(); true`);
  await page.key(K.Space);
  const rm = await waitFor(async () => (await page.ev(`window.__TAURI_INTERNALS__.invoke('get_settings')`)).reduced_motion === true, 8000, 250);
  check("teclado: Espacio en «Movimiento reducido» → Rust lo aplica (true)", !!rm);
  await page.key(K.Space); // se deja como estaba
  await waitFor(async () => (await page.ev(`window.__TAURI_INTERNALS__.invoke('get_settings')`)).reduced_motion === false, 8000, 250);

  // numéricos: escribir + Enter; rechazo; ajuste de rango
  const type = async (sel, text) => {
    await page.ev(`(() => { const e = document.querySelector('${sel}'); e.focus(); e.select(); return true; })()`);
    if (text === "") await page.key(K.Backspace);
    else await page.send("Input.insertText", { text });
  };
  const speedSel = "input[type=number][id$=speed]";
  await type(speedSel, "300");
  await page.key(K.Enter);
  const sp = await waitFor(async () => (await page.ev(`window.__TAURI_INTERNALS__.invoke('get_settings')`)).speed === 300, 8000, 250);
  check("teclado: escribir 300 + Enter en Velocidad → Rust aplica 300", !!sp);
  await type(speedSel, "");
  await page.key(K.Enter);
  const rej = await waitFor(async () => (await page.ev(`document.querySelector('.notice')?.textContent`)) || null, 8000, 200);
  const restored = await page.ev(`document.querySelector('${speedSel}').value`);
  check("valor rechazado por Rust: aviso breve visible y el control vuelve al valor real (300)", !!rej && restored === "300", `${rej} / ${restored}`);
  await type(speedSel, "99999");
  await page.key(K.Enter);
  const clamp = await waitFor(async () => (await page.ev(`document.querySelector('${speedSel}').value`)) === "600", 8000, 200);
  const adj = await page.ev(`document.querySelector('.notice')?.textContent`);
  check("fuera de rango: Rust ajusta a 600 y la ventana lo muestra con aviso", !!clamp && /600/.test(adj ?? ""), adj ?? "");
  await type(speedSel, "260");
  await page.key(K.Enter);
  await waitFor(async () => (await page.ev(`window.__TAURI_INTERNALS__.invoke('get_settings')`)).speed === 260, 8000, 250);

  // 7) medios: reduced motion, forced colors, tema oscuro
  await page.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
  const light = await page.ev(`getComputedStyle(document.body).backgroundColor`);
  await page.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "dark" }] });
  const dark = await page.ev(`getComputedStyle(document.body).backgroundColor`);
  check("tema claro/oscuro: el fondo cambia con prefers-color-scheme", light !== dark, `${light} / ${dark}`);
  await page.send("Emulation.setEmulatedMedia", { features: [{ name: "forced-colors", value: "active" }, { name: "prefers-reduced-motion", value: "reduce" }] });
  const fc = await page.ev(`({ forced: matchMedia('(forced-colors: active)').matches, reduce: matchMedia('(prefers-reduced-motion: reduce)').matches, border: getComputedStyle(document.querySelector('.banner')).borderTopStyle, anim: getComputedStyle(document.body).animationName })`);
  check("alto contraste y movimiento reducido: la página responde (bordes conservados, sin animaciones)", fc.forced && fc.reduce && fc.border === "solid" && fc.anim === "none", JSON.stringify(fc));
  await page.send("Emulation.setEmulatedMedia", { features: [] });

  // 8) escala / tamaño de texto sin desbordes
  const overflow = [];
  for (const [w, h] of [[360, 420], [520, 680], [900, 700]]) {
    for (const dsf of [1, 1.25, 1.5, 2]) {
      for (const fs of [100, 125, 150, 200]) {
        await page.send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: dsf, mobile: false });
        await page.ev(`document.documentElement.style.fontSize = '${fs}%'; true`);
        const o = await page.ev(`({ sw: document.documentElement.scrollWidth, cw: document.documentElement.clientWidth })`);
        if (o.sw > o.cw) overflow.push(`${w}px @${dsf}x texto ${fs}% → ${o.sw}>${o.cw}`);
      }
    }
  }
  await page.ev(`document.documentElement.style.fontSize = ''; true`);
  await page.send("Emulation.clearDeviceMetricsOverride");
  check("sin desborde horizontal en 360/520/900 px × escala 100/125/150/200 % × texto 100–200 % (emulado)", overflow.length === 0, overflow.slice(0, 3).join("; "));

  // 9) idioma inglés
  await page.send("Emulation.setUserAgentOverride", { userAgent: await page.ev("navigator.userAgent"), acceptLanguage: "en-US,en" });
  await page.send("Page.reload");
  await sleep(2500);
  const en = await page.ev(`({ h1: document.querySelector('h1')?.textContent, lang: document.documentElement.lang, langs: navigator.languages })`);
  check("idioma: con navegador en inglés la ventana sale en inglés (lang=en)", en.lang === "en" && /Miel settings/.test(en.h1 ?? ""), JSON.stringify(en));
  await page.send("Emulation.setUserAgentOverride", { userAgent: await page.ev("navigator.userAgent"), acceptLanguage: dom.langs.join(",") });
  report.notes.push(`idioma de la persona en esta máquina: ${JSON.stringify(dom.langs)}; ventana en «${dom.lang}»`);
  page.close();
  page = null;

  // 10) se destruye al cerrar (no se oculta) y se libera
  await waitFor(async () => (await settingsTarget()) === undefined, 60000, 500);
  check("al cerrarla la página desaparece (la ventana se destruye)", (await settingsTarget()) === undefined, `a los ${rel()} s`);
  await sleep(1000);
  const leftover = winAttrs(child.pid).filter((w) => /Ajustes/.test(w.title));
  check("tras cerrar no queda ninguna ventana de ajustes (ni oculta)", leftover.length === 0, JSON.stringify(leftover));
  await sleep(1500);
  report.stats.afterClose3s = procStats(child.pid);
  await sleep(3000);
  report.stats.afterClose7s = procStats(child.pid);

  // 11) reapertura con la misma etiqueta
  const tgt2 = await waitFor(settingsTarget, 25000);
  check("reabrir tras cerrar funciona (etiqueta liberada)", !!tgt2, `a los ${rel()} s`);
  if (tgt2) {
    await sleep(2500);
    const p2 = cdp(tgt2.webSocketDebuggerUrl);
    await p2.opened;
    const c2 = await p2.ev(`document.querySelectorAll('input').length`);
    check("la ventana reabierta muestra sus controles", c2 === 7, String(c2));
    p2.close();
    report.stats.reopened = procStats(child.pid);
  }
  await waitFor(async () => (await settingsTarget()) === undefined, 40000, 500);
  await sleep(7000);
  report.stats.afterSecondClose = procStats(child.pid);
  const log = readFileSync(logFile, "utf8");
  const created = (log.match(/ajustes Created/g) ?? []).length;
  check("Rust registró 2 creaciones y 1 enfoque (abrir, enfocar, cerrar, reabrir)", created === 2 && /ajustes Focused/.test(log), `Created×${created}`);
} catch (e) {
  check("ejecución sin excepciones", false, String(e?.stack ?? e));
} finally {
  try {
    page?.close();
  } catch {}
  try {
    ps(`Get-Process miel -ErrorAction SilentlyContinue | Stop-Process -Force; Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -like '*remote-debugging-port=${PORT}*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }`);
  } catch {}
  await sleep(1500);
  if (existed) copyFileSync(bak, settingsFile);
  else {
    rmSync(settingsFile, { force: true });
    rmSync(join(settingsDir, "settings.v1.bak.json"), { force: true });
    if (!dirExisted) rmSync(settingsDir, { recursive: true, force: true });
  }
  report.settingsRestored = existed ? "restaurado" : `ausente como al inicio: ${!existsSync(settingsFile)}`;
  report.leftover = ps("(Get-Process miel -ErrorAction SilentlyContinue | Measure-Object).Count");
  writeFileSync(outFile, JSON.stringify(report, null, 2));
  const bad = report.checks.filter((c) => !c.ok);
  console.log(`\n${report.checks.length - bad.length}/${report.checks.length} comprobaciones OK; settings.json: ${report.settingsRestored}; miel restantes: ${report.leftover}`);
  process.exit(bad.length ? 1 : 0);
}
