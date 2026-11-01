# Checklist manual reducida para v0.1.0

Se hace **con el instalador definitivo** (el generado desde el clon limpio; ver `CLEAN_CLONE_VALIDATION.md`). Nada de esto
está hecho todavía: **no marques una casilla que no ejecutaste**. Anota los resultados en `docs/TEST_LOG.md` (sección nueva).

| Dato | Valor |
|---|---|
| Fecha / persona | |
| Windows (versión y compilación) | |
| Commit validado | |
| SHA-256 del instalador **probado** (debe coincidir con la release) | |
| ¿Había una versión anterior de Miel instalada? | En el equipo de desarrollo **sí** (clave de desinstalación `Miel 0.1.0` presente): esta prueba es una **actualización** sobre ella. Para probar instalación limpia, desinstala antes (sin marcar la casilla de datos si quieres conservar preferencias) |

Preparación: `Get-FileHash` del `.exe` descargado y comparación con `SHA256SUMS.txt`. Cierra Miel antes de instalar (menú de la bandeja → Cerrar).

| # | Prueba | Cómo | Resultado esperado | ✔ / ✘ | Notas |
|---|---|---|---|---|---|
| 1 | **Instalación** | Ejecuta el instalador (verás SmartScreen: *Más información → Ejecutar de todas formas*, solo tras verificar el hash). No pide administrador. Luego: `.\tools\check_install.ps1 -Phase Installed -LicensesFile .\THIRD_PARTY_LICENSES.txt` | Instala en `%LOCALAPPDATA%\Miel`; aparece en *Aplicaciones*; el script da **OK**: `miel.exe` y **`THIRD_PARTY_LICENSES.txt`** presentes y con el mismo SHA-256 | | |
| 2 | **Animaciones** | Arranca Miel y observa 1–2 minutos | La abeja flota, bate las alas y parpadea; el movimiento es continuo | | |
| 3 | **Un solo rostro** | Mira la abeja de cerca y deja que cambie de estado (Follow/Rest/Reaction) | **Un único rostro** a la vez (nunca varios apilados ni nube de pensamiento y "z" juntas); mejillas de color normal, no rojo intenso | | |
| 4 | **Bandeja** | Icono en *Mostrar iconos ocultos* → menú | Mostrar/Ocultar, Seguir cursor (casilla), Pausar/**Reanudar** (la etiqueta cambia y la abeja se queda quieta al pausar) y Cerrar funcionan; una 2.ª ejecución de Miel **no** crea otra abeja ni otro icono | | |
| 5 | **Click-through** | Coloca la abeja sobre un editor o el navegador; haz clic y escribe **encima y alrededor** de ella | Los clics pasan a la app de debajo; Miel no roba el foco ni se interpone al escribir | | |
| 6 | **Burbuja** | Con Miel **sin pausar**, espera hasta ~6 minutos (la primera tarda 2–4 min y sale en un cambio de comportamiento; luego hay 12 min entre mensajes) | Aparece una burbuja crema con una frase en español sobre la abeja, con su cara a juego; desaparece sola a los 4–6 s; **pausar** cancela la burbuja | | |
| 7 | **Desinstalación** | *Configuración → Aplicaciones → Miel → Desinstalar* (prueba una vez con la casilla *Eliminar los datos de la aplicación* y otra sin ella). Luego: `.\tools\check_install.ps1 -Phase Uninstalled` | El script da **OK**: **`THIRD_PARTY_LICENSES.txt` eliminado**, `miel.exe` eliminado y carpeta vacía o inexistente; con la casilla marcada desaparecen también `%APPDATA%\lat.bkode.miel` y `%LOCALAPPDATA%\lat.bkode.miel`; sin ella se conservan | | |

Si algo falla: anota el síntoma, la versión de Windows y, si existe, `frontend-errors.log` (revísalo antes de compartirlo; sin datos personales) y abre un issue.

## Limitaciones declaradas (NO se cubren con esta checklist y siguen pendientes)

Se publican como limitaciones conocidas (README y notas de release); **no se presentan como probadas**:

- Prueba prolongada de **ocho horas** (estabilidad y memoria).
- **Varios monitores** (cruce entre pantallas, coordenadas negativas con el instalador).
- **Cambios de DPI/escala** y de resolución reales.
- **Nuevas mediciones de rendimiento** (CPU/RAM) con la build corregida: las de `docs/PERFORMANCE.md` están invalidadas.
