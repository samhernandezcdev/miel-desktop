# Miel — guía del personaje

Mascota de escritorio original: una abeja 2D en SVG, pensada para Electron + React + TypeScript.
"Miel" es un nombre de trabajo; cámbialo si prefieres otro.

> Esta fase cubre solo identidad visual y animación. No hay lógica de escritorio, IA ni detección de ventanas.

## 1. Exploración: tres conceptos

| | **A · Gota de miel** (elegido) | **B · Pompón** | **C · Pétalo** |
|---|---|---|---|
| Personalidad | Tierna, atenta, discreta | Esponjosa, juguetona | Elegante, ágil, algo seria |
| Silueta | Gota redondeada, punta superior inclinada a la izquierda, panza a la derecha. Cuerpo ≈ 74 × 76 u | Esfera peluda con borde irregular de mechones | Cuerpo alargado y estrecho, alas largas tipo hoja |
| Proporciones | Ojos grandes (14 × 17 u) en la mitad superior; alas medianas | Cuerpo muy grande, ojos puntuales, alas diminutas | Cuerpo delgado, cara pequeña, alas dominantes |
| Colores | `#F7B538` · `#3B2A21` · `#FFF1CC` | `#FFC93C` · `#5A3E2B` · `#FFF4D9` | `#F2A900` · `#2E2A3B` · `#BDE3F0` |
| Rasgos distintivos | Pelusa festoneada en el "cuello", punta de antena hexagonal, halo claro tipo sticker | Mechón rebelde, textura de pelusa | Contraste ámbar + índigo, formas angulares |
| Animación | Capas independientes (alas, antenas, ojos, cuerpo); fácil | Squash and stretch fácil, pero el borde peludo es costoso de animar limpio | Muy buen aleteo; el cuerpo largo exige rotaciones más cuidadas |
| Legibilidad a 64 px | **Alta**: silueta grande, rayas con alto contraste, ojos legibles | Media: el borde irregular se difumina | Media-baja: la cara es pequeña y las expresiones se pierden |

**Elección: A.** Es la única que deja espacio real para ocho expresiones, mantiene la identidad "abeja" con solo dos rayas y sigue siendo legible a 64 px.

## 2. Reglas del diseño (no negociables entre estados)

- **Cuerpo**: una sola gota, mismo path en todos los estados. Lo que cambia es la pose, no el dibujo.
- **Ojos**: óvalos oscuros con dos destellos blancos. El ojo derecho es ligeramente más grande (asimetría intencional).
- **Firma visual**: pelusa crema festoneada entre cara y rayas, y antena izquierda con punta hexagonal (la derecha es redonda).
- **Rayas**: dos, curvas, inclinadas 1.5 u. Nunca más de dos.
- **Alas**: blancas al 78 % con contorno azul grisáceo y una nervadura. Sin brillos adicionales.
- **Halo claro** (`#FFF7E0`) alrededor de cuerpo, antenas y patas: garantiza legibilidad sobre fondos oscuros.
- **Sin degradados.** El volumen sale de una sola sombra plana (cuarto inferior derecho) y un brillo plano.
- Tono: **tierna pero sobria**. Nada de lágrimas, corazones ni lenguas gigantes.

## 3. Paleta definitiva

| Rol | Hex | Uso |
|---|---|---|
| `ink` | `#3B2A21` | Contorno, rayas, ojos, antenas, patas, bocas |
| `honey` | `#F7B538` | Color base del cuerpo |
| `honeyLight` | `#FFD56B` | Brillo, punta de antenas |
| `honeyShade` | `#E8962A` | Sombra plana inferior derecha |
| `cream` | `#FFF1CC` | Pelusa del cuello, burbuja de pensamiento |
| `rim` | `#FFF7E0` | Halo exterior (fondos oscuros) |
| `blush` | `#F28B72` | Mejillas (opacidad .4–.8 según estado) |
| `tongue` | `#EC6F5B` | Lengua |
| `wingFill` | `#FFFFFF` | Alas (opacidad .78) |
| `wingStroke` | `#86AFCF` | Contorno de alas |
| `wingVein` | `#B7D2E5` | Nervadura |

Distribución aproximada: 60 % miel, 25 % tinta, 10 % crema, 5 % acentos (rubor, alas). El mismo contenido está en `assets/bee/palette.json`.

## 4. Estados

| Estado | Ojos | Boca | Pose y alas | Extra |
|---|---|---|---|---|
| `idle` | Abiertos | Sonrisa mínima | Inclinada −3°, aleteo suave | — |
| `happy` | Arcos "^ ^" | Sonrisa abierta con lengua | Ligeramente elevada, aleteo más vivo | Rubor fuerte |
| `curious` | Miran arriba a la derecha | "o" pequeña desplazada | Inclinada +6° | Una ceja levantada |
| `thinking` | Miran arriba a la izquierda, uno entrecerrado | Ondulada | Inclinada −5°, alas relajadas | Burbuja con "…" |
| `sleepy` | Párpados que caen hacia afuera | "o" de bostezo | Baja, antenas caídas, aleteo mínimo | "z z" sobre la cabeza |
| `surprised` | Muy abiertos | Círculo | Elevada, escala 1.03, alas arriba | Tres marcas de impacto |
| `talking` | Abiertos | Alterna abierta/cerrada | Aleteo normal | Boca animada por CSS |
| `resting` | Arcos cerrados "‿ ‿" | Sonrisa suave | Asentada y aplastada, **alas plegadas**, sin flotación | — |

## 5. Movimiento

Principio: **ligero y discreto**. Cada capa anima una sola propiedad para que nada se pelee.

| Capa | Animación | Valores base (`idle`) |
|---|---|---|
| `.float` | Traslación vertical | ±3 u, 3.8 s |
| `.sway` | Balanceo | ±1.6°, 5.4 s |
| `.pose` | Transición entre estados | 0.7 s, curva suave |
| `.wing-*` / `.flap` | Aleteo simétrico | ±7°, 0.34 s |
| `.antenna-*` `.wiggle` | Oscilación | ±3°, 4.6 s y 5.3 s (desfasadas) |
| `.eyes` | Parpadeo | cada 6 s, ~0.5 s en total (solo con ojos abiertos) |
| `.face` | Fundido cruzado | 0.3 s |

Todas las variables por estado (`--wing-amp`, `--float-amp`, `--rot`…) están en `assets/bee/bee.css`.

**Accesibilidad**: con `prefers-reduced-motion: reduce` se eliminan todas las animaciones y transiciones; la abeja sigue cambiando de estado pero de forma estática. En Windows, el ajuste "Mostrar animaciones" se refleja en esta media query dentro de Chromium/Electron.

## 6. Archivos

```
assets/bee/
  bee-idle.svg … bee-resting.svg   8 estados estáticos, autónomos, sin dependencias
  bee-rig.svg                      maestro: los 8 rostros + CSS embebido; se controla con data-state
  bee.css                          animaciones y tabla de estados (misma hoja que el rig)
  palette.json
tools/
  build_bee.py                     generador (fuente única de verdad; solo librería estándar)
  render_check.py, zoom.py         rasterizado para revisión visual (requieren rsvg-convert y Pillow)
docs/
  BEE_GUIDE.md, sheet-light.png, sheet-dark.png
```

Para modificar el personaje, edita `tools/build_bee.py` y ejecuta `python3 tools/build_bee.py`. Así los 8 estados no pueden desviarse entre sí.

Estructura interna de cada SVG (ids nombrados): `fit > bee(.float) > .sway > pose > { wings(wing-left, wing-right), antennae(antenna-left, antenna-right), legs, body(stripes, collar), face(cheeks, eyes(eye-left, eye-right), mouth, extras) }`.

## 7. Integración en React (recomendaciones, sin implementar todavía)

1. **Usa el rig, no los 8 archivos, para la mascota viva.** `bee-rig.svg` contiene los 8 rostros y cambia con `data-state`, lo que da las transiciones y el fundido cruzado gratis. Los 8 SVG estáticos sirven para documentación, iconos de bandeja, notificaciones o previsualizaciones.
2. **Inline, no `<img>`.** Un SVG dentro de `<img>` no se puede controlar desde React ni heredar el CSS de la app. Con Vite, `vite-plugin-svgr` y el sufijo `?react`.
3. **Que ningún paso de svgo toque estos assets**: puede eliminar ids, `<style>` o `viewBox`. `vite-plugin-svgr` no lo ejecuta por defecto; si añades `@svgr/plugin-svgo`, excluye `bee-*.svg` o pasa `svgo: false`. Comprobado: SVGR convierte `bee-rig.svg` a JSX válido conservando `<style>`, `className` y `data-state`, y `{...props}` queda después, así que `data-state={state}` sobrescribe el valor por defecto.
4. **Componente mínimo**:
   ```tsx
   import BeeRig from '@/assets/bee/bee-rig.svg?react';
   import '@/assets/bee/bee.css';

   export type BeeState =
     | 'idle' | 'happy' | 'curious' | 'thinking'
     | 'sleepy' | 'surprised' | 'talking' | 'resting';

   export function Bee({ state = 'idle', size = 96 }: { state?: BeeState; size?: number }) {
     return <BeeRig data-state={state} width={size} height={size} aria-label={`Miel, ${state}`} />;
   }
   ```
   El rig ya trae su `<style>`; cargar también `bee.css` es redundante pero inocuo (mismas reglas). Elige una de las dos vías.
5. **Una sola instancia visible.** Los ids del rig son fijos; si montas dos abejas a la vez, genera otra copia con prefijo (`build_rig_svg(css, p="b2-")` en `tools/build_bee.py`).
6. **Tamaños recomendados**: 64, 96 y 128 px. Por debajo de 48 px las rayas y el rostro pierden legibilidad.
7. **Electron**:
   - El `svg` tiene `overflow: visible`; deja un margen de ≥ 25 % alrededor del sprite en la ventana transparente para la flotación, el aleteo y los extras (burbuja, z, marcas).
   - Mueve la abeja con `transform: translate3d()` en un contenedor exterior, no tocando las capas internas.
   - Pausa con `animation-play-state: paused` cuando la ventana esté oculta u ocluida para ahorrar CPU.
   - `talking` ya alterna la boca por CSS; basta con activar el estado mientras muestres un mensaje.
8. **Transición entre estados**: cambia solo `data-state`. No desmontes el componente, o perderás el fundido y la inercia de la pose.

## 8. Publicación y propiedad intelectual

- El diseño está construido desde cero con curvas y primitivas propias. No se calcó, importó ni derivó de ningún personaje, imagen, juego o librería de iconos; no usa fuentes ni recursos externos.
- Se evitó a propósito la estética cúbica/voxel y se partió de una silueta, proporciones, rostro y detalles (pelusa festoneada, antena hexagonal, halo claro) propios.
- **Esto no es una garantía de ausencia de conflictos de propiedad intelectual.** Antes de distribuirla ampliamente o comercializarla, conviene buscar similitudes con mascotas existentes y revisar el nombre "Miel" por marcas.
- Recuerda añadir una licencia al repositorio y decidir si los assets llevan una distinta al código (por ejemplo, MIT para el código y CC BY 4.0 para los SVG).
