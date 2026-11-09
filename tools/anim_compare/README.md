# Comparación visual antes/después (sin capturar tu pantalla)

Renderiza `bee-rig.nostyle.svg` + un `bee.css` con Chromium/Edge **sin cabeza** (`--headless=new`, perfil temporal) y fija
las animaciones CSS en instantes exactos (Web Animations API: `pause()` + `currentTime`), de modo que el fotograma *i* es el mismo
instante en todas las variantes. No usa tu escritorio ni tu ventana de Miel.

```
python -I tools/build_bee.py --out <dir> --css-only [--rest-fps 10|20] [--idle-flap standard|lowamp|burst]   # CSS de cada variante
node tools/anim_compare/capture.mjs <config.json> <carpeta>          # fotogramas PNG (config: ver el encabezado del script)
python -I -X utf8 tools/anim_compare/make_gif.py <carpeta> <estado> <salida.gif> <variante> [<variante> ...]   # GIF lado a lado
```
Solo biblioteca estándar de Python y Node 22 (WebSocket y fetch incorporados); necesita Edge o Chrome instalado. Resultado: `docs/img/anim-compare/` (carpeta ignorada por Git: los GIF no se versionan, se regeneran con estos comandos).
