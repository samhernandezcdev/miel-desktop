# Política de seguridad / Security policy

**[Español](#español) · [English](#english)**

---

# Español

## Cómo reportar una vulnerabilidad

**No abras un issue público** para vulnerabilidades. Usa el reporte privado de GitHub: pestaña **Security → Report a vulnerability** de este repositorio (la persona titular debe tenerlo activado; ver `docs/PUBLISHING_CHECKLIST.md`). Si no está disponible, escribe a **[CONTACTO DE SEGURIDAD: pendiente de definir por la persona titular antes de publicar]**.

Incluye, si puedes: versión de Miel y de Windows, pasos para reproducirlo, impacto que imaginas y, si existe, una propuesta de corrección. No incluyas datos personales ajenos.

## Qué esperar

Es un proyecto comunitario y gratuito, sin SLA. La intención es acusar recibo en pocos días, evaluarlo, preparar una corrección cuando proceda y darte crédito si lo deseas. Te pedimos tiempo razonable para corregir antes de divulgarlo públicamente.

## Versiones con soporte

| Versión | Soporte de seguridad |
|---|---|
| 0.1.x | Sí (última publicada) |
| Anteriores | No |

## Qué se considera en alcance

Por ejemplo: ejecución de código o lectura/escritura de archivos fuera del directorio propio de Miel, apertura de red no esperada, fuga de datos, debilitamiento de la CSP o de las capabilities de Tauri, problemas en el instalador NSIS o en la forma de distribuirlo, y vulnerabilidades en dependencias que afecten a Miel de forma real.

Fuera de alcance: problemas de la propia configuración de Windows o de WebView2 (repórtalos a Microsoft), ingeniería social, y la advertencia de SmartScreen por el instalador sin firmar (limitación conocida).

## Modelo de seguridad, en pocas líneas

- Sin red, telemetría, cuentas ni claves en el código de Miel; CSP estricta (`default-src 'self'`, `connect-src` solo IPC local) y capabilities mínimas (`src-tauri/capabilities/default.json`).
- Escribe solo en `%APPDATA%\lat.bkode.miel` y `%LOCALAPPDATA%\lat.bkode.miel`.
- El instalador **no está firmado**: verifica siempre el SHA-256 publicado en la release (ver `README.md`).

---

# English

## Reporting a vulnerability

**Do not open a public issue** for vulnerabilities. Use GitHub private reporting: **Security → Report a vulnerability** on this repository (the owner must enable it; see `docs/PUBLISHING_CHECKLIST.md`). If unavailable, write to **[SECURITY CONTACT: to be defined by the project owner before publishing]**.

Please include, if you can: Miel and Windows versions, reproduction steps, the impact you foresee and, if any, a suggested fix. Do not include other people's personal data.

## What to expect

This is a free, community project with no SLA. We aim to acknowledge reports within a few days, assess them, prepare a fix when appropriate and credit you if you wish. Please allow reasonable time to fix before public disclosure.

## Supported versions

| Version | Security support |
|---|---|
| 0.1.x | Yes (latest published) |
| Older | No |

## Scope

In scope, for example: code execution or file read/write outside Miel's own directories, unexpected network access, data leaks, weakened CSP or Tauri capabilities, issues in the NSIS installer or its distribution, and dependency vulnerabilities that actually affect Miel.

Out of scope: issues in Windows or WebView2 themselves (report to Microsoft), social engineering, and the SmartScreen warning for the unsigned installer (known limitation).

## Security model in brief

- No network, telemetry, accounts or keys in Miel's code; strict CSP and minimal Tauri capabilities.
- Writes only to `%APPDATA%\lat.bkode.miel` and `%LOCALAPPDATA%\lat.bkode.miel`.
- The installer is **not signed**: always verify the SHA-256 published with the release (see `README.md`).
