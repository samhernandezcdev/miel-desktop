# Miel v0.1.0

> Notas para el cuerpo de la GitHub Release, que se publica como **PRE-RELEASE** (y primero en borrador). Antes de publicar: sustituir
> `[SHA256 DEL INSTALADOR]` y `[SHA256 DE LOS AVISOS]` por los hashes de los archivos que realmente se suban, calculados del
> **instalador definitivo generado desde un clon limpio del código versionado** (`docs/CLEAN_CLONE_VALIDATION.md`), no de ningún
> instalador provisional; y revisar los marcadores `[...]` (`docs/PUBLISHING_CHECKLIST.md`).

**[Español](#español) · [English](#english)**

---

# Español

> **Pre-release.** Primera versión pública, con pruebas manuales aún pendientes (ver *Limitaciones conocidas*).

Primera versión pública de **Miel**, una abeja animada para el escritorio de Windows 11: flota, a veces sigue al cursor, descansa, cambia de expresión y de vez en cuando dice una frase corta. Local, discreta, sin conexión, sin cuentas y sin telemetría.

## ⚠️ Instalador sin firmar

`Miel_0.1.0_x64-setup.exe` **no tiene firma de código**, así que **Windows SmartScreen puede mostrar "Windows protegió su PC"**. Es esperable en un instalador sin firmar y con pocas descargas. Descárgalo solo desde esta página, **verifica el SHA-256** y, si coincide y decides continuar, usa *Más información → Ejecutar de todas formas*. **No desactives SmartScreen ni tu antivirus.** Si prefieres no ejecutar un binario sin firmar, compílalo desde el código (ver el README).

## Verificar el SHA-256

```powershell
Get-FileHash .\Miel_0.1.0_x64-setup.exe -Algorithm SHA256
```

El resultado debe coincidir con `SHA256SUMS.txt` de esta release:

```
[SHA256 DEL INSTALADOR]  Miel_0.1.0_x64-setup.exe
[SHA256 DE LOS AVISOS]  THIRD_PARTY_LICENSES.txt
```

Si no coincide, **no lo ejecutes** y abre un issue.

## Qué incluye

- Ventana transparente, sin marco, siempre visible, *click-through* y sin robar el foco.
- Abeja original con 8 expresiones y animaciones.
- Comportamientos Idle, Follow, Rest y Reaction.
- Mensajes locales en una burbuja (16 frases; por defecto, 12 minutos entre mensajes; el primero tarda 2–4 minutos).
- Bandeja del sistema: Mostrar/Ocultar, Seguir cursor, Pausar/Reanudar, Cerrar.
- Preferencias persistentes (JSON), instancia única, varios monitores, DPI y coordenadas negativas.
- Funciona sin conexión; no hay red, telemetría, cuentas ni claves en el código de Miel.

## Requisitos

- Windows 11 **x64** con WebView2 (incluido en Windows 11; si faltara, el instalador descarga el *bootstrapper* de Microsoft y solo en ese caso necesita conexión).
- Instalación **por usuario**: no pide privilegios de administrador (`%LOCALAPPDATA%\Miel`).

## Limitaciones conocidas

- Instalador **sin firmar** (SmartScreen avisará).
- **Pruebas manuales pendientes**, no se dan por hechas: instalación/actualización/desinstalación de principio a fin, varios monitores y cambios reales de DPI/resolución, uso prolongado (8 h) y comprobaciones con el instalador ya instalado. Detalle en `docs/TEST_LOG.md`.
- **Sin cifras de rendimiento**: las mediciones de CPU/RAM anteriores están invalidadas (se tomaron con una build sin animaciones CSS) y deben repetirse con la build corregida.
- Miel aparece **encima de todo**, también de apps a pantalla completa (aún no hay modo "no molestar").
- Sin ventana de ajustes: tamaño, velocidad e intervalo de mensajes se cambian editando `settings.json` con Miel cerrada.
- Textos de la interfaz y frases solo en español. Solo Windows 11 x64.
- Si WebView2 falla, la ventana queda vacía (la bandeja sigue funcionando).

## Para desinstalar

*Configuración → Aplicaciones → Miel → Desinstalar.* La casilla *Eliminar los datos de la aplicación* borra además las preferencias (`%APPDATA%\lat.bkode.miel`) y los datos de WebView2 (`%LOCALAPPDATA%\lat.bkode.miel`).

## Licencias y dependencias

**Licencias:** código bajo MIT o Apache-2.0 (a tu elección); arte, personaje y documentación bajo CC BY 4.0; el nombre "Miel", BKODE y el logotipo, sin licenciar (política de marca en `TRADEMARK_POLICY.md`). © 2026 Samuel David Hernández Contreras (BKODE). Avisos y licencias completos de terceros: `THIRD_PARTY_LICENSES.txt` (adjunto a esta release y también instalado junto a Miel en `%LOCALAPPDATA%\Miel`). Lista: `docs/THIRD_PARTY.md`.

## Contribuir y reportar

Issues, ideas y contribuciones son bienvenidas: ver `CONTRIBUTING.md`, `ROADMAP.md` y el Código de conducta. Las vulnerabilidades se reportan en privado (`SECURITY.md`).

---

# English

> **Pre-release.** First public release, with manual tests still pending (see *Known limitations*).

First public release of **Miel**, an animated bee for the Windows 11 desktop: it floats, sometimes follows the cursor, rests, changes expression, and now and then says a short line. Local, discreet, offline, no accounts, no telemetry.

## ⚠️ Unsigned installer

`Miel_0.1.0_x64-setup.exe` is **not code-signed**, so **Windows SmartScreen may show "Windows protected your PC"**. That is expected for an unsigned, little-downloaded installer. Download it only from this page, **verify the SHA-256**, and if it matches and you decide to continue, use *More info → Run anyway*. **Do not disable SmartScreen or your antivirus.** If you would rather not run an unsigned binary, build it from source (see the README).

## Verify the SHA-256

```powershell
Get-FileHash .\Miel_0.1.0_x64-setup.exe -Algorithm SHA256
```

The result must match `SHA256SUMS.txt` in this release:

```
[SHA256 OF THE INSTALLER]  Miel_0.1.0_x64-setup.exe
[SHA256 OF THE NOTICES]  THIRD_PARTY_LICENSES.txt
```

If it does not match, **do not run it** and open an issue.

## What's included

- Transparent, frameless, always-on-top, click-through window that never steals focus.
- Original bee with 8 expressions and animations.
- Idle, Follow, Rest and Reaction behaviors.
- Local messages in a speech bubble (16 phrases; 12 minutes apart by default; the first appears after 2–4 minutes).
- System tray: Show/Hide, Follow cursor, Pause/Resume, Quit.
- Persistent preferences (JSON), single instance, multi-monitor, DPI and negative coordinates.
- Works offline; no networking, telemetry, accounts or keys in Miel's code.

## Requirements

- Windows 11 **x64** with WebView2 (included in Windows 11; if missing, the installer downloads Microsoft's bootstrapper and only then needs a connection).
- **Per-user** install: no administrator rights (`%LOCALAPPDATA%\Miel`).

## Known limitations

- **Unsigned** installer (SmartScreen will warn).
- **Pending manual tests**, not claimed as done: full install/update/uninstall, multiple monitors and real DPI/resolution changes, long-running use (8 h), and checks with the installed build. See `docs/TEST_LOG.md`.
- **No performance figures**: earlier CPU/RAM measurements are invalidated (taken with a build without CSS animations) and must be redone with the fixed build.
- Miel shows **on top of everything**, including full-screen apps (no "do not disturb" yet).
- No settings window: size, speed and message interval are changed by editing `settings.json` while Miel is closed.
- UI text and phrases are Spanish-only. Windows 11 x64 only.
- If WebView2 fails, the window stays empty (the tray keeps working).

## Uninstalling

*Settings → Apps → Miel → Uninstall.* The *Delete application data* checkbox also removes preferences (`%APPDATA%\lat.bkode.miel`) and WebView2 data (`%LOCALAPPDATA%\lat.bkode.miel`).

## Licenses and dependencies

**Licenses:** code under MIT or Apache-2.0 (at your option); art, character and documentation under CC BY 4.0; the name "Miel", BKODE and the logo are not licensed (trademark policy in `TRADEMARK_POLICY.md`). © 2026 Samuel David Hernández Contreras (BKODE). Full third-party notices and licenses: `THIRD_PARTY_LICENSES.txt` (attached to this release and also installed next to Miel in `%LOCALAPPDATA%\Miel`). List: `docs/THIRD_PARTY.md`.

## Contributing and reporting

Issues, ideas and contributions are welcome: see `CONTRIBUTING.md`, `ROADMAP.md` and the Code of Conduct. Report vulnerabilities privately (`SECURITY.md`).
