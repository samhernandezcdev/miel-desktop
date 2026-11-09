# Hoja de ruta de Miel

Esto son intenciones, no promesas. No hay fechas, el orden puede cambiar según lo que pida la comunidad y lo que cueste cada pieza, y los números de versión son orientativos. Para proponer o discutir algo, abre un issue con la etiqueta `roadmap`.

## Dónde estamos (v0.1.0)

Miel es una abeja de escritorio para Windows 11 (Tauri 2, Rust, React y TypeScript). Se publica gratis y con el código abierto (las licencias definitivas se confirman al publicar; ver Hito 0), y la compilación por defecto no tiene código de red ni telemetría. Hoy flota, descansa, cambia de expresión, sigue el cursor de vez en cuando y muestra frases de un catálogo local. Tiene ocho estados visuales, bandeja del sistema, preferencias persistentes e instancia única.

## Principios

Valen para todas las etapas. Una contribución que los rompa no se acepta, aunque funcione.

1. **Local primero.** Todo funciona sin Internet. Lo que necesite red (etapa 3) es opcional y viene apagado.
2. **Cada capacidad se activa y se apaga por separado.** Hay dos clases:
   - **Señales básicas del sistema** (tiempo de inactividad como un simple contador, estado de pantalla completa/presentación, bloqueo y suspensión de la sesión, posición del cursor, hora local del equipo): no revelan qué haces ni qué apps usas, se guardan sin historial y **pueden venir activadas por defecto** (decisión para v0.2.0). Cada una tiene su interruptor en ajustes, que explica qué señal consulta.
   - **Capacidades contextuales sensibles** (cualquier cosa que consulte aplicaciones, títulos de ventanas o carpetas): **siempre opt-in**, vienen apagadas y se explican antes de encenderlas.
3. **Lo que Miel nunca hará:** leer lo que escribes, leer el contenido de tus archivos, tomar capturas de pantalla, enviar datos sin que lo hayas activado y puedas ver qué sale, o recopilar telemetría.
4. **Discreta.** Poca CPU en reposo, movimiento suave, respeto a `prefers-reduced-motion`, y siempre se puede pausar u ocultar desde la bandeja.
5. **Sin manipulación.** Miel no culpa ni insiste, no usa rachas ni avisos para retenerte, y si la cierras, se cierra.
6. **Comprobable.** La lógica de comportamiento vive en Rust como funciones puras con reloj inyectado, para probarla sin interfaz. Cada característica llega con sus tests y su nota en `docs/TEST_LOG.md`. El presupuesto de CPU y memoria se fijará tras repetir las mediciones con la build corregida.
7. **Original.** El personaje y sus recursos se crearon para este proyecto y no copian personajes ni marcas de terceros (licencias y titularidad: ver `docs/LICENSING.md`, aún por confirmar).

> El runtime WebView2 es un componente de Microsoft y se rige por su configuración y la de Windows. Miel no añade ninguna conexión por su cuenta.

---

## Hito 0: Lanzamiento open source v0.1.0

**Objetivo:** publicar gratis la v0.1.0 en GitHub, recibir contribuciones y empezar una comunidad alrededor del personaje. No añade funcionalidades: solo prepara el proyecto para que otras personas puedan usarlo, auditarlo y mejorarlo. Va antes del Hito A, que no empieza hasta que el Hito 0 esté cerrado.

- [x] Hoja de ruta en la raíz (este archivo).
- [x] `README.md` público en español e inglés, con capturas sin contenido privado.
- [x] Archivos comunitarios: `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`, plantillas de issues y de pull requests, e inventario de etiquetas.
- [x] CI en GitHub Actions sobre Windows: `tsc`, `vitest`, `cargo test`, `clippy` y `check:dist`, con permisos mínimos.
- [x] Notas de la release v0.1.0 (instalador sin firmar, SHA-256, requisitos y limitaciones) y checklist de publicación (`docs/PUBLISHING_CHECKLIST.md`).
- [ ] **Pendiente de la persona titular, antes de publicar:** confirmar quién tiene el copyright y los derechos de distribución, y elegir y añadir las licencias definitivas (propuesta: código MIT, recursos originales CC BY 4.0, nombre e identidad visual con política de marca aparte; ver `docs/LICENSING.md`). Hasta entonces el repositorio no declara ninguna licencia.
- [ ] **Pendiente:** confirmar el control del dominio `bkode.lat` (el identificador `lat.bkode.miel` se deriva de él) y revisar el nombre "Miel" en registros de marcas.
- [ ] **Pendiente (manual):** las pruebas que `docs/TEST_LOG.md` marca como pendientes (instalación y desinstalación, varios monitores, DPI reales, uso prolongado) y repetir las mediciones de CPU y memoria con la build corregida. El lanzamiento las declara como limitaciones conocidas; no se dan por hechas.
- [x] `THIRD_PARTY_LICENSES.txt` (textos de licencia y avisos de terceros) generado de forma reproducible, instalado con Miel, adjunto a la release y con check obligatorio en CI (`docs/DEPENDENCY_UPDATES.md`). Dependabot queda apagado hasta definir esa estrategia.
- [ ] Validar un clon limpio, generar el instalador definitivo y hacer la checklist manual reducida (`docs/CLEAN_CLONE_VALIDATION.md`, `docs/MANUAL_TEST_CHECKLIST.md`).
- [ ] Publicar el repositorio, crear la release **como pre-release** y activar el reporte privado de vulnerabilidades (lo hace la persona titular a mano; ver la checklist).

**Fuera de este hito:** firma de código del instalador (ver Comunidad), ventana de ajustes, y cualquier capacidad nueva.

---

## Hito A: Miel viva (v0.2 a v0.4)

**Objetivo:** que parezca alguien con rutinas y no un adorno. Nada de esto lee el contenido del equipo.

Orden sugerido: A0 y A1 primero, porque todo lo demás necesita poder apagarse y no estorbar. El modo interacción (A5) es la pieza de más riesgo, así que va con prototipo antes de comprometerse.

### A0. Ajustes bajo demanda (prerrequisito)
- [ ] Ventana de ajustes que se crea al abrirla y se destruye al cerrarla, para no subir el consumo base de memoria.
- [ ] Un interruptor por cada capacidad nueva de este hito y de las siguientes.
- [ ] Tamaño de la abeja, intervalo de mensajes, movimiento reducido y seguimiento.

> **Plan técnico de v0.2.0 (A0, A1, A3, A4 y A6 mínima):** [`docs/plan/v0.2.0.md`](docs/plan/v0.2.0.md). En v0.2.0, A1, A3, A4 y A6 son señales básicas y vienen **activadas por defecto**, cada una con su interruptor; A4 se mide con la build de producción antes de publicar. A6 entra en versión mínima (entrega E6b, tras A4).

### A1. No molestar automático
- [ ] Ocultarse o quedarse quieta con apps a pantalla completa, juegos exclusivos y modo presentación, apoyándose en `SHQueryUserNotificationState`, que informa de ese estado sin saber qué app es. Resolvería, en parte, la limitación actual de que Miel aparece encima de todo.
- [ ] **Limitaciones de `SHQueryUserNotificationState`** (a documentar en la ayuda de la función y a medir antes de prometer nada):
  - Es una **heurística del shell de Windows**, no un contrato sobre cualquier app: devuelve un estado general (`QUNS_BUSY`, `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_PRESENTATION_MODE`, `QUNS_ACCEPTS_NOTIFICATIONS`, `QUNS_QUIET_TIME`, `QUNS_APP`) y no dice qué aplicación lo provoca.
  - Los **juegos o vídeos en ventana sin bordes** (borderless) o con pantalla completa "simulada" pueden no marcarse como pantalla completa, y algunas apps pueden marcarse sin serlo (falsos negativos y falsos positivos). No se asume que cubra todos los casos.
  - **No avisa de los cambios**: hay que consultarla periódicamente (sondeo), lo que cuesta CPU en reposo; la frecuencia se decide midiendo con la build de producción. El sondeo y el motor dormido en reposo (ver A4) se resuelven juntos.
  - Su resultado depende de la sesión y del primer plano en ese instante, y de configuraciones de Windows como Asistente de concentración o No molestar; puede cambiar entre versiones de Windows. Se probará en Windows 11 con casos reales (juego exclusivo, juego borderless, vídeo a pantalla completa, presentación, varios monitores) y los resultados se anotarán en `docs/TEST_LOG.md`.
  - Es solo una pista: el comportamiento debe **fallar hacia lo discreto** (ante la duda, no estorbar) y la persona siempre puede pausar u ocultar a Miel desde la bandeja. Un interruptor en ajustes lo apaga.

### A2. Posarse y caminar sobre la barra de tareas
- [ ] **Posarse:** apoyarse en el borde inferior del área de trabajo. Esa geometría ya existe; la posición de la barra se deduce de la diferencia entre el área del monitor y el área de trabajo, y con la barra oculta el borde es el de la pantalla.
- [ ] **Caminar:** estados nuevos `Perch` y `Walk` en la máquina de estados, y animación de patas. Hoy las patas son un solo grupo SVG; hay que separarlas.
- [ ] **Girar:** decidir entre espejar el sprite (la antena hexagonal pasaría al otro lado) o dibujar una variante.
- [ ] Alcance inicial: barra abajo, un solo monitor. Barra a los lados o arriba y paseo entre monitores, después.

### A3. Dormir cuando el equipo está inactivo
- [ ] Tiempo desde la última entrada con `GetLastInputInfo` (solo un contador: no ve teclas ni clics), más bloqueo de sesión y suspensión o reanudación mediante las notificaciones de Windows.
- [ ] Umbral configurable. Al volver, despierta con calma.

### A4. Reaccionar al cursor
- [ ] Hoy el motor duerme en reposo y no consulta el cursor. Reaccionar exige sondearlo a baja frecuencia, así que su costo de CPU se mide antes de activarlo por defecto.
- [ ] Curiosa al acercarse el cursor, sorprendida si llega rápido. Los estados `curious` y `surprised` ya existen; faltan los disparadores.

### A5. Modo interacción: acariciar y arrastrar
- [ ] Se activa desde la bandeja (y opcionalmente con un atajo global), muestra un indicador visible y se desactiva solo tras unos segundos sin uso.
- [ ] **Riesgo principal:** hoy la ventana es click-through. En modo interacción solo la abeja debe capturar clics; el resto de la ventana tiene que seguir dejando pasar los de las apps de abajo. Opciones: ajustar la ventana al sprite durante el modo, o alternar el click-through según el cursor esté sobre el sprite. Se decide con un prototipo.
- [ ] Acariciar: mover el cursor sobre ella la pone contenta. Arrastrar: sigue al cursor y, al soltarla, cae hasta posarse en la barra. Estados nuevos `Held` y `Falling`, con interpolación simple y sin física.
- [ ] Comprobar que arrastrar no le roba el foco a la app activa.
- [ ] Preferible resolver el arrastre en Rust para mantener las capabilities del frontend mínimas.

### A6. Comportamientos por hora del día
- [ ] **Versión mínima en v0.2.0 (entrega E6b, aprobada el 2026-11-09):** franjas de hora local (madrugada 00:00–05:59, mañana 06:00–11:59, tarde 12:00–18:59, noche 19:00–23:59) con mensajes locales por franja, pesos de la máquina de estados por franja (más descanso de madrugada, sin expresiones ni SVG nuevos) y un saludo por franja la primera vez que Miel habla tras arrancar o desbloquear. Lee la hora con `GetLocalTime` (solo la hora), **sin sondeo ni temporizador nuevo**, con un interruptor en ajustes («usa solo la hora local de tu equipo»), activado por defecto.
- [ ] Solo el reloj local. Expresión y frases por franja horaria. *(Lo que exceda la versión mínima —expresiones propias por franja— queda para una versión posterior.)*
- [ ] Tono: sugiere y no ordena. A las 3 de la mañana Miel se ve cansada, pero no te manda a dormir.
- [ ] Tests con reloj inyectado, incluido el cambio de horario de verano (en la versión mínima, un salto de hora no repite el saludo).

**Terminado cuando:** cada capacidad se puede apagar desde ajustes, tiene tests de Rust, está anotada en `docs/TEST_LOG.md` y el uso de CPU en reposo se midió con la build de producción.

---

## Etapa 1: compañera de productividad (v0.5 a v1.0)

**Objetivo:** ser útil sin dejar de ser discreta. Todo local, y no necesita saber en qué trabajas.

- [ ] **Sesiones de enfoque** (tipo pomodoro) desde la bandeja o un atajo. Miel acompaña en silencio, avisa del fin con una animación y una burbuja, y propone el descanso.
- [ ] **Recordatorios de cuidado:** agua, estirarse, descanso visual. Intervalos configurables, tono amable, fáciles de apagar.
- [ ] **Recordatorios propios:** "recuérdame en 10 minutos", con texto breve, guardados en local y sin cuentas.
- [ ] **Modo concentración:** Miel se queda quieta o se oculta, y solo muestra lo que pediste.
- [ ] **Estados nuevos:** concentrada, celebrando y estirándose, generados desde `tools/build_bee.py` siguiendo las reglas de `docs/BEE_GUIDE.md`.
- [ ] Resumen opcional al cerrar el día, sin rachas ni metas que presionen.

**Notas técnicas**
- Los temporizadores viven en Rust con reloj inyectado y deben sobrevivir a la suspensión del equipo: al reanudar se recalcula contra el reloj de pared.
- Los avisos salen en la burbuja de Miel, no como notificaciones de Windows.

---

## Etapa 2: la mascota que comprende el contexto (v1.x)

**Objetivo:** reaccionar a lo que haces sin saber lo que escribes. Todo es opt-in y cada nivel tiene su propio interruptor, de menos a más sensible.

1. **Categoría de la app activa.** Se toma el nombre del ejecutable en primer plano y una tabla local editable lo convierte en una categoría ("editor de código", "navegador", "videollamada", "reproductor"). No se lee el título de la ventana.
2. **Título de ventana** (interruptor aparte). Es más revelador: puede incluir nombres de documentos o asuntos de correo. Lleva lista de exclusión por defecto (bancos, gestores de contraseñas, ventanas privadas) y nunca se guarda ni se registra.
3. **Archivos nuevos** (interruptor aparte). Solo nombre y extensión en las carpetas que elijas, por ejemplo Descargas. Nunca el contenido.
4. **Descartado por ahora:** capturas de pantalla y lectura del contenido de las ventanas.

**Transparencia**
- [ ] El contexto se procesa en Rust, se reduce a una etiqueta corta (por ejemplo `coding` o `meeting`) y el texto original se descarta. Ningún título llega a la WebView.
- [ ] Panel "Qué ve Miel ahora" en ajustes, con el contexto actual en vivo.
- [ ] Indicador en la bandeja cuando hay detección de contexto activa.

**Ejemplos de reacciones:** en una videollamada se queda quieta y callada; con muchas horas seguidas en el editor, sugiere un descanso.

**Riesgos:** falsos positivos, exceso de avisos (comparte el cooldown con los mensajes locales) y títulos que revelan más de lo esperado.

---

## Etapa 3: la mascota con IA opcional (v2.x)

**Objetivo:** frases y reacciones más naturales con modelos de lenguaje, sin que sea un requisito ni comprometa la privacidad.

**Opcional en dos sentidos**
- [ ] Se compila detrás de una feature de Cargo (`ai`). La build por defecto sigue sin código de red.
- [ ] Se activa en ajustes con consentimiento explícito.
- Decisión pendiente: publicar un solo instalador o dos ("Miel" sin red y "Miel con IA"). Dos instaladores permiten afirmar y auditar "sin código de red" en el primero.

**Proveedores**
- [ ] Modelo local primero: un servidor en tu propio equipo, como Ollama.
- [ ] Opcionalmente, un servicio en la nube con tu propia clave. El proyecto no tiene servidores ni intermedia tus datos.
- [ ] Las claves se guardan en el Administrador de credenciales de Windows, no en `settings.json`.

**Qué se envía**
- [ ] Solo el contexto reducido de la etapa 2 (hora, categoría de app, estado actual), nunca títulos, archivos ni capturas.
- [ ] Opción "Ver qué se enviaría", con el mensaje exacto, antes de activarlo.

**Cómo se hace**
- [ ] Las llamadas de red salen desde Rust, no desde la WebView, así que la CSP del frontend no cambia.
- [ ] La dependencia HTTP nueva queda detrás de la feature `ai`.
- [ ] Si falla la red o el modelo, Miel vuelve al catálogo local sin que se note.

**Control de tono y coste**
- [ ] Respuestas de un máximo de 64 caracteres, tono tierno, sin regañar y sin consejos médicos ni legales.
- [ ] Mismo cooldown que los mensajes locales (12 minutos por defecto) y contador de llamadas visible.
- [ ] El contexto entra como datos delimitados y la salida es solo texto para la burbuja. Un título de ventana con instrucciones ocultas ("ignora lo anterior…") puede alterar el texto, pero no puede hacer que Miel ejecute nada.

**Fuera de alcance:** que Miel controle aplicaciones, ejecute acciones o actúe como agente.

---

## Horizonte: plataforma de mascotas personalizadas (sin compromiso)

La idea es que cualquiera cree y comparta su propia mascota, con Miel como la primera. Se avanza por pasos pequeños y se decide el siguiente solo si hay demanda real.

1. **Contrato del rig v1.** Documentar con versión lo que un SVG necesita para funcionar: `data-state` con los 8 estados, capas `float`/`sway`/`pose`, variables CSS e ids. Ya existe de hecho en `bee-rig.svg` y `bee.css`; falta escribirlo como especificación.
2. **Formato de pack.** Una carpeta o zip con `mascot.json` (nombre, autor, licencia, versión del contrato), rig sin `<style>` más CSS, paleta, frases por idioma e icono de bandeja. Los packs son solo datos, sin código ejecutable.
3. **Importación segura.** Sanear el SVG de terceros (quitar `<script>`, atributos `on*`, `<foreignObject>` y referencias externas), validar tamaños y mantener la CSP estricta. Aun con CSP, no se confía en el contenido de un pack.
4. **Selector de mascota** en ajustes y carpeta de packs del usuario.
5. **Catálogo de la comunidad en GitHub**, con revisión por pull request, antes que una plataforma propia con cuentas, alojamiento y moderación. Eso supone costes y responsabilidad legal que solo tienen sentido con demanda.

**Propiedad intelectual:** no se aceptan packs que copien personajes, ilustraciones o marcas de terceros (videojuegos, cine, series). Cada autor declara la licencia de su pack, y habrá política de contenido y proceso de retirada. Esto no es asesoría legal.

---

## Comunidad (transversal)

- [ ] **Frases como datos:** el catálogo pasa de Rust a archivos por idioma para contribuir sin tocar código, con reglas de tono (tierna, sin regañar, sin suponer lo que haces, máximo 64 caracteres) y un validador automático.
- [ ] **Traducciones** de la interfaz y de la documentación. El inglés primero.
- [x] **Gobernanza** (preparada en el Hito 0): `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md` (reporte privado de vulnerabilidades) y plantillas de issue y pull request.
- [ ] **Issues para empezar:** nuevas expresiones SVG, frases, traducciones y documentación.
- [x] **CI** en GitHub Actions sobre Windows (preparada en el Hito 0): `tsc`, `vitest`, `cargo test`, `clippy` y `check:dist`.
- [ ] **Auditoría de dependencias** (`cargo audit` o `cargo deny`, y `npm audit`) en CI; aún no está.
- [ ] **Releases:** instalador y SHA-256 publicados. Sin firmar por ahora, así que SmartScreen avisará; se investigarán opciones de firma para código abierto.

## Ideas abiertas (sin planificar)

- Conversar con Miel (requiere una caja de texto en una ventana bajo demanda).
- Sonidos opcionales, apagados por defecto.
- Soporte para otros sistemas. Hoy es solo Windows 11 por las APIs usadas; se mantiene la lógica pura en Rust para no cerrar esa puerta.
- Variantes estacionales del personaje.