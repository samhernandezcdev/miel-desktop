# Rendimiento de la animación: reglas para quien crea una mascota / Animation performance rules

**[Español](#español) · [English](#english)**

> Reglas aprendidas midiendo a Miel en reposo (E0b, 2026-11-08). Datos y método: [`plan/v0.2.0-baseline.md`](plan/v0.2.0-baseline.md) §7 y §8, `docs/measurements/v020-e0b-*`, arnés `tools/e0b/`. Todas las cifras son **% de un núcleo de CPU** (Miel + WebView2) en **un equipo con monitor a 165 Hz**; no extrapolar sin medir.

---

# Español

## La regla principal
**El costo en reposo son los fotogramas que el navegador compone por refresco de pantalla, no cuántas animaciones hay.** Mientras *alguna* animación cambie un valor, WebView2 compone y presenta un fotograma por refresco (165 por segundo aquí) en una ventana transparente siempre visible. Cada fotograma cuesta ≈ 0,06 % de un núcleo, ≈ 70 % en el proceso de GPU. Con todo apagado el costo cae a ≈ 0,4 %; quitar una sola animación apenas cambia nada porque las demás mantienen los fotogramas.

## Reglas
1. **Piensa en fotogramas por segundo, no en animaciones.** Mide con todo apagado (el suelo) y con todo activo; si la diferencia es grande, el problema es la tasa de fotogramas.
2. **Promover capas no ayuda.** `will-change` / capas propias: −8 a −12 %, dentro del ruido. Mover flotación y balanceo a la raíz `<svg>` (para que vayan por el compositor) **empeoró un 43–54 %** y sumó ≈ 75 MB de memoria privada. No lo hagas sin medir.
3. **Apaga lo invisible.** Los rostros con `opacity:0` seguían animando (6 parpadeos y 2 «zzz» ocultos) y las animaciones con amplitud 0 (aleteo y flotación en reposo) también. Costaban fotogramas para nada. Regla del generador: `.face:not(.face-<estado>)` sin animación.
4. **Cuantiza con `steps()` ALINEADOS a un reloj común.** `animation-timing-function: steps(N, end)` hace que el valor cambie solo N veces por intervalo, y Chromium solo produce un fotograma cuando algo cambia. Todos los pasos deben caer en la **misma rejilla de 1/fps s**: ajusta cada duración al múltiplo más cercano de 1/fps (p. ej. 30 fps → 0,34 s pasa a 0,3333 s). Sin alinear, el ahorro es mucho menor (idle: 6,7 % desalineado frente a 5,2 % alineado a 30 fps). La función de temporización se aplica **entre keyframes**, no a todo el ciclo.
5. **Elige los fps por estado.** Movimiento lento y de poca amplitud (balanceo ±1,6°, antenas ±3°): 10–20 fps es imperceptible. Movimiento rápido (aleteo, 0,34 s): 30 fps; cada paso mueve la punta del ala menos de ≈ 1 px a 64–128 px de tamaño.
6. **Pausa y oculta ya cuestan el suelo.** `animation-play-state: paused` (Pausada/Oculta) deja la CPU en ≈ 0 %; no hace falta tocarlo. Respeta `prefers-reduced-motion` (animaciones y transiciones fuera).
7. **Guarda la regla en el build.** `tools/check_dist.mjs` falla si el CSS compilado pierde la capa de rendimiento; `src/test/checkDist.test.ts` prueba que falle. El generador (`tools/build_bee.py`) es la fuente: no edites `bee.css` a mano.
8. **Mide siempre igual.** Binario de producción, 5 repeticiones intercaladas, medianas y rangos, estados delimitados por el registro del motor, y anota la frecuencia de refresco.

## Cifras (binario de producción con `custom-protocol`, 165 Hz; media por repetición, mediana entre 5 repeticiones, [rango])
| Estado | Antes | Después (Rest/Sleep 20 fps, Idle 30 fps) | Después (Rest/Sleep 10 fps) |
|---|---|---|---|
| Idle | 10,78 [10,54–10,98] | 5,30 [4,58–6,18] (−51 %) | 5,79 [5,39–6,68] (misma regla de Idle) |
| Rest | 9,86 [9,60–10,19] | 2,98 [2,76–3,69] (−70 %) | 2,49 [2,32–3,07] (−75 %) |
| Paused | ≈ 0,00 | ≈ 0,00 | ≈ 0,00 |
| Hidden | ≈ 0,00 | ≈ 0,00 | ≈ 0,00 |

Alternativas de aleteo en Idle (con Rest a 20 fps): menor amplitud (7° → 4,5°) 5,27 [3,69–5,60]; **ráfagas con pausas** 4,63 [3,97–5,31] — mantener la posición entre ráfagas ahorra poco (≈ −0,7 puntos, rangos solapados) porque el resto de animaciones sigue marcando la rejilla de fotogramas.

## Límites
Un solo equipo (Ryzen 7 5800X, RTX 3060, monitor principal a 165 Hz, plan Equilibrado). El costo debería **escalar con la frecuencia de refresco** (a 60 Hz sería menor): no medido. Las muestras de 1 s tienen resolución de ≈ 1,7 puntos (ticks de 15,6 ms): por eso se usan medias por repetición y las diferencias de ≲ 1 punto no son concluyentes. La medición de producción usó la característica `diag` (solo para el guion de pausa y el registro de estados). Los estados transitorios de reacción (feliz, curiosa, pensativa, sorprendida, hablando) **no** se cuantizaron: siguen costando ≈ 10 %. Los GIF de la comparación visual (Chromium sin cabeza, mismos instantes en todas las columnas) **no se versionan**: se regeneran en `docs/img/anim-compare/` (ignorada por Git) con `tools/anim_compare/`; pasos y comandos en [`tools/anim_compare/README.md`](../tools/anim_compare/README.md) (`python -I tools/build_bee.py --out <dir> --css-only`, `node tools/anim_compare/capture.mjs <config.json> <carpeta>` y `python -I -X utf8 tools/anim_compare/make_gif.py <carpeta> <estado> <salida.gif> <variante>...`).

---

# English

## The main rule
**Idle cost is the number of frames the browser composites per display refresh, not how many animations there are.** As long as *any* animation changes a value, WebView2 composites and presents one frame per refresh (165 per second here) for an always-visible transparent window. Each frame costs ≈ 0.06 % of one core, ≈ 70 % of it in the GPU process. With everything off the cost drops to ≈ 0.4 %; removing a single animation barely changes anything because the others keep the frames coming.

## Rules
1. **Think in frames per second, not in animations.** Measure with everything off (the floor) and everything on; a big gap means the frame rate is the problem.
2. **Promoting layers does not help.** `will-change` / dedicated layers: −8 to −12 %, within noise. Moving float and sway to the root `<svg>` (to use the compositor) made it **43–54 % worse** and added ≈ 75 MB of private memory. Don't do it without measuring.
3. **Switch off what you cannot see.** Faces with `opacity:0` kept animating (6 blinks and 2 hidden "zzz") and so did zero-amplitude animations (rest flap and float). They cost frames for nothing. Generator rule: no animation under `.face:not(.face-<state>)`.
4. **Quantize with `steps()` ALIGNED to a common clock.** `animation-timing-function: steps(N, end)` makes the value change only N times per interval, and Chromium only produces a frame when something changes. All steps must land on the **same 1/fps s grid**: round each duration to the nearest multiple of 1/fps (e.g. 30 fps → 0.34 s becomes 0.3333 s). Without alignment the saving is much smaller (idle: 6.7 % unaligned vs 5.2 % aligned at 30 fps). The timing function applies **between keyframes**, not to the whole cycle.
5. **Pick fps per state.** Slow, small motion (sway ±1.6°, antennae ±3°): 10–20 fps is imperceptible. Fast motion (flap, 0.34 s): 30 fps; each step moves the wing tip by less than ≈ 1 px at 64–128 px size.
6. **Pause and hidden already cost the floor.** `animation-play-state: paused` (Paused/Hidden) keeps CPU at ≈ 0 %; leave it alone. Honor `prefers-reduced-motion` (no animations or transitions).
7. **Guard the rule in the build.** `tools/check_dist.mjs` fails if the compiled CSS loses the performance layer; `src/test/checkDist.test.ts` tests that it fails. The generator (`tools/build_bee.py`) is the source: do not hand-edit `bee.css`.
8. **Always measure the same way.** Production binary, 5 interleaved repetitions, medians and ranges, states delimited by the engine's own log, and note the refresh rate.

## Figures (production binary with `custom-protocol`, 165 Hz; mean per repetition, median across 5 repetitions, [range])
| State | Before | After (Rest/Sleep 20 fps, Idle 30 fps) | After (Rest/Sleep 10 fps) |
|---|---|---|---|
| Idle | 10.78 [10.54–10.98] | 5.30 [4.58–6.18] (−51 %) | 5.79 [5.39–6.68] (same Idle rule) |
| Rest | 9.86 [9.60–10.19] | 2.98 [2.76–3.69] (−70 %) | 2.49 [2.32–3.07] (−75 %) |
| Paused | ≈ 0.00 | ≈ 0.00 | ≈ 0.00 |
| Hidden | ≈ 0.00 | ≈ 0.00 | ≈ 0.00 |

Idle flap alternatives (Rest at 20 fps): lower amplitude (7° → 4.5°) 5.27 [3.69–5.60]; **bursts with pauses** 4.63 [3.97–5.31] — holding the position between bursts saves little (≈ −0.7 points, overlapping ranges) because the other animations still set the frame grid.

## Limits
One machine (Ryzen 7 5800X, RTX 3060, main monitor at 165 Hz, Balanced plan). Cost should **scale with the refresh rate** (lower at 60 Hz): not measured. 1 s samples have ≈ 1.7-point resolution (15.6 ms ticks): hence per-repetition means, and differences of ≲ 1 point are inconclusive. The production measurement used the `diag` feature (only for the pause script and the state log). The transient reaction states (happy, curious, thinking, surprised, talking) were **not** quantized: they still cost ≈ 10 %. The GIFs of the visual comparison (headless Chromium, same instants in every column) are **not versioned**: regenerate them into `docs/img/anim-compare/` (ignored by Git) with `tools/anim_compare/`; steps and commands in [`tools/anim_compare/README.md`](../tools/anim_compare/README.md) (`python -I tools/build_bee.py --out <dir> --css-only`, `node tools/anim_compare/capture.mjs <config.json> <folder>` and `python -I -X utf8 tools/anim_compare/make_gif.py <folder> <state> <out.gif> <variant>...`).
