//! Motor de comportamiento y movimiento: UN hilo, bloqueado en una `Condvar`.
//!
//! - Duerme hasta el próximo plazo de la FSM, o hasta una orden (`Cmd`) o el cierre.
//! - Solo "tickea" (≈30 Hz) mientras hay movimiento o se sigue al cursor; en Idle/Rest quietos,
//!   Paused y Hidden no hay polling (el hilo permanece bloqueado).
//! - El cursor solo se consulta durante Follow.
//! - Terminación explícita: `Engine::shutdown()` (también en `Drop`) activa una bandera, despierta
//!   el hilo y lo une (`join`).
//!
//! - Mensajes (`Talker`) y cambios de geometría reutilizan este mismo hilo y su Condvar: no hay
//!   hilos ni temporizadores adicionales. El único plazo extra es el fin de la burbuja visible y,
//!   tras un aviso de cambio de pantalla, un pequeño debounce.
//!
//! Todo acceso al sistema pasa por el trait `Host` → el motor se prueba sin ventana. El tiempo pasa por
//! el trait `Clock` (`SystemClock` en producción, `FakeClock` en pruebas).

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::clock::{Clock, SystemClock};
use super::dnd::{self, Change, Detector};
use super::sleep;
use super::geometry::{
    anchor_of, clamp_anchor, ensure_center_visible, follow_target, fully_visible, nearest_area,
    rest_spot, window_origin, Frame, Rect, Vec2,
};
use super::messages::{Talker, TalkerConfig};
use super::motion::{Motion, MotionParams};
use super::probe::SystemProbe;
use super::sleep::{Action, SleepTask};
use super::state::{Behavior, Fsm, Ms, Timings, View};

#[derive(Debug, Clone, Copy)]
pub struct MonitorInfo {
    /// Área de trabajo en píxeles físicos (excluye la barra de tareas).
    pub work: Rect,
    pub scale: f64,
}

/// Frontera con el sistema. Implementación real: `host::TauriHost`.
pub trait Host: Send + Sync + 'static {
    /// Posición física del cursor (solo se llama durante Follow).
    fn cursor(&self) -> Option<Vec2>;
    fn monitors(&self) -> Vec<MonitorInfo>;
    /// Rectángulo físico actual de la ventana.
    fn window_rect(&self) -> Option<Rect>;
    fn set_position(&self, x: i32, y: i32);
    fn set_visible(&self, visible: bool);
    /// Notifica un cambio de vista (solo cuando cambia, nunca por frame).
    fn emit_view(&self, view: &View);
}

/// Órdenes al motor. Las de comportamiento las envía la bandeja; `GeometryChanged` y
/// `NativeGeometryEvents`, los avisos nativos de pantalla/DPI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// Ajustes explícitos (la bandeja usa los `Toggle*`); los usará la carga de preferencias (Etapa 7).
    #[allow(dead_code)]
    Pause(bool),
    Hidden(bool),
    #[allow(dead_code)]
    FollowEnabled(bool),
    /// Cambió la resolución, el DPI, el área de trabajo o la disposición de monitores.
    /// Se agrupa (debounce) y provoca relectura + reubicación si la posición dejó de ser válida.
    GeometryChanged,
    /// Alternan sobre el estado REAL del motor (no sobre una copia en la UI): sin carreras de doble clic.
    TogglePause,
    ToggleFollow,
    /// Los avisos nativos de pantalla están instalados: ya no hace falta la revisión de
    /// seguridad al despertar (queda solo como respaldo si la instalación falla).
    NativeGeometryEvents,
    /// Empieza (`true`) o termina (`false`) un episodio de supresión automática (E4). Al terminar se
    /// limpia la anulación. **E1b: solo alcanzable desde pruebas y `diag`** (en producción nadie lo envía).
    #[allow(dead_code)]
    Suppressed(bool),
    /// "Mostrar de todos modos": anula el episodio de supresión actual (si no hay ninguno en vigor, no
    /// hace nada). **E1b: solo alcanzable desde pruebas y `diag`.**
    #[allow(dead_code)]
    DndOverride,
    /// El interruptor «No molestar automático» cambió (E4b): se aplica al instante. `false` termina cualquier
    /// episodio y detiene el sondeo; `true` lo reanuda (con una lectura inmediata si Miel no está oculta ni pausada).
    DndAuto(bool),
    /// El interruptor «Sueño» cambió (E5): se aplica al instante. `false` despierta y detiene el sondeo.
    SleepEnabled(bool),
    /// Nuevo umbral de inactividad en ms (E5): recalcula el plazo al instante.
    SleepAfter(Ms),
    /// Bloqueo/desbloqueo de la sesión y suspensión/reanudación del equipo (E5).
    Session(SessionEvent),
}

/// Avisos de sesión y energía de Windows (`WTS_SESSION_LOCK/UNLOCK`, `PBT_APMSUSPEND/RESUME*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Locked,
    Unlocked,
    Suspended,
    Resumed,
}

#[derive(Debug, Clone, Copy)]
pub struct EngineConfig {
    pub timings: Timings,
    /// Velocidad máxima (px lógicos/s) y aceleración (px lógicos/s²).
    pub speed: f64,
    pub accel: f64,
    pub tolerance: f64,
    /// Distancia cómoda al cursor (px lógicos).
    pub follow_distance: f64,
    /// Frecuencia mientras hay movimiento (Hz).
    pub follow_hz: f64,
    /// Frecuencia de vigilancia del cursor durante Follow cuando la abeja ya está quieta (Hz).
    pub follow_settled_hz: f64,
    /// Cada cuánto se reconsultan monitores/ventana mientras se mueve.
    pub refresh_every: Duration,
    /// Los avisos de cambio de pantalla llegan en ráfagas: se espera este tiempo antes de actuar.
    pub geometry_debounce: Duration,
    /// Posición del ancla (centro del sprite) dentro de la ventana, como fracción (x, y).
    /// Debe coincidir con `--bee-anchor-y` en `app.css` (la burbuja ocupa el espacio de arriba).
    pub anchor_frac: (f64, f64),
    pub talker: TalkerConfig,
    /// Estado inicial (preferencias persistidas): seguimiento permitido y pausa. `Hidden` nunca se restaura.
    pub initial_follow: bool,
    pub initial_paused: bool,
    /// «No molestar automático» activado al arrancar (`dnd_auto_enabled`). Sin sonda no hace nada.
    pub dnd_auto: bool,
    /// Intervalo del sondeo de No molestar (ms del reloj del motor).
    pub dnd_poll_ms: Ms,
    /// «Sueño» activado al arrancar (`sleep_enabled`) y umbral de inactividad (ms). Sin sonda no hace nada.
    pub sleep_enabled: bool,
    pub sleep_after_ms: Ms,
    pub seed: Option<u64>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            timings: Timings::default(),
            speed: 260.0,
            accel: 800.0,
            tolerance: 1.5,
            follow_distance: 130.0,
            follow_hz: 30.0,
            follow_settled_hz: 10.0,
            refresh_every: Duration::from_secs(2),
            geometry_debounce: Duration::from_millis(300),
            anchor_frac: ANCHOR_FRAC,
            talker: TalkerConfig::default(),
            initial_follow: true,
            initial_paused: false,
            dnd_auto: false,
            dnd_poll_ms: dnd::POLL_MS,
            sleep_enabled: false,
            sleep_after_ms: sleep::DEFAULT_AFTER_MS,
            seed: None,
        }
    }
}

/// Sprite centrado en horizontal y al 64 % de la altura de la ventana (260×200 lógicos →
/// 128 px de espacio para la burbuja sobre el sprite y 72 px debajo).
pub const ANCHOR_FRAC: (f64, f64) = (0.5, 0.64);

struct Inner {
    cmds: Vec<Cmd>,
    shutdown: bool,
    /// El hilo del motor terminó (lo marca su guarda, también si hay un pánico).
    stopped: bool,
    view: View,
}

struct Shared {
    inner: Mutex<Inner>,
    cv: Condvar,
    /// Aviso de "motor detenido" (condvar aparte: `notify_one` de `send` nunca despierta a un
    /// esperador de cierre en lugar del motor).
    done: Condvar,
}

/// Marca `stopped` al salir del hilo del motor, por cualquier camino.
struct StopGuard<'a>(&'a Shared);

impl Drop for StopGuard<'_> {
    fn drop(&mut self) {
        lock(&self.0.inner).stopped = true;
        self.0.done.notify_all();
    }
}

fn lock(m: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Manejador del motor (estado gestionado por Tauri).
pub struct Engine {
    shared: Arc<Shared>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl Engine {
    pub fn spawn<H: Host>(host: Arc<H>, cfg: EngineConfig) -> Engine {
        Self::spawn_with_clock(host, cfg, Arc::new(SystemClock::new()))
    }

    /// Como `spawn`, con el reloj del motor inyectado (pruebas deterministas con `FakeClock`). Sin sonda del
    /// sistema: No molestar automático no hace nada.
    pub fn spawn_with_clock<H: Host>(host: Arc<H>, cfg: EngineConfig, clock: Arc<dyn Clock>) -> Engine {
        Self::spawn_with(host, cfg, clock, None)
    }

    /// Arranque completo: reloj y sonda del sistema inyectados (producción: `SystemClock` y `WindowsProbe`).
    pub fn spawn_with<H: Host>(
        host: Arc<H>,
        cfg: EngineConfig,
        clock: Arc<dyn Clock>,
        probe: Option<Arc<dyn SystemProbe>>,
    ) -> Engine {
        let mut rng = match cfg.seed {
            Some(s) => fastrand::Rng::with_seed(s),
            None => fastrand::Rng::new(),
        };
        let mut fsm = Fsm::new(cfg.timings, 0, &mut rng);
        let mut talker = Talker::new(cfg.talker, 0, &mut rng);
        if !cfg.initial_follow {
            fsm.set_follow_enabled(0, &mut rng, false);
        }
        if cfg.initial_paused {
            fsm.set_paused(0, true);
            talker.set_suspended(0, true);
        }
        let shared = Arc::new(Shared {
            inner: Mutex::new(Inner {
                cmds: Vec::new(),
                shutdown: false,
                stopped: false,
                view: compose(&fsm, &talker),
            }),
            cv: Condvar::new(),
            done: Condvar::new(),
        });
        let mut worker = Worker::new(host, cfg, fsm, talker, rng, clock);
        worker.shared = Some(Arc::clone(&shared));
        worker.probe = probe;
        let sh = Arc::clone(&shared);
        let handle = std::thread::Builder::new()
            .name("miel-engine".into())
            .spawn(move || worker.run(&sh))
            .expect("no se pudo crear el hilo del motor");
        Engine { shared, join: Mutex::new(Some(handle)) }
    }

    pub fn send(&self, cmd: Cmd) {
        {
            let mut g = lock(&self.shared.inner);
            if g.shutdown {
                return; // cerrando: no se programan tareas nuevas
            }
            g.cmds.push(cmd);
        }
        self.shared.cv.notify_all();
    }

    /// Estado actual (consulta de solo lectura para la sincronización inicial).
    pub fn view(&self) -> View {
        lock(&self.shared.inner).view
    }

    /// Pide la terminación SIN esperar (seguro desde el hilo principal): activa la bandera y
    /// despierta la Condvar. Idempotente. A partir de aquí `send` descarta órdenes.
    pub fn request_stop(&self) {
        lock(&self.shared.inner).shutdown = true;
        self.shared.cv.notify_all();
    }

    /// Espera (como máximo `timeout`) a que el hilo del motor haya terminado. `true` si terminó.
    pub fn wait_stopped(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut g = lock(&self.shared.inner);
        while !g.stopped {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            g = self.shared.done.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner()).0;
        }
        true
    }

    /// Terminación completa con plazo: `request_stop` + espera + `join` (solo si terminó, para no
    /// bloquear si el hilo estuviera atascado en una llamada del sistema). Idempotente.
    /// Debe llamarse con el bucle de eventos ya detenido o desde un hilo que no sea el principal:
    /// las consultas de ventana de Tauri esperan respuesta del hilo principal.
    pub fn shutdown_timeout(&self, timeout: Duration) -> bool {
        self.request_stop();
        let stopped = self.wait_stopped(timeout);
        if stopped {
            let handle = self.join.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(h) = handle {
                let _ = h.join();
            }
        }
        stopped
    }

    pub fn shutdown(&self) {
        self.shutdown_timeout(Duration::from_secs(5));
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Vista pública: estado de la FSM + mensaje visible. La versión es la suma de los contadores
/// monótonos de ambos → sube con cualquier cambio observable de cualquiera de los dos.
fn same_view(a: &View, b: &View) -> bool {
    (a.behavior, a.expression, a.paused, a.hidden, a.follow_enabled, a.message, a.suppressed, a.sleeping)
        == (b.behavior, b.expression, b.paused, b.hidden, b.follow_enabled, b.message, b.suppressed, b.sleeping)
}

fn compose(fsm: &Fsm, talker: &Talker) -> View {
    let mut v = fsm.view();
    v.version += talker.version();
    if let (Some(m), Some(e)) = (talker.message(), talker.expression_override()) {
        v.message = Some(m);
        v.expression = e;
    }
    v
}

/// Periodo de una espera expresado en milisegundos enteros, redondeado hacia ARRIBA. La `Condvar` de Windows ya
/// redondeaba hacia arriba los `Duration` con fracciones de ms (33,3 ms → 34 ms), así que la espera efectiva
/// no cambia al pasar los plazos a milisegundos del motor.
fn period_ms(hz: f64) -> Ms {
    (1000.0 / hz).ceil() as Ms
}

/// Próximo instante (en ms del reloj del motor) en que el hilo debe despertar por sí mismo; `None` = dormir
/// indefinidamente. Mínimo absoluto de: plazo de la FSM, fin de la burbuja del hablante, cadencia del
/// movimiento (en movimiento o siguiendo al cursor), debounce de geometría y sondeo de No molestar (con su puerta).
/// Suspendido (oculta, en pausa o suprimida sin anulación) solo cuentan el debounce y el sondeo. Solo lee: no cambia nada.
fn next_deadline<H: Host>(w: &Worker<H>, now: Ms) -> Option<Ms> {
    // Sondeo de No molestar: tiene su propia puerta (también corre MIENTRAS está suprimida, para saber cuándo termina;
    // nunca con la ventana oculta o pausada por la persona).
    let by_dnd = if w.dnd_gate() { w.dnd.next_poll } else { None };
    // Sueño (E5): `idle_check` despierta / `wake_poll` durmiendo; su puerta exige estar corriendo.
    let by_sleep = if w.sleep_gate() { w.sleep.next } else { None };
    if w.fsm.is_suspended() {
        // pausado/oculto/suprimido: solo un aviso de geometría o el sondeo de recuperación pueden despertarlo
        return [w.geom_due, by_dnd].into_iter().flatten().min();
    }
    let by_state = [w.fsm.next_wakeup(), w.talker.next_wakeup()].into_iter().flatten().min();
    let by_motion = if w.moving {
        Some(now + period_ms(w.cfg.follow_hz))
    } else if w.fsm.behavior() == Behavior::Follow {
        Some(now + period_ms(w.cfg.follow_settled_hz))
    } else {
        None
    };
    [by_state, by_motion, w.geom_due, by_dnd, by_sleep].into_iter().flatten().min()
}

struct Worker<H: Host> {
    host: Arc<H>,
    cfg: EngineConfig,
    fsm: Fsm,
    talker: Talker,
    rng: fastrand::Rng,
    clock: Arc<dyn Clock>,
    motion: Motion,
    mons: Vec<MonitorInfo>,
    areas: Vec<Rect>,
    win: Frame,
    last_set: (i32, i32),
    last_refresh: Option<Ms>,
    last_tick: Ms,
    rest_target: Option<Vec2>,
    moving: bool,
    /// Instante en que se procesará un aviso de cambio de geometría (debounce).
    geom_due: Option<Ms>,
    topology: Vec<(Rect, u64)>,
    last_emitted: View,
    native_events: bool,
    /// Última visibilidad pedida a la ventana (empieza visible; la primera muestra la hace el ciclo de vida).
    shown: bool,
    /// Estado compartido con `Engine` (la vista publicada). `None` en las pruebas sin hilo.
    shared: Option<Arc<Shared>>,
    /// Sonda del sistema (E4b). `None` = sin sondeo (pruebas y plataformas sin sonda).
    probe: Option<Arc<dyn SystemProbe>>,
    /// Tarea `dnd_poll` del planificador.
    dnd: DndTask,
    /// Tareas `idle_check` / `wake_poll` del planificador (E5).
    sleep: SleepTask,
}

/// Estado de la tarea de sondeo de No molestar. Solo hay un plazo (`next_poll`) y entra en `next_deadline`.
struct DndTask {
    /// Interruptor «No molestar automático».
    enabled: bool,
    det: Detector,
    /// Próximo sondeo. `Some` solo mientras la puerta está abierta (ver `Worker::dnd_gate`).
    next_poll: Option<Ms>,
}

impl<H: Host> Worker<H> {
    fn new(
        host: Arc<H>,
        cfg: EngineConfig,
        fsm: Fsm,
        talker: Talker,
        rng: fastrand::Rng,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let (fx, fy) = cfg.anchor_frac;
        let last_emitted = compose(&fsm, &talker);
        let last_tick = clock.now_ms();
        let dnd_auto = cfg.dnd_auto;
        let sleep = SleepTask::new(cfg.sleep_enabled, cfg.sleep_after_ms);
        Self {
            host,
            cfg,
            fsm,
            talker,
            rng,
            clock,
            motion: Motion::default(),
            mons: Vec::new(),
            areas: Vec::new(),
            win: Frame::with_anchor_frac(260.0, 200.0, fx, fy),
            last_set: (0, 0),
            last_refresh: None,
            last_tick,
            rest_target: None,
            moving: false,
            geom_due: None,
            topology: Vec::new(),
            last_emitted,
            native_events: false,
            shown: true,
            shared: None,
            probe: None,
            dnd: DndTask { enabled: dnd_auto, det: Detector::new(), next_poll: None },
            sleep,
        }
    }

    /// Alinea el mensaje visible con `messages_allowed` (pausa, oculta, suprimida o dormida lo cancelan).
    fn sync_talker(&mut self, now: Ms) -> bool {
        self.talker.set_suspended(now, !self.fsm.messages_allowed())
    }

    /// Alinea la ventana con la visibilidad efectiva (oculta por la persona o suprimida). Al volver a
    /// mostrarla relee monitores y reubica si la posición dejó de ser válida.
    fn sync_visibility(&mut self) {
        let visible = self.fsm.is_visible();
        if visible != self.shown {
            self.shown = visible;
            self.host.set_visible(visible);
            if visible {
                self.on_geometry_changed();
            }
        }
    }

    fn now(&self) -> Ms {
        self.clock.now_ms()
    }

    // ---- No molestar automático (E4b): una tarea más del planificador único ----------------------------------

    /// Puerta de estado del sondeo (plan §4.4): hay sonda, el interruptor está activo y la persona no ocultó ni
    /// pausó a Miel. SÍ corre mientras está suprimida (para saber cuándo termina el episodio).
    fn dnd_gate(&self) -> bool {
        self.probe.is_some() && self.dnd.enabled && !self.fsm.is_hidden() && !self.fsm.is_paused()
    }

    /// Alinea `next_poll` con la puerta. Al abrirse (arranque, mostrar, reanudar, activar el interruptor) se consulta de
    /// inmediato y las rachas de histéresis empiezan de cero; al cerrarse (ocultar/pausar a mano, apagar) el sondeo se
    /// detiene por completo.
    fn dnd_sync_gate(&mut self, now: Ms) {
        match (self.dnd_gate(), self.dnd.next_poll) {
            (true, None) => {
                self.dnd.det.forget_streaks();
                self.dnd.next_poll = Some(now);
            }
            (false, Some(_)) => self.dnd.next_poll = None,
            _ => {}
        }
    }

    /// Abre o cierra el episodio de supresión en la FSM y alinea ventana y mensajes.
    fn set_episode(&mut self, now: Ms, suppressed: bool) -> bool {
        // Al terminar el episodio se reconsulta la inactividad ANTES de mostrar (plan §4.4): reaparece dormida si toca.
        let slept = !suppressed && self.fsm.is_suppression_episode() && self.sleep_reevaluate(now);
        let changed = self.fsm.set_suppressed(now, suppressed) | slept;
        self.sync_visibility();
        self.sync_talker(now) | changed
    }

    /// Si el sondeo vence, lee la sonda, aplica la histéresis y, si cambia el episodio, lo traslada a la FSM.
    fn dnd_poll(&mut self, now: Ms) -> bool {
        let Some(due) = self.dnd.next_poll else { return false };
        if now < due || !self.dnd_gate() {
            return false;
        }
        self.dnd.next_poll = Some(now + self.cfg.dnd_poll_ms);
        let Some(probe) = self.probe.clone() else { return false };
        match self.dnd.det.feed(probe.notification_state()) {
            Some(Change::Enter) => {
                if crate::DIAG {
                    eprintln!("[miel] No molestar: empieza la supresión");
                }
                self.set_episode(now, true)
            }
            Some(Change::Exit) => {
                if crate::DIAG {
                    eprintln!("[miel] No molestar: termina la supresión");
                }
                self.set_episode(now, false)
            }
            Some(Change::ExitByErrors) => {
                eprintln!("[miel] No molestar: {} errores seguidos de la sonda; termina la supresión", dnd::MAX_ERRORS);
                self.set_episode(now, false)
            }
            None => false,
        }
    }

    // ---- Sueño (E5): tareas `idle_check` y `wake_poll` del planificador único -------------------------------------

    /// Puerta de estado (plan §4.4): hay sonda, el interruptor está activo, Miel corre (visible y sin pausa) y la
    /// sesión no está bloqueada ni suspendida (entonces duerme sin sondear). Nunca corre oculta, pausada ni suprimida.
    fn sleep_gate(&self) -> bool {
        self.probe.is_some() && self.sleep.enabled && self.fsm.is_running() && !self.sleep.away.any()
    }

    /// Alinea `next` con la puerta: al abrirse se consulta de inmediato; al cerrarse el sondeo se detiene por completo.
    fn sleep_sync_gate(&mut self, now: Ms) {
        match (self.sleep_gate(), self.sleep.next) {
            (true, None) => {
                self.sleep.errors = 0;
                self.sleep.next = Some(now);
            }
            (false, Some(_)) => self.sleep.next = None,
            _ => {}
        }
    }

    /// Entra en el sueño (sin sorpresa). Devuelve `true` si cambió algo observable.
    fn fall_asleep(&mut self, now: Ms) -> bool {
        if crate::DIAG && !self.fsm.is_sleeping() {
            eprintln!("[miel] Sueño: se duerme");
        }
        let changed = self.fsm.set_sleeping(now, true);
        self.sync_talker(now) | changed
    }

    /// Despierta con la sorpresa breve (solo se ve si está corriendo). `activity`: la causa cuenta como actividad
    /// reciente (acción de la persona, desbloqueo, reanudación) aunque el contador de Windows aún no la refleje.
    fn wake_up(&mut self, now: Ms, activity: bool) -> bool {
        if activity {
            self.sleep.activity_at = Some(now);
        }
        if crate::DIAG && self.fsm.is_sleeping() {
            eprintln!("[miel] Sueño: despierta");
        }
        let was_asleep = self.fsm.is_sleeping();
        let changed = self.fsm.wake(now);
        if was_asleep {
            // «sin mensaje» al despertar: ni durante la sorpresa ni justo al terminar (el fin de la reacción es un
            // cambio de comportamiento y, con el cooldown ya vencido, soltaría una frase). Un respiro breve.
            self.talker.hold_off(now, sleep::WAKE_QUIET_MS);
        }
        self.sync_talker(now) | changed
    }

    /// Acción explícita de la persona (bandeja, ajustes): termina el sueño sin esperar al sondeo (oculta o pausada no
    /// se muestrea). Con la sesión bloqueada o suspendida no hace nada (no hay persona).
    fn explicit_wake(&mut self, now: Ms) -> bool {
        if self.sleep.away.any() || !self.fsm.is_sleeping() {
            return false;
        }
        self.wake_up(now, true)
    }

    /// Lee la sonda y aplica el veredicto. Devuelve `true` si cambió algo observable.
    fn sleep_apply_reading(&mut self, now: Ms) -> bool {
        let Some(probe) = self.probe.clone() else { return false };
        let asleep = self.fsm.is_sleeping();
        match self.sleep.on_reading(now, asleep, probe.idle_ms()) {
            Action::None => false,
            Action::Sleep => self.fall_asleep(now),
            Action::Wake => self.wake_up(now, false),
        }
    }

    /// Reconsulta inmediata (fin de un episodio de supresión, antes de mostrar). Sin sonda, interruptor apagado o
    /// sesión ausente no cambia nada.
    fn sleep_reevaluate(&mut self, now: Ms) -> bool {
        if self.probe.is_none() || !self.sleep.enabled || self.sleep.away.any() {
            return false;
        }
        self.sleep_apply_reading(now)
    }

    /// Si la comprobación vence (y la puerta está abierta), lee la sonda.
    fn sleep_poll(&mut self, now: Ms) -> bool {
        let Some(due) = self.sleep.next else { return false };
        if now < due || !self.sleep_gate() {
            return false;
        }
        self.sleep_apply_reading(now)
    }

    fn on_session(&mut self, ev: SessionEvent, now: Ms) -> bool {
        match ev {
            SessionEvent::Locked => self.sleep.away.locked = true,
            SessionEvent::Suspended => self.sleep.away.suspended = true,
            SessionEvent::Unlocked => self.sleep.away.locked = false,
            SessionEvent::Resumed => self.sleep.away.suspended = false,
        }
        if !self.sleep.enabled {
            return false;
        }
        if self.sleep.away.any() {
            // bloqueo o suspensión: duerme de inmediato, sin esperar el umbral ni sondear
            self.sleep.next = None;
            self.fall_asleep(now)
        } else {
            // desbloqueo/reanudación (y nada más lo mantiene ausente): despierta con la sorpresa y cuenta como actividad
            self.wake_up(now, true)
        }
    }

    fn run(mut self, shared: &Shared) {
        let _stop = StopGuard(shared);
        self.refresh(true);
        let now0 = self.now();
        self.dnd_sync_gate(now0);
        self.sleep_sync_gate(now0);
        loop {
            let timeout = self.wait_timeout();
            let cmds = {
                let mut g = lock(&shared.inner);
                if g.shutdown {
                    return;
                }
                if g.cmds.is_empty() {
                    g = match timeout {
                        Some(d) => shared.cv.wait_timeout(g, d).unwrap_or_else(|e| e.into_inner()).0,
                        None => shared.cv.wait(g).unwrap_or_else(|e| e.into_inner()),
                    };
                    if g.shutdown {
                        return;
                    }
                }
                std::mem::take(&mut g.cmds)
            };
            let now = self.now();
            if crate::DIAG {
                // Contador de despertares del planificador (E5, solo diagnóstico): una línea por despertar con el
                // estado en que estaba; las cifras por minuto se calculan fuera (tools/e5/analyze_e5.py).
                let unix_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
                eprintln!("[miel] t={unix_ms} despertar {}", self.wake_label());
            }
            self.tick(now, cmds);
        }
    }

    /// Estado del motor para el contador de despertares (diagnóstico).
    fn wake_label(&self) -> &'static str {
        if self.fsm.is_hidden() {
            "oculta"
        } else if self.fsm.is_paused() {
            "pausada"
        } else if !self.fsm.is_visible() {
            "suprimida"
        } else if self.fsm.is_sleeping() {
            "durmiendo"
        } else {
            match self.fsm.behavior() {
                Behavior::Idle => "idle",
                Behavior::Follow => "follow",
                Behavior::Rest => "rest",
                Behavior::Reaction => "reaction",
            }
        }
    }

    /// Un paso del motor tras despertar: aplica las órdenes, avanza FSM y hablante, emite la vista si cambió,
    /// procesa el aviso de geometría vencido y mueve la abeja. No espera ni crea hilos: se prueba con `FakeClock`.
    fn tick(&mut self, now: Ms, cmds: Vec<Cmd>) {
        let mut changed = false;
        for c in cmds {
            changed |= self.apply(c, now);
        }
        self.dnd_sync_gate(now);
        self.sleep_sync_gate(now);
        changed |= self.fsm.step(now, &mut self.rng);
        changed |= self.dnd_poll(now);
        self.sleep_sync_gate(now); // el sondeo de No molestar pudo cerrar o abrir la puerta (episodio)
        changed |= self.sleep_poll(now);
        // la reacción de despertar o el paso sleepy→resting pueden vencer en este mismo instante
        changed |= self.fsm.step(now, &mut self.rng);
        changed |= self.sync_talker(now);
        changed |= self.talker.step(now);
        changed |= self.talker.observe(now, &self.fsm.view(), &mut self.rng);
        if changed {
            // La FSM puede cambiar algo que el mensaje visible tapa (p. ej. sleepy→resting con
            // la cara del mensaje activa): solo se emite si la vista compuesta es distinta.
            let view = compose(&self.fsm, &self.talker);
            if !same_view(&view, &self.last_emitted) {
                self.last_emitted = view;
                if let Some(shared) = &self.shared {
                    lock(&shared.inner).view = view; // antes de emitir, como siempre
                }
                self.host.emit_view(&view);
            }
        }
        if self.geom_due.is_some_and(|d| self.now() >= d) {
            self.geom_due = None;
            self.on_geometry_changed();
        }
        self.drive_motion();
    }

    /// Cuánto puede dormir el hilo. `None` = indefinidamente (suspendido y sin avisos pendientes).
    fn wait_timeout(&self) -> Option<Duration> {
        let now = self.now();
        next_deadline(self, now).map(|d| Duration::from_millis(d.saturating_sub(now)))
    }

    fn apply(&mut self, cmd: Cmd, now: Ms) -> bool {
        match cmd {
            Cmd::Pause(p) => {
                let mut changed = self.fsm.set_paused(now, p);
                if !p {
                    changed |= self.explicit_wake(now); // reanudar desde la bandeja: el sueño termina ya
                }
                self.sync_talker(now) | changed
            }
            Cmd::Hidden(h) => {
                let mut changed = self.fsm.set_hidden(now, h);
                if !h {
                    changed |= self.explicit_wake(now); // mostrar desde la bandeja
                }
                self.sync_visibility(); // al mostrar: relee monitores y reubica si hace falta
                self.sync_talker(now) | changed
            }
            Cmd::Suppressed(s) => self.set_episode(now, s),
            Cmd::DndOverride => {
                let mut changed = self.fsm.set_dnd_override(now);
                if changed {
                    changed |= self.explicit_wake(now); // «Mostrar de todos modos»
                }
                self.sync_visibility();
                self.sync_talker(now) | changed
            }
            Cmd::DndAuto(on) => {
                self.dnd.enabled = on;
                if on {
                    false // `dnd_sync_gate` abre la puerta (si procede) y lee de inmediato
                } else {
                    // apagar termina CUALQUIER episodio al instante (también limpia la anulación) y detiene el sondeo
                    self.dnd.det.reset();
                    self.dnd.next_poll = None;
                    self.set_episode(now, false)
                }
            }
            Cmd::SleepEnabled(on) => {
                self.sleep.enabled = on;
                self.sleep.errors = 0;
                if on {
                    // activar con la sesión ausente (no ocurre desde la interfaz): duerme; si no, `sleep_sync_gate` lee ya
                    if self.sleep.away.any() {
                        return self.fall_asleep(now);
                    }
                    false
                } else {
                    self.sleep.next = None; // apagar detiene el sondeo y despierta
                    let was_away = self.sleep.away.any();
                    let c = if self.fsm.is_sleeping() { self.wake_up(now, !was_away) } else { false };
                    c | self.sync_talker(now)
                }
            }
            Cmd::SleepAfter(ms) => {
                self.sleep.after = ms.max(1);
                self.sleep.next = None; // `sleep_sync_gate` lo reabre con una lectura inmediata (recalcula el plazo)
                false
            }
            Cmd::Session(ev) => self.on_session(ev, now),
            Cmd::FollowEnabled(e) => {
                let mut changed = self.fsm.set_follow_enabled(now, &mut self.rng, e);
                if e {
                    changed |= self.explicit_wake(now); // activar «Seguir» despierta (plan §4.2 c)
                }
                changed
            }
            Cmd::TogglePause => {
                let mut changed = self.fsm.toggle_paused(now);
                if !self.fsm.is_paused() {
                    changed |= self.explicit_wake(now);
                }
                self.sync_talker(now) | changed
            }
            Cmd::ToggleFollow => {
                let mut changed = self.fsm.toggle_follow(now, &mut self.rng);
                if self.fsm.view().follow_enabled {
                    changed |= self.explicit_wake(now);
                }
                changed
            }
            Cmd::NativeGeometryEvents => {
                self.native_events = true;
                false
            }
            Cmd::GeometryChanged => {
                // ráfaga de avisos → una sola relectura tras el debounce
                let due = self.now() + self.cfg.geometry_debounce.as_millis() as Ms;
                self.geom_due.get_or_insert(due);
                false
            }
        }
    }

    /// Relee monitores y rectángulo de la ventana (re-sincroniza si algo la movió desde fuera).
    fn refresh(&mut self, force: bool) {
        if !force && !self.refresh_due() {
            return;
        }
        self.last_refresh = Some(self.now());
        let mons = self.host.monitors();
        if !mons.is_empty() {
            let topo: Vec<(Rect, u64)> = mons.iter().map(|m| (m.work, m.scale.to_bits())).collect();
            if topo != self.topology {
                self.topology = topo;
                self.rest_target = None; // el destino de descanso depende del área de trabajo
            }
            self.areas = mons.iter().map(|m| m.work).collect();
            self.mons = mons;
        }
        if let Some(r) = self.host.window_rect() {
            let (fx, fy) = self.cfg.anchor_frac;
            self.win = Frame::with_anchor_frac(r.w as f64, r.h as f64, fx, fy);
            if (r.x, r.y) != self.last_set || self.motion.pos == Vec2::default() {
                self.motion = Motion::at(anchor_of((r.x, r.y), self.win));
                self.last_set = (r.x, r.y);
            }
        }
    }

    /// ¿Ha pasado `refresh_every` desde la última relectura de monitores/ventana?
    fn refresh_due(&self) -> bool {
        let every = self.cfg.refresh_every.as_millis() as Ms;
        self.last_refresh.is_none_or(|t| self.now().saturating_sub(t) >= every)
    }

    /// Reacción a un cambio real de pantalla: relee monitores/ventana y, si la posición dejó de
    /// ser válida (monitor desconectado, resolución menor, área de trabajo reducida), reubica a
    /// Miel dentro del área visible. Funciona también en Idle, Paused y Hidden.
    fn on_geometry_changed(&mut self) {
        self.refresh(true);
        if self.areas.is_empty() {
            return;
        }
        if !fully_visible(&self.areas, self.motion.pos, self.win) {
            let fixed = clamp_anchor(&self.areas, self.motion.pos, self.win);
            self.motion.pos = fixed;
            self.motion.stop();
            self.moving = false;
            self.place(fixed);
        }
    }

    fn scale_here(&self) -> f64 {
        nearest_area(&self.areas, self.motion.pos)
            .and_then(|a| self.mons.iter().find(|m| m.work == a))
            .map_or(1.0, |m| m.scale)
    }

    fn params(&self, factor: f64) -> MotionParams {
        let s = self.scale_here();
        MotionParams {
            max_speed: self.cfg.speed * s * factor,
            max_accel: self.cfg.accel * s,
            tolerance: self.cfg.tolerance * s,
        }
    }

    fn place(&mut self, anchor: Vec2) {
        let origin = window_origin(anchor, self.win);
        if origin != self.last_set {
            self.host.set_position(origin.0, origin.1);
            self.last_set = origin;
        }
    }

    fn drive_motion(&mut self) {
        let now = self.now();
        let dt = now.saturating_sub(self.last_tick) as f64 / 1000.0;
        self.last_tick = now;
        if self.fsm.is_suspended() {
            self.motion.stop();
            self.moving = false;
            return;
        }
        let b = self.fsm.behavior();
        let active = self.moving || b == Behavior::Follow || b == Behavior::Rest;
        if active {
            self.refresh(false);
        } else if !self.native_events && self.refresh_due() {
            // Respaldo SIN avisos nativos: al despertar (fin de Idle), ¿sigue la abeja visible?
            self.refresh(true);
            if !fully_visible(&self.areas, self.motion.pos, self.win) && !self.areas.is_empty() {
                let fixed = clamp_anchor(&self.areas, self.motion.pos, self.win);
                self.motion.pos = fixed;
                self.motion.stop();
                self.place(fixed);
            }
        }

        match b {
            Behavior::Idle | Behavior::Follow => self.rest_target = None,
            Behavior::Rest if self.rest_target.is_none() => {
                self.rest_target = Some(rest_spot(&self.areas, self.motion.pos, self.win));
            }
            _ => {}
        }

        let (target, factor) = match b {
            Behavior::Follow => {
                let t = self.host.cursor().map(|c| {
                    let dist = self.cfg.follow_distance * self.scale_here();
                    follow_target(c, self.motion.pos, dist, &self.areas, self.win)
                });
                (t, 1.0)
            }
            Behavior::Rest | Behavior::Reaction => (self.rest_target, 0.6),
            Behavior::Idle => (None, 1.0),
        };

        let params = self.params(factor);
        self.moving = self.motion.step(target, dt, &params);
        if self.moving {
            let p = ensure_center_visible(&self.areas, self.motion.pos, self.win);
            self.motion.pos = p;
            self.place(p);
        } else {
            // llegada o parada: coloca la posición final exacta
            let p = self.motion.pos;
            self.place(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::CATALOG;
    use super::super::state::Expression;
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::thread::sleep;

    struct Fake {
        cursor: Mutex<Option<Vec2>>,
        cursor_calls: AtomicUsize,
        positions: Mutex<Vec<(i32, i32)>>,
        views: Mutex<Vec<View>>,
        visible: AtomicBool,
        visible_calls: AtomicUsize,
        /// Simula un `getter` de Tauri que espera al hilo principal: mientras sea `true`, `cursor()` se bloquea.
        cursor_blocked: Mutex<bool>,
        cursor_cv: Condvar,
        in_cursor: AtomicBool,
        win: Mutex<Rect>,
        monitors: Mutex<Vec<MonitorInfo>>,
        monitor_calls: AtomicUsize,
        stamps: Mutex<Vec<(Instant, i32, i32)>>,
    }

    impl Fake {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                cursor: Mutex::new(Some(Vec2::new(500.0, 400.0))),
                cursor_calls: AtomicUsize::new(0),
                positions: Mutex::new(vec![]),
                views: Mutex::new(vec![]),
                visible: AtomicBool::new(true),
                visible_calls: AtomicUsize::new(0),
                cursor_blocked: Mutex::new(false),
                cursor_cv: Condvar::new(),
                in_cursor: AtomicBool::new(false),
                win: Mutex::new(Rect::new(1600, 780, 260, 200)),
                monitors: Mutex::new(vec![MonitorInfo { work: Rect::new(0, 0, 1920, 1032), scale: 1.0 }]),
                monitor_calls: AtomicUsize::new(0),
                stamps: Mutex::new(vec![]),
            })
        }
        fn calls(&self) -> usize {
            self.cursor_calls.load(Ordering::SeqCst)
        }
        fn moves(&self) -> usize {
            self.positions.lock().unwrap().len()
        }
    }

    impl Host for Fake {
        fn cursor(&self) -> Option<Vec2> {
            self.cursor_calls.fetch_add(1, Ordering::SeqCst);
            self.in_cursor.store(true, Ordering::SeqCst);
            let mut blocked = self.cursor_blocked.lock().unwrap();
            while *blocked {
                blocked = self.cursor_cv.wait(blocked).unwrap();
            }
            drop(blocked);
            *self.cursor.lock().unwrap()
        }
        fn monitors(&self) -> Vec<MonitorInfo> {
            self.monitor_calls.fetch_add(1, Ordering::SeqCst);
            self.monitors.lock().unwrap().clone()
        }
        fn window_rect(&self) -> Option<Rect> {
            Some(*self.win.lock().unwrap())
        }
        fn set_position(&self, x: i32, y: i32) {
            self.positions.lock().unwrap().push((x, y));
            self.stamps.lock().unwrap().push((Instant::now(), x, y));
            let mut w = self.win.lock().unwrap();
            w.x = x;
            w.y = y;
        }
        fn set_visible(&self, v: bool) {
            self.visible_calls.fetch_add(1, Ordering::SeqCst);
            self.visible.store(v, Ordering::SeqCst);
        }
        fn emit_view(&self, v: &View) {
            self.views.lock().unwrap().push(*v);
        }
    }

    fn quiet_cfg() -> EngineConfig {
        // Idle larguísimo: el motor debe dormir sin hacer nada.
        EngineConfig {
            timings: Timings { idle: (600_000, 600_000), ..Timings::default() },
            seed: Some(1),
            ..EngineConfig::default()
        }
    }

    fn follow_cfg() -> EngineConfig {
        EngineConfig {
            timings: Timings {
                idle: (20, 20),
                follow: (400, 400),
                follow_cooldown: 600_000,
                weights: (1, 0, 0),
                reaction_cooldown: 600_000,
                ..Timings::default()
            },
            speed: 3000.0,
            accel: 20_000.0,
            seed: Some(1),
            ..EngineConfig::default()
        }
    }

    #[test]
    fn idle_does_not_poll_cursor_or_move() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_cfg());
        sleep(Duration::from_millis(250));
        assert_eq!(host.calls(), 0);
        assert_eq!(host.moves(), 0);
        e.shutdown();
    }

    #[test]
    fn paused_and_hidden_do_no_work() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), follow_cfg());
        sleep(Duration::from_millis(150)); // ya en Follow
        e.send(Cmd::Pause(true));
        sleep(Duration::from_millis(100));
        let (c, m) = (host.calls(), host.moves());
        sleep(Duration::from_millis(300));
        assert_eq!((host.calls(), host.moves()), (c, m), "pausado: sin cursor ni movimiento");
        assert!(e.view().paused);
        e.send(Cmd::Hidden(true));
        sleep(Duration::from_millis(100));
        assert!(!host.visible.load(Ordering::SeqCst));
        let c2 = host.calls();
        sleep(Duration::from_millis(250));
        assert_eq!(host.calls(), c2);
        e.shutdown();
    }

    #[test]
    fn follow_moves_towards_cursor_then_stops_polling() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), follow_cfg());
        sleep(Duration::from_millis(250));
        assert!(host.calls() > 0, "Follow debe consultar el cursor");
        assert!(host.moves() > 0, "debe moverse");
        // fin del Follow (400 ms) + margen para frenar
        sleep(Duration::from_millis(700));
        let (c, m) = (host.calls(), host.moves());
        sleep(Duration::from_millis(400));
        assert_eq!((host.calls(), host.moves()), (c, m), "tras Follow: sin polling ni movimiento");
        // se acercó al cursor pero conservando distancia (no sobre él)
        let w = *host.win.lock().unwrap();
        let anchor = anchor_of((w.x, w.y), Frame::with_anchor_frac(260.0, 200.0, ANCHOR_FRAC.0, ANCHOR_FRAC.1));
        let d = anchor.dist(Vec2::new(500.0, 400.0));
        assert!(d < 700.0, "se acercó al cursor (partía a ≈1315 px): {d}");
        assert!(d > 100.0, "no se posó sobre el cursor: {d}");
        e.shutdown();
    }

    #[test]
    fn views_are_emitted_with_increasing_versions() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), follow_cfg());
        sleep(Duration::from_millis(900));
        e.shutdown();
        let views = host.views.lock().unwrap();
        assert!(views.len() >= 2, "Idle→Follow→Idle: {}", views.len());
        assert!(views.windows(2).all(|w| w[0].version < w[1].version));
        assert_eq!(views[0].behavior, Behavior::Follow);
    }

    #[test]
    fn shutdown_is_prompt_idempotent_and_joins() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_cfg());
        let t = Instant::now();
        e.shutdown();
        e.shutdown();
        assert!(t.elapsed() < Duration::from_millis(500));
        let n = host.moves();
        e.send(Cmd::Pause(true)); // tras el cierre no debe pasar nada ni colgarse
        sleep(Duration::from_millis(50));
        assert_eq!(host.moves(), n);
    }

    #[test]
    fn drop_terminates_the_thread() {
        let host = Fake::new();
        {
            let _e = Engine::spawn(host.clone(), follow_cfg());
            sleep(Duration::from_millis(100));
        }
        let c = host.calls();
        sleep(Duration::from_millis(200));
        assert_eq!(host.calls(), c, "el hilo no sigue vivo tras Drop");
        assert_eq!(Arc::strong_count(&host), 1, "el hilo liberó su Arc<Host>");
    }

    #[test]
    fn follow_disabled_never_polls() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), follow_cfg());
        e.send(Cmd::FollowEnabled(false));
        sleep(Duration::from_millis(60));
        let c = host.calls();
        sleep(Duration::from_millis(500));
        assert_eq!(host.calls(), c);
        e.shutdown();
    }

    #[test]
    fn offscreen_window_is_pulled_back_on_wake() {
        let host = Fake::new();
        *host.win.lock().unwrap() = Rect::new(2500, 500, 260, 200); // monitor desconectado
        let cfg = EngineConfig {
            timings: Timings { idle: (50, 50), weights: (0, 0, 0), ..Timings::default() },
            refresh_every: Duration::from_millis(10),
            seed: Some(1),
            ..EngineConfig::default()
        };
        let e = Engine::spawn(host.clone(), cfg);
        sleep(Duration::from_millis(400));
        e.shutdown();
        let w = *host.win.lock().unwrap();
        assert!(w.x as f64 + 260.0 <= 1920.0 && w.x >= 0, "x={}", w.x);
    }

    #[test]
    fn motion_params_scale_with_dpi_at_100_125_150_200() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let cfg = EngineConfig::default();
            let mut rng = fastrand::Rng::with_seed(1);
            let fsm = Fsm::new(cfg.timings, 0, &mut rng);
            let talker = Talker::new(cfg.talker, 0, &mut rng);
            let mut w = Worker::new(Fake::new(), cfg, fsm, talker, rng, Arc::new(SystemClock::new()));
            let work = Rect::new(0, 0, 3840, 2100);
            w.areas = vec![work];
            w.mons = vec![MonitorInfo { work, scale }];
            w.motion = Motion::at(Vec2::new(1000.0, 1000.0));
            let p = w.params(1.0);
            assert!((p.max_speed - cfg.speed * scale).abs() < 1e-9);
            assert!((p.max_accel - cfg.accel * scale).abs() < 1e-9);
            assert!((p.tolerance - cfg.tolerance * scale).abs() < 1e-9);
            assert!((w.scale_here() - scale).abs() < 1e-9);
        }
    }

    #[test]
    fn engine_emits_reactions_and_rest_with_expressions() {
        use super::super::state::Expression;
        let host = Fake::new();
        let cfg = EngineConfig {
            timings: Timings {
                idle: (20, 20),
                follow: (100, 100),
                reaction: (60, 60),
                rest: (150, 150),
                rest_settle: 50,
                follow_cooldown: 600_000,
                reaction_cooldown: 0,
                weights: (0, 1, 1),
            },
            seed: Some(3),
            ..EngineConfig::default()
        };
        let e = Engine::spawn(host.clone(), cfg);
        sleep(Duration::from_millis(1500));
        e.shutdown();
        let views = host.views.lock().unwrap();
        let reactions = views.iter().filter(|v| v.behavior == Behavior::Reaction).count();
        let rests = views.iter().filter(|v| v.behavior == Behavior::Rest).count();
        assert!(reactions > 0, "sin reacciones: {} vistas", views.len());
        assert!(rests > 0);
        assert!(views.iter().any(|v| v.expression == Expression::Resting));
        assert!(views.windows(2).all(|w| w[0].version < w[1].version));
    }

    // ---- cambios de geometría ----

    fn geo_cfg() -> EngineConfig {
        EngineConfig {
            timings: Timings { idle: (600_000, 600_000), ..Timings::default() },
            geometry_debounce: Duration::from_millis(40),
            seed: Some(1),
            ..EngineConfig::default()
        }
    }

    fn set_monitor(host: &Fake, work: Rect, scale: f64) {
        *host.monitors.lock().unwrap() = vec![MonitorInfo { work, scale }];
    }

    #[test]
    fn geometry_change_wakes_a_sleeping_engine_without_any_polling() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), geo_cfg());
        sleep(Duration::from_millis(150));
        let base = host.monitor_calls.load(Ordering::SeqCst);
        sleep(Duration::from_millis(400));
        assert_eq!(host.monitor_calls.load(Ordering::SeqCst), base, "reposo: cero consultas de monitores");
        // la resolución baja: la ventana (x=1600..1860, y=780..980) queda fuera del área 1280×720
        set_monitor(&host, Rect::new(0, 0, 1280, 720), 1.0);
        e.send(Cmd::GeometryChanged);
        sleep(Duration::from_millis(300));
        let w = *host.win.lock().unwrap();
        assert!(w.x >= 0 && w.x + 260 <= 1280 && w.y >= 0 && w.y + 200 <= 720, "ventana fuera: {w:?}");
        assert_eq!(host.monitor_calls.load(Ordering::SeqCst), base + 1);
        e.shutdown();
    }

    #[test]
    fn burst_of_geometry_events_is_coalesced_into_one_refresh() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), geo_cfg());
        sleep(Duration::from_millis(100));
        let base = host.monitor_calls.load(Ordering::SeqCst);
        for _ in 0..20 {
            e.send(Cmd::GeometryChanged);
        }
        sleep(Duration::from_millis(300));
        assert_eq!(host.monitor_calls.load(Ordering::SeqCst), base + 1);
        e.shutdown();
    }

    #[test]
    fn geometry_change_with_valid_position_does_not_move() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), geo_cfg());
        sleep(Duration::from_millis(100));
        e.send(Cmd::GeometryChanged);
        sleep(Duration::from_millis(250));
        assert_eq!(host.moves(), 0, "posición todavía válida: no se toca");
        e.shutdown();
    }

    #[test]
    fn geometry_change_relocates_even_when_paused_or_hidden() {
        for cmd in [Cmd::Pause(true), Cmd::Hidden(true)] {
            let host = Fake::new();
            let e = Engine::spawn(host.clone(), geo_cfg());
            sleep(Duration::from_millis(100));
            e.send(cmd);
            sleep(Duration::from_millis(60));
            let cursor_before = host.calls();
            set_monitor(&host, Rect::new(-1920, 0, 1920, 1032), 1.0); // solo queda el monitor izquierdo
            e.send(Cmd::GeometryChanged);
            sleep(Duration::from_millis(300));
            let w = *host.win.lock().unwrap();
            assert!(w.x >= -1920 && w.x + 260 <= 0, "{cmd:?}: {w:?}");
            assert_eq!(host.calls(), cursor_before, "sin consultar el cursor");
            e.shutdown();
        }
    }

    #[test]
    fn dpi_change_resyncs_window_size_and_keeps_it_inside() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), geo_cfg());
        sleep(Duration::from_millis(100));
        // 150 %: Windows reescala la ventana (390×300 físicos) y la deja desbordando por abajo-derecha
        *host.win.lock().unwrap() = Rect::new(1600, 780, 390, 300);
        set_monitor(&host, Rect::new(0, 0, 1920, 1032), 1.5);
        e.send(Cmd::GeometryChanged);
        sleep(Duration::from_millis(300));
        let w = *host.win.lock().unwrap();
        assert!(w.x + 390 <= 1920 && w.y + 300 <= 1032, "{w:?}");
        e.shutdown();
    }

    // ---- velocidad con delta time real ----

    #[test]
    fn speed_limit_holds_against_real_elapsed_time() {
        let host = Fake::new();
        let cfg = EngineConfig {
            speed: 500.0,
            accel: 100_000.0,
            follow_hz: 60.0,
            timings: Timings { follow: (1500, 1500), ..follow_cfg().timings },
            ..follow_cfg()
        };
        let e = Engine::spawn(host.clone(), cfg);
        sleep(Duration::from_millis(1400));
        e.shutdown();
        let st = host.stamps.lock().unwrap();
        assert!(st.len() > 10, "debe haberse movido: {}", st.len());
        let mut worst: f64 = 0.0;
        for w in st.windows(2) {
            let dt = w[1].0.duration_since(w[0].0).as_secs_f64().max(0.001);
            let d = f64::hypot((w[1].1 - w[0].1) as f64, (w[1].2 - w[0].2) as f64);
            worst = worst.max((d - 2.0) / dt); // 2 px de holgura por redondeo a enteros
        }
        assert!(worst <= 500.0 * 1.35, "velocidad instantánea {worst:.0} px/s > límite 500 (+35 % de jitter)");
        // y en ventanas de ~0.3 s el promedio no supera el límite
        for (i, a) in st.iter().enumerate() {
            if let Some(b) = st[i..].iter().find(|b| b.0.duration_since(a.0).as_secs_f64() >= 0.3) {
                let v = f64::hypot((b.1 - a.1) as f64, (b.2 - a.2) as f64) / b.0.duration_since(a.0).as_secs_f64();
                assert!(v <= 500.0 * 1.1, "promedio {v:.0} px/s");
            }
        }
    }

    // ---- mensajes a través del motor ----

    fn talk_cfg(initial: Ms, rest: Ms, idle: Ms) -> EngineConfig {
        EngineConfig {
            timings: Timings {
                idle: (idle, idle),
                follow: (100, 100),
                rest: (rest, rest),
                reaction: (100, 100),
                rest_settle: 30,
                follow_cooldown: 600_000,
                reaction_cooldown: 600_000,
                weights: (0, 1, 0),
            },
            talker: TalkerConfig {
                enabled: true,
                cooldown: 600_000,
                initial_delay: (initial, initial),
                history_len: 6,
            },
            seed: Some(2),
            ..EngineConfig::default()
        }
    }

    #[test]
    fn engine_shows_and_clears_messages_and_emits_only_on_change() {
        let host = Fake::new();
        // Rest corto: al salir de Rest la frase de descanso se cancela
        let e = Engine::spawn(host.clone(), talk_cfg(0, 200, 30));
        sleep(Duration::from_millis(700));
        e.shutdown();
        let views = host.views.lock().unwrap();
        let with = views.iter().find(|v| v.message.is_some()).expect("debe aparecer un mensaje");
        let m = with.message.unwrap();
        let cat = CATALOG.iter().find(|c| c.id == m.id).expect("id del catálogo");
        assert_eq!(m.text, cat.text);
        assert_eq!(with.expression, cat.expression, "expresión coherente con la frase");
        assert_eq!(with.behavior, Behavior::Rest);
        // al terminar Rest el mensaje desaparece y la expresión vuelve a la de la FSM
        let idx = views.iter().position(|v| v.message.is_some()).unwrap();
        assert!(views[idx + 1..].iter().any(|v| v.message.is_none() && v.behavior == Behavior::Idle));
        for w in views.windows(2) {
            assert!(w[0].version < w[1].version, "versiones estrictamente crecientes");
            assert!(
                (w[0].behavior, w[0].expression, w[0].message, w[0].paused, w[0].hidden)
                    != (w[1].behavior, w[1].expression, w[1].message, w[1].paused, w[1].hidden),
                "evento duplicado sin cambio real: {:?}",
                w
            );
        }
        // cooldown largo: solo UN mensaje en toda la sesión
        let started = views.windows(2).filter(|w| w[0].message.is_none() && w[1].message.is_some()).count()
            + usize::from(views[0].message.is_some());
        assert_eq!(started, 1, "sin acumulación ni repetición durante el cooldown");
    }

    #[test]
    fn pause_cancels_the_visible_message_and_resume_does_not_bring_it_back() {
        let host = Fake::new();
        // Rest larguísimo: el mensaje (4–6 s) seguiría visible si nadie lo cancelara
        let e = Engine::spawn(host.clone(), talk_cfg(0, 600_000, 20));
        sleep(Duration::from_millis(250));
        assert!(e.view().message.is_some(), "mensaje visible al entrar en Rest");
        e.send(Cmd::Pause(true));
        sleep(Duration::from_millis(100));
        let v = e.view();
        assert!(v.paused && v.message.is_none(), "pausar cancela el mensaje");
        assert!(matches!(v.expression, Expression::Sleepy | Expression::Resting), "expresión de la FSM restaurada: {:?}", v.expression);
        e.send(Cmd::Pause(false));
        sleep(Duration::from_millis(300));
        let v2 = e.view();
        assert!(!v2.paused && v2.message.is_none(), "no reaparece tras reanudar");
        assert_eq!(host.calls(), 0);
        e.shutdown();
    }

    #[test]
    fn hiding_cancels_message_and_nothing_runs_while_hidden() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), talk_cfg(0, 600_000, 20));
        sleep(Duration::from_millis(250));
        assert!(e.view().message.is_some());
        e.send(Cmd::Hidden(true));
        sleep(Duration::from_millis(100));
        assert!(e.view().message.is_none() && e.view().hidden);
        assert!(!host.visible.load(Ordering::SeqCst));
        let n = host.views.lock().unwrap().len();
        sleep(Duration::from_millis(400));
        assert_eq!(host.views.lock().unwrap().len(), n, "oculto: ningún evento nuevo");
        e.shutdown();
    }

    #[test]
    fn disabled_messages_never_appear_but_behaviors_still_run() {
        let host = Fake::new();
        let mut cfg = talk_cfg(0, 200, 30);
        cfg.talker.enabled = false;
        let e = Engine::spawn(host.clone(), cfg);
        sleep(Duration::from_millis(500));
        e.shutdown();
        let views = host.views.lock().unwrap();
        assert!(views.iter().all(|v| v.message.is_none()));
        assert!(views.iter().any(|v| v.behavior == Behavior::Rest));
    }

    #[test]
    fn view_serializes_message_for_the_frontend() {
        let v = View {
            version: 7,
            behavior: Behavior::Rest,
            expression: Expression::Sleepy,
            paused: false,
            hidden: false,
            follow_enabled: true,
            message: Some(super::super::messages::MessageView { id: "rest.little", text: "Voy a descansar un ratito." }),
            suppressed: false,
            sleeping: false,
        };
        let j = serde_json::to_value(v).unwrap();
        assert_eq!(j["message"]["id"], "rest.little");
        assert_eq!(j["message"]["text"], "Voy a descansar un ratito.");
        assert_eq!(j["expression"], "sleepy");
        let none = serde_json::to_value(View { message: None, ..v }).unwrap();
        assert!(none["message"].is_null());
    }

    #[test]
    fn native_events_disable_the_wake_time_safety_net() {
        let run = |native: bool| {
            let host = Fake::new();
            let cfg = EngineConfig {
                timings: Timings { idle: (30, 30), weights: (0, 0, 0), ..Timings::default() },
                refresh_every: Duration::from_millis(5),
                seed: Some(1),
                ..EngineConfig::default()
            };
            let e = Engine::spawn(host.clone(), cfg);
            if native {
                e.send(Cmd::NativeGeometryEvents);
            }
            sleep(Duration::from_millis(100));
            let base = host.monitor_calls.load(Ordering::SeqCst);
            sleep(Duration::from_millis(400)); // ~13 despertares por plazo de Idle
            let extra = host.monitor_calls.load(Ordering::SeqCst) - base;
            e.shutdown();
            extra
        };
        assert!(run(false) > 3, "sin avisos nativos la red de seguridad revisa al despertar");
        assert_eq!(run(true), 0, "con avisos nativos no hay relecturas espontáneas");
    }

    // ---- controles de la bandeja sobre el motor ----

    fn quiet_follow_cfg() -> EngineConfig {
        // como follow_cfg pero con 300 ms de Idle antes de decidir: da tiempo a enviar órdenes
        EngineConfig {
            timings: Timings { idle: (300, 300), ..follow_cfg().timings },
            ..follow_cfg()
        }
    }

    #[test]
    fn show_and_hide_are_idempotent() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_cfg());
        sleep(Duration::from_millis(50));
        e.send(Cmd::Hidden(false)); // ya visible: no-op
        sleep(Duration::from_millis(60));
        assert_eq!(host.visible_calls.load(Ordering::SeqCst), 0, "mostrar estando visible no toca la ventana");
        for _ in 0..3 {
            e.send(Cmd::Hidden(true));
        }
        sleep(Duration::from_millis(80));
        assert_eq!(host.visible_calls.load(Ordering::SeqCst), 1, "ocultar x3 → una sola llamada");
        assert!(!host.visible.load(Ordering::SeqCst) && e.view().hidden);
        for _ in 0..3 {
            e.send(Cmd::Hidden(false));
        }
        sleep(Duration::from_millis(80));
        assert_eq!(host.visible_calls.load(Ordering::SeqCst), 2, "mostrar x3 → una sola llamada");
        assert!(host.visible.load(Ordering::SeqCst) && !e.view().hidden);
        e.shutdown();
    }

    #[test]
    fn showing_revalidates_geometry_even_without_a_display_event() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_cfg());
        sleep(Duration::from_millis(60));
        e.send(Cmd::Hidden(true));
        sleep(Duration::from_millis(60));
        // mientras está oculta cambia la pantalla y no llega ningún aviso (p. ej. se perdió)
        set_monitor(&host, Rect::new(0, 0, 1280, 720), 1.0);
        e.send(Cmd::Hidden(false));
        sleep(Duration::from_millis(150));
        let w = *host.win.lock().unwrap();
        assert!(w.x >= 0 && w.x + 260 <= 1280 && w.y >= 0 && w.y + 200 <= 720, "al mostrar debe quedar visible: {w:?}");
        e.shutdown();
    }

    #[test]
    fn pause_then_hide_then_show_stays_paused_and_does_no_work() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_follow_cfg());
        sleep(Duration::from_millis(60));
        e.send(Cmd::Pause(true));
        e.send(Cmd::Hidden(true));
        sleep(Duration::from_millis(80));
        let v = e.view();
        assert!(v.paused && v.hidden);
        e.send(Cmd::Hidden(false));
        sleep(Duration::from_millis(80));
        let v = e.view();
        assert!(v.paused && !v.hidden, "mostrar NO reanuda");
        assert!(host.visible.load(Ordering::SeqCst), "la ventana sí vuelve a verse");
        let (c, m) = (host.calls(), host.moves());
        sleep(Duration::from_millis(600)); // el Follow (a los 300 ms) no debe arrancar
        assert_eq!((host.calls(), host.moves()), (c, m), "pausada: sin cursor ni movimiento");
        e.send(Cmd::Pause(false));
        sleep(Duration::from_millis(500));
        assert!(host.calls() > c, "al reanudar vuelve la actividad (Follow)");
        e.shutdown();
    }

    #[test]
    fn hide_then_pause_then_resume_stays_hidden() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_follow_cfg());
        sleep(Duration::from_millis(50));
        e.send(Cmd::Hidden(true));
        e.send(Cmd::Pause(true));
        e.send(Cmd::Pause(false));
        sleep(Duration::from_millis(80));
        assert!(e.view().hidden && !e.view().paused);
        assert!(!host.visible.load(Ordering::SeqCst));
        let c = host.calls();
        sleep(Duration::from_millis(600));
        assert_eq!(host.calls(), c, "oculta: sin actividad aunque se reanude la pausa");
        e.shutdown();
    }

    #[test]
    fn toggles_act_on_the_real_state_even_when_fired_in_bursts() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_cfg());
        sleep(Duration::from_millis(40));
        assert!(e.view().follow_enabled && !e.view().paused);
        for _ in 0..3 {
            e.send(Cmd::TogglePause);
            e.send(Cmd::ToggleFollow);
        }
        sleep(Duration::from_millis(80));
        let v = e.view();
        assert!(v.paused, "3 alternancias de pausa → pausada");
        assert!(!v.follow_enabled, "3 alternancias de seguimiento → desactivado");
        e.send(Cmd::TogglePause);
        e.send(Cmd::ToggleFollow);
        sleep(Duration::from_millis(60));
        let v = e.view();
        assert!(!v.paused && v.follow_enabled);
        e.shutdown();
    }

    #[test]
    fn follow_disabled_via_toggle_while_paused_never_follows_after_resume() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_follow_cfg());
        sleep(Duration::from_millis(40));
        e.send(Cmd::TogglePause);
        e.send(Cmd::ToggleFollow);
        sleep(Duration::from_millis(60));
        let v = e.view();
        assert!(v.paused && !v.follow_enabled);
        e.send(Cmd::TogglePause);
        sleep(Duration::from_millis(900));
        assert_eq!(host.calls(), 0, "con el seguimiento desactivado no se consulta el cursor");
        e.shutdown();
    }

    #[test]
    fn follow_flag_changes_are_emitted_once_each() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), quiet_cfg());
        sleep(Duration::from_millis(40));
        e.send(Cmd::ToggleFollow);
        sleep(Duration::from_millis(50));
        e.send(Cmd::ToggleFollow);
        sleep(Duration::from_millis(50));
        e.shutdown();
        let views = host.views.lock().unwrap();
        assert_eq!(views.len(), 2, "un evento por cambio real: {views:?}");
        assert!(!views[0].follow_enabled && views[1].follow_enabled);
        assert!(views[0].version < views[1].version);
    }

    #[test]
    fn hiding_cancels_message_and_showing_does_not_resurrect_it_or_skip_the_cooldown() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), talk_cfg(0, 600_000, 20));
        sleep(Duration::from_millis(250));
        assert!(e.view().message.is_some());
        e.send(Cmd::Hidden(true));
        sleep(Duration::from_millis(80));
        assert!(e.view().message.is_none());
        e.send(Cmd::Hidden(false));
        sleep(Duration::from_millis(400));
        let v = e.view();
        assert!(v.message.is_none() && !v.hidden, "no reaparece al mostrar: el cooldown sigue corriendo");
        e.shutdown();
    }

    // ---- cierre ----

    #[test]
    fn request_stop_never_blocks_and_engine_stops() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), follow_cfg());
        sleep(Duration::from_millis(150));
        let t = Instant::now();
        e.request_stop();
        e.request_stop();
        assert!(t.elapsed() < Duration::from_millis(50), "no bloquea al hilo que lo pide");
        assert!(e.wait_stopped(Duration::from_secs(2)));
        assert!(e.shutdown_timeout(Duration::from_secs(1)), "idempotente tras parar");
        let before = (host.calls(), host.moves());
        e.send(Cmd::TogglePause); // descartada: cerrando
        sleep(Duration::from_millis(200));
        assert_eq!((host.calls(), host.moves()), before, "tras el cierre no hay más actividad");
    }

    #[test]
    fn stop_during_motion_ends_motion_promptly() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), follow_cfg());
        sleep(Duration::from_millis(200));
        assert!(host.moves() > 0, "en movimiento");
        let t = Instant::now();
        assert!(e.shutdown_timeout(Duration::from_secs(2)));
        assert!(t.elapsed() < Duration::from_millis(500));
        let m = host.moves();
        sleep(Duration::from_millis(250));
        assert_eq!(host.moves(), m);
    }

    #[test]
    fn wait_stopped_times_out_on_a_stuck_host_call_then_completes() {
        let host = Fake::new();
        *host.cursor_blocked.lock().unwrap() = true; // simula un getter esperando al hilo principal
        let e = Engine::spawn(host.clone(), follow_cfg());
        let t0 = Instant::now();
        while !host.in_cursor.load(Ordering::SeqCst) {
            assert!(t0.elapsed() < Duration::from_secs(3), "el motor debía llegar a consultar el cursor");
            sleep(Duration::from_millis(5));
        }
        e.request_stop();
        let t = Instant::now();
        assert!(!e.wait_stopped(Duration::from_millis(150)), "atascado: la espera acotada devuelve false");
        assert!(t.elapsed() < Duration::from_millis(600), "sin colgarse: {:?}", t.elapsed());
        assert!(!e.shutdown_timeout(Duration::from_millis(50)), "tampoco hace join de un hilo atascado");
        // se libera la llamada del sistema → el motor termina por sí solo
        *host.cursor_blocked.lock().unwrap() = false;
        host.cursor_cv.notify_all();
        assert!(e.wait_stopped(Duration::from_secs(2)));
        assert!(e.shutdown_timeout(Duration::from_secs(1)), "ahora sí hace join");
    }

    #[test]
    fn concurrent_shutdowns_are_safe() {
        let host = Fake::new();
        let e = Arc::new(Engine::spawn(host.clone(), follow_cfg()));
        sleep(Duration::from_millis(100));
        let hs: Vec<_> = (0..6)
            .map(|_| {
                let e = Arc::clone(&e);
                std::thread::spawn(move || e.shutdown_timeout(Duration::from_secs(2)))
            })
            .collect();
        for h in hs {
            assert!(h.join().expect("sin pánico"));
        }
        assert!(e.wait_stopped(Duration::from_millis(10)));
    }

    // ---- estado inicial desde preferencias persistidas ----

    #[test]
    fn engine_starts_paused_when_the_preferences_say_so_and_resumes_cleanly() {
        let host = Fake::new();
        let cfg = EngineConfig { initial_paused: true, ..quiet_follow_cfg() };
        let e = Engine::spawn(host.clone(), cfg);
        assert!(e.view().paused && !e.view().hidden, "pausada pero visible");
        sleep(Duration::from_millis(700)); // el Follow (a los 300 ms) NO debe arrancar
        assert_eq!((host.calls(), host.moves()), (0, 0), "pausada desde el arranque: sin actividad");
        assert!(host.views.lock().unwrap().is_empty(), "el estado inicial no se emite: se consulta con get_view");
        e.send(Cmd::TogglePause);
        sleep(Duration::from_millis(500));
        assert!(!e.view().paused && host.calls() > 0, "reanuda desde el estado persistido");
        e.shutdown();
    }

    #[test]
    fn engine_starts_with_follow_disabled_from_preferences() {
        let host = Fake::new();
        let cfg = EngineConfig { initial_follow: false, ..quiet_follow_cfg() };
        let e = Engine::spawn(host.clone(), cfg);
        assert!(!e.view().follow_enabled);
        sleep(Duration::from_millis(900));
        assert_eq!(host.calls(), 0, "sin Follow no se consulta el cursor");
        e.shutdown();
    }

    #[test]
    fn restored_pause_blocks_spontaneous_messages_until_resume() {
        let host = Fake::new();
        let cfg = EngineConfig { initial_paused: true, ..talk_cfg(0, 200, 30) };
        let e = Engine::spawn(host.clone(), cfg);
        sleep(Duration::from_millis(500));
        assert!(e.view().message.is_none() && host.views.lock().unwrap().is_empty());
        e.shutdown();
    }

    // ---- cierre en cada situación (revisión de ciclo de vida) ----

    fn stops_cleanly(cfg: EngineConfig, setup: impl FnOnce(&Engine, &Fake), warmup_ms: u64) {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), cfg);
        setup(&e, &host);
        sleep(Duration::from_millis(warmup_ms));
        let t = Instant::now();
        e.request_stop();
        assert!(t.elapsed() < Duration::from_millis(50), "request_stop no bloquea");
        assert!(e.wait_stopped(Duration::from_secs(2)), "el motor termina");
        assert!(e.shutdown_timeout(Duration::from_secs(1)), "y se une (join)");
        let before = (host.calls(), host.moves(), host.views.lock().unwrap().len(), host.visible_calls.load(Ordering::SeqCst));
        sleep(Duration::from_millis(250));
        let after = (host.calls(), host.moves(), host.views.lock().unwrap().len(), host.visible_calls.load(Ordering::SeqCst));
        assert_eq!(before, after, "tras el cierre no hay ninguna actividad del sistema");
    }

    #[test]
    fn stop_while_following() {
        stops_cleanly(follow_cfg(), |_, h| assert!(h.calls() < 1000), 250);
    }

    #[test]
    fn stop_during_a_reaction() {
        let cfg = EngineConfig {
            timings: Timings { reaction: (60_000, 60_000), weights: (0, 0, 1), idle: (20, 20), reaction_cooldown: 0, ..follow_cfg().timings },
            talker: TalkerConfig { enabled: false, ..TalkerConfig::default() },
            seed: Some(4),
            ..EngineConfig::default()
        };
        stops_cleanly(cfg, |e, _| { let _ = e; }, 200);
    }

    #[test]
    fn stop_with_a_visible_message() {
        let cfg = talk_cfg(0, 600_000, 20);
        stops_cleanly(cfg, |_, _| {}, 250);
    }

    #[test]
    fn stop_while_paused() {
        stops_cleanly(quiet_follow_cfg(), |e, _| e.send(Cmd::Pause(true)), 100);
    }

    #[test]
    fn stop_while_hidden() {
        stops_cleanly(quiet_follow_cfg(), |e, _| e.send(Cmd::Hidden(true)), 100);
    }

    #[test]
    fn stop_while_paused_and_hidden_with_a_pending_geometry_notice() {
        stops_cleanly(
            quiet_follow_cfg(),
            |e, _| {
                e.send(Cmd::Pause(true));
                e.send(Cmd::Hidden(true));
                e.send(Cmd::GeometryChanged);
            },
            100,
        );
    }

    #[test]
    fn stop_requested_only_once_is_honored_even_if_asked_many_times_concurrently() {
        let host = Fake::new();
        let e = Arc::new(Engine::spawn(host, follow_cfg()));
        sleep(Duration::from_millis(100));
        let hs: Vec<_> = (0..8)
            .map(|i| {
                let e = Arc::clone(&e);
                std::thread::spawn(move || if i % 2 == 0 { e.request_stop() } else { let _ = e.shutdown_timeout(Duration::from_secs(2)); })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert!(e.wait_stopped(Duration::from_secs(1)));
    }

    // ---- Reloj inyectado (E1a): el motor mide el tiempo con `Clock`, no con `Instant` ----

    fn wait_until(mut cond: impl FnMut() -> bool) -> bool {
        for _ in 0..100 {
            if cond() {
                return true;
            }
            sleep(Duration::from_millis(20));
        }
        cond()
    }

    #[test]
    fn frozen_fake_clock_keeps_the_fsm_waiting_even_as_real_time_passes() {
        let host = Fake::new();
        let clock = Arc::new(super::super::clock::FakeClock::new(0));
        let e = Engine::spawn_with_clock(host.clone(), follow_cfg(), clock.clone());
        sleep(Duration::from_millis(300)); // 15 veces el plazo de Idle (20 ms) en tiempo real
        assert_eq!(e.view().behavior, Behavior::Idle, "con el reloj parado no vence ningún plazo");
        assert_eq!(host.calls(), 0);
        clock.advance(30); // ahora sí vence el plazo de Idle
        assert!(wait_until(|| e.view().behavior == Behavior::Follow), "con el reloj avanzado, pasa a Follow");
        e.shutdown();
    }

    #[test]
    fn geometry_debounce_follows_the_injected_clock() {
        let host = Fake::new();
        let clock = Arc::new(super::super::clock::FakeClock::new(0));
        let e = Engine::spawn_with_clock(host.clone(), quiet_cfg(), clock.clone());
        sleep(Duration::from_millis(100));
        e.send(Cmd::GeometryChanged);
        sleep(Duration::from_millis(100));
        let before = host.monitor_calls.load(Ordering::SeqCst);
        sleep(Duration::from_millis(600)); // el doble del debounce (300 ms) en tiempo real
        assert_eq!(host.monitor_calls.load(Ordering::SeqCst), before, "reloj parado: sigue pendiente");
        clock.advance(300);
        assert!(
            wait_until(|| host.monitor_calls.load(Ordering::SeqCst) > before),
            "al cumplirse el debounce según el reloj, relee los monitores"
        );
        e.shutdown();
    }

    #[test]
    fn spawn_uses_the_system_clock() {
        // `spawn` (producción) sigue funcionando por tiempo real: Idle de 20 ms → Follow.
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), follow_cfg());
        assert!(wait_until(|| e.view().behavior == Behavior::Follow));
        e.shutdown();
    }

    // ---- E1b: supresión (alcanzable solo desde pruebas y diag) ----

    #[test]
    fn suppression_hides_and_restores_the_window_through_the_engine() {
        let host = Fake::new();
        let clock = Arc::new(super::super::clock::FakeClock::new(0));
        let e = Engine::spawn_with_clock(host.clone(), quiet_cfg(), clock);
        sleep(Duration::from_millis(60));
        e.send(Cmd::Suppressed(true));
        assert!(wait_until(|| !host.visible.load(Ordering::SeqCst)), "suprimida: ventana oculta");
        assert!(e.view().suppressed && !e.view().hidden, "hidden conserva su significado de v0.1.0");
        e.send(Cmd::Suppressed(false));
        assert!(wait_until(|| host.visible.load(Ordering::SeqCst)), "fin del episodio: vuelve a mostrarse");
        assert!(!e.view().suppressed);
        e.shutdown();
    }

    #[test]
    fn show_anyway_overrides_the_episode_and_the_next_episode_suppresses_again() {
        let host = Fake::new();
        let clock = Arc::new(super::super::clock::FakeClock::new(0));
        let e = Engine::spawn_with_clock(host.clone(), quiet_cfg(), clock);
        e.send(Cmd::Suppressed(true));
        assert!(wait_until(|| !host.visible.load(Ordering::SeqCst)));
        e.send(Cmd::DndOverride);
        assert!(wait_until(|| host.visible.load(Ordering::SeqCst)), "anulada: se muestra");
        assert!(!e.view().suppressed, "la vista ya no marca supresión en vigor");
        let calls = host.visible_calls.load(Ordering::SeqCst);
        e.send(Cmd::Suppressed(false)); // fin del episodio: limpia la anulación, ya estaba visible
        sleep(Duration::from_millis(80));
        assert_eq!(host.visible_calls.load(Ordering::SeqCst), calls, "sin llamadas de ventana redundantes");
        e.send(Cmd::Suppressed(true));
        assert!(wait_until(|| !host.visible.load(Ordering::SeqCst)), "episodio nuevo: vuelve a suprimir");
        e.shutdown();
    }

    #[test]
    fn show_with_hidden_and_active_suppression_overrides_and_keeps_pause() {
        let host = Fake::new();
        let clock = Arc::new(super::super::clock::FakeClock::new(0));
        let e = Engine::spawn_with_clock(host.clone(), quiet_cfg(), clock);
        e.send(Cmd::Pause(true));
        e.send(Cmd::Hidden(true));
        e.send(Cmd::Suppressed(true));
        assert!(wait_until(|| e.view().hidden && e.view().suppressed));
        e.send(Cmd::Hidden(false)); // "Mostrar"
        assert!(wait_until(|| host.visible.load(Ordering::SeqCst)));
        let v = e.view();
        assert!(!v.hidden && v.paused && !v.suppressed, "quita hidden, anula el episodio y no toca paused: {v:?}");
        e.shutdown();
    }

    #[test]
    fn suppression_commands_without_an_episode_change_nothing_and_emit_nothing() {
        let host = Fake::new();
        let clock = Arc::new(super::super::clock::FakeClock::new(0));
        let e = Engine::spawn_with_clock(host.clone(), quiet_cfg(), clock);
        let before = e.view();
        e.send(Cmd::DndOverride); // 8b: no existe sin supresión
        e.send(Cmd::Suppressed(false));
        sleep(Duration::from_millis(150));
        assert_eq!(e.view(), before);
        assert!(host.views.lock().unwrap().is_empty(), "same_view evita emisiones extra");
        assert_eq!(host.visible_calls.load(Ordering::SeqCst), 0);
        e.shutdown();
    }

    #[test]
    fn suppression_cancels_the_visible_message_and_it_does_not_come_back() {
        let host = Fake::new();
        let e = Engine::spawn(host.clone(), talk_cfg(0, 600_000, 20));
        sleep(Duration::from_millis(250));
        assert!(e.view().message.is_some(), "mensaje visible al entrar en Rest");
        e.send(Cmd::Suppressed(true));
        assert!(wait_until(|| e.view().message.is_none()), "suprimir cancela el mensaje");
        e.send(Cmd::Suppressed(false));
        sleep(Duration::from_millis(100));
        assert!(e.view().message.is_none(), "no vuelve por sí solo");
        e.shutdown();
    }

    #[test]
    fn a_suppressed_engine_does_not_wake_up_by_itself() {
        // plazos de Idle muy cortos: si siguiera corriendo, decidiría algo y pediría el cursor
        let host = Fake::new();
        let clock = Arc::new(super::super::clock::FakeClock::new(0));
        let e = Engine::spawn_with_clock(host.clone(), follow_cfg(), clock.clone());
        e.send(Cmd::Suppressed(true));
        assert!(wait_until(|| e.view().suppressed));
        let views = host.views.lock().unwrap().len();
        clock.advance(10_000);
        sleep(Duration::from_millis(300));
        assert_eq!(host.views.lock().unwrap().len(), views, "suprimida: sin plazos, sin vistas nuevas");
        assert_eq!(host.calls(), 0, "y sin consultar el cursor");
        e.shutdown();
    }

    // ---- E4a: `next_deadline` y `Worker::tick`, sin hilos ni esperas reales (FakeClock) ----

    use super::super::clock::FakeClock;

    fn worker(cfg: EngineConfig) -> (Worker<Fake>, Arc<FakeClock>, Arc<Fake>) {
        let host = Fake::new();
        let clock = Arc::new(FakeClock::new(0));
        let mut rng = match cfg.seed {
            Some(s) => fastrand::Rng::with_seed(s),
            None => fastrand::Rng::new(),
        };
        let fsm = Fsm::new(cfg.timings, 0, &mut rng);
        let talker = Talker::new(cfg.talker, 0, &mut rng);
        let mut w = Worker::new(host.clone(), cfg, fsm, talker, rng, clock.clone());
        w.refresh(true);
        (w, clock, host)
    }

    #[test]
    fn next_deadline_is_none_when_hidden_paused_or_suppressed_without_override() {
        for (name, cmd) in [("oculta", Cmd::Hidden(true)), ("pausada", Cmd::Pause(true)), ("suprimida", Cmd::Suppressed(true))] {
            let (mut w, _c, _h) = worker(quiet_cfg());
            assert_eq!(next_deadline(&w, 0), Some(600_000), "en marcha: plazo de Idle");
            w.tick(0, vec![cmd]);
            assert_eq!(next_deadline(&w, 0), None, "{name}: dormir indefinidamente");
            assert_eq!(w.wait_timeout(), None, "{name}");
        }
        // suprimida pero con anulación: vuelve a correr
        let (mut w, _c, _h) = worker(quiet_cfg());
        w.tick(0, vec![Cmd::Suppressed(true)]);
        assert_eq!(next_deadline(&w, 0), None);
        w.tick(0, vec![Cmd::DndOverride]);
        assert_eq!(next_deadline(&w, 0), Some(600_000), "anulada: corre otra vez");
        w.tick(0, vec![Cmd::Suppressed(false)]);
        assert_eq!(next_deadline(&w, 0), Some(600_000));
    }

    #[test]
    fn next_deadline_only_counts_the_geometry_debounce_while_suspended() {
        let (mut w, clock, _h) = worker(quiet_cfg());
        clock.set(100);
        w.tick(100, vec![Cmd::Hidden(true), Cmd::GeometryChanged]);
        assert_eq!(next_deadline(&w, 100), Some(400), "oculta: solo el aviso de geometría");
        assert_eq!(w.wait_timeout(), Some(Duration::from_millis(300)));
        clock.set(400);
        w.tick(400, vec![]);
        assert_eq!(next_deadline(&w, 400), None, "procesado el aviso, vuelve a dormir indefinidamente");
    }

    #[test]
    fn next_deadline_minimum_moves_as_the_clock_advances() {
        let (mut w, clock, _h) = worker(quiet_cfg());
        assert_eq!(next_deadline(&w, 0), Some(600_000));
        clock.set(100);
        w.tick(100, vec![Cmd::GeometryChanged]); // debounce de 300 ms → vence en 400
        assert_eq!(next_deadline(&w, 100), Some(400), "el debounce es el mínimo");
        assert_eq!(w.wait_timeout(), Some(Duration::from_millis(300)));
        clock.set(250);
        assert_eq!(next_deadline(&w, 250), Some(400), "el plazo es absoluto: no se desplaza");
        assert_eq!(w.wait_timeout(), Some(Duration::from_millis(150)), "la espera sí se acorta");
        clock.set(400);
        w.tick(400, vec![]);
        assert_eq!(next_deadline(&w, 400), Some(600_000), "vencido el debounce, vuelve el plazo de Idle");
        // en movimiento: cadencia de 30 Hz (33,3 ms → 34 ms, redondeo hacia arriba)
        w.moving = true;
        assert_eq!(next_deadline(&w, 400), Some(434));
        assert_eq!(w.wait_timeout(), Some(Duration::from_millis(34)));
        w.moving = false;
    }

    #[test]
    fn next_deadline_follows_the_fsm_deadline_and_the_cursor_cadence_in_follow() {
        let (mut w, clock, _h) = worker(follow_cfg());
        let d0 = next_deadline(&w, 0).unwrap();
        assert_eq!(d0, 20, "Idle de 20 ms");
        clock.set(30);
        w.tick(30, vec![]);
        assert_eq!(w.fsm.behavior(), Behavior::Follow);
        let d = next_deadline(&w, 30).unwrap();
        assert!(d <= 30 + 34, "en Follow se vigila el cursor como mucho cada ~34 ms: {d}");
        assert!(d > 30);
    }

    #[test]
    fn asleep_the_only_deadline_is_the_sleepy_to_resting_step() {
        let (mut w, clock, host) = worker(quiet_cfg());
        clock.set(1000);
        w.fsm.set_sleeping(1000, true);
        assert_eq!(next_deadline(&w, 1000), Some(5000), "sleepy → resting a los rest_settle (4 s)");
        assert_eq!(w.fsm.view().expression, super::super::state::Expression::Sleepy);
        clock.set(5000);
        w.tick(5000, vec![]);
        assert_eq!(w.fsm.view().expression, super::super::state::Expression::Resting);
        assert_eq!(w.fsm.next_wakeup(), None, "la FSM ya no tiene plazos");
        assert!(!host.views.lock().unwrap().is_empty(), "el paso se emitió");
        // Rest conserva el movimiento hacia su rincón (cadencia de 34 ms) hasta llegar; después, nada.
        let mut now = 5000;
        while let Some(d) = next_deadline(&w, now) {
            assert!(d <= now + 34, "solo la cadencia del movimiento: {d}");
            now = d;
            clock.set(now);
            w.tick(now, vec![]);
            assert!(now < 60_000, "el movimiento debe terminar");
        }
        clock.set(5_000_000);
        w.tick(5_000_000, vec![]);
        assert_eq!(next_deadline(&w, 5_000_000), None, "dormir no vence por sí solo");
    }

    /// Avanza el reloj de plazo en plazo (`next_deadline`) durante `total_ms` sin actividad y devuelve la
    /// secuencia de vistas emitidas y el número de ticks.
    fn run_idle_scenario(total_ms: Ms) -> (Vec<View>, usize) {
        let cfg = EngineConfig { seed: Some(7), talker: TalkerConfig::default(), ..EngineConfig::default() };
        let (mut w, clock, host) = worker(cfg);
        let mut now = 0;
        let mut ticks = 0;
        while now < total_ms {
            let next = next_deadline(&w, now).map_or(total_ms, |d| d.min(total_ms));
            assert!(next >= now, "el plazo nunca queda en el pasado tras un tick: {next} < {now}");
            now = next;
            clock.set(now);
            w.tick(now, vec![]);
            ticks += 1;
            assert!(ticks < 400_000, "el escenario no avanza");
        }
        let views = host.views.lock().unwrap().clone();
        (views, ticks)
    }

    #[test]
    fn ten_minutes_without_activity_run_in_no_real_time_and_are_deterministic() {
        let t0 = Instant::now();
        let (a, ticks) = run_idle_scenario(600_000);
        assert!(t0.elapsed() < Duration::from_secs(5), "sin esperas reales: {:?}", t0.elapsed());
        let (b, ticks_b) = run_idle_scenario(600_000);
        assert_eq!(ticks, ticks_b);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert!(same_view(x, y) && x.version == y.version, "misma semilla, misma secuencia");
        }
        assert!(!a.is_empty(), "en 10 minutos algo cambia");
        assert!(a.iter().any(|v| v.behavior != Behavior::Idle), "se visita algún comportamiento distinto de Idle");
        assert!(a.windows(2).all(|p| p[0].version <= p[1].version), "la versión nunca retrocede");
    }

    // ---- E4b: sondeo de No molestar (FakeProbe + FakeClock, sin hilos ni esperas reales) ----

    use super::super::probe::{FakeProbe, NotifState, ProbeError};

    type DndWorker = (Worker<Fake>, Arc<FakeClock>, Arc<Fake>, Arc<FakeProbe>);

    fn dnd_worker_with(cfg: EngineConfig) -> DndWorker {
        let (mut w, clock, host) = worker(EngineConfig { dnd_auto: true, ..cfg });
        let probe = Arc::new(FakeProbe::new());
        w.probe = Some(probe.clone());
        (w, clock, host, probe)
    }

    fn dnd_worker() -> DndWorker {
        dnd_worker_with(quiet_cfg())
    }

    /// Avanza el reloj a `t` y da un paso del motor (sin órdenes).
    fn at(w: &mut Worker<Fake>, clock: &FakeClock, t: Ms) {
        clock.set(t);
        w.tick(t, vec![]);
    }

    fn at_with(w: &mut Worker<Fake>, clock: &FakeClock, t: Ms, cmds: Vec<Cmd>) {
        clock.set(t);
        w.tick(t, cmds);
    }

    fn busy() -> Result<NotifState, ProbeError> {
        Ok(NotifState::Busy)
    }
    fn calm() -> Result<NotifState, ProbeError> {
        Ok(NotifState::AcceptsNotifications)
    }

    #[test]
    fn the_first_poll_is_immediate_and_then_every_interval_inside_the_single_scheduler() {
        let (mut w, clock, _h, probe) = dnd_worker();
        at(&mut w, &clock, 0);
        assert_eq!(probe.notification_calls(), 1, "lectura inmediata al abrirse la puerta");
        assert_eq!(next_deadline(&w, 0), Some(2_500), "el sondeo es el mínimo (Idle vence a los 600 s)");
        assert_eq!(w.wait_timeout(), Some(Duration::from_millis(2_500)));
        at(&mut w, &clock, 1_000);
        assert_eq!(probe.notification_calls(), 1, "antes del plazo no se vuelve a leer");
        at(&mut w, &clock, 2_500);
        assert_eq!(probe.notification_calls(), 2);
        assert_eq!(next_deadline(&w, 2_500), Some(5_000));
    }

    #[test]
    fn without_a_probe_or_with_the_switch_off_nothing_is_polled() {
        let (mut w, clock, _h) = worker(EngineConfig { dnd_auto: true, ..quiet_cfg() }); // sin sonda
        at(&mut w, &clock, 0);
        assert_eq!(next_deadline(&w, 0), Some(600_000));
        let (mut w, clock, _h, probe) = dnd_worker_with(EngineConfig { dnd_auto: false, ..quiet_cfg() });
        w.dnd.enabled = false;
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 10_000);
        assert_eq!(probe.notification_calls(), 0);
        assert_eq!(next_deadline(&w, 10_000), Some(600_000));
    }

    #[test]
    fn two_consecutive_busy_reads_suppress_and_hide_the_window() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        assert!(!w.fsm.view().suppressed, "una lectura no basta");
        assert!(host.visible.load(Ordering::SeqCst));
        at(&mut w, &clock, 2_500);
        let v = w.fsm.view();
        assert!(v.suppressed && !v.hidden && !v.paused, "hidden/paused conservan su significado");
        assert!(!host.visible.load(Ordering::SeqCst), "ventana oculta");
        assert!(host.views.lock().unwrap().last().unwrap().suppressed, "la vista emitida lo refleja");
        // suprimida: la FSM está suspendida pero el sondeo sigue (recuperación)
        assert!(w.fsm.is_suspended());
        assert_eq!(next_deadline(&w, 2_500), Some(5_000));
    }

    #[test]
    fn two_consecutive_calm_reads_end_the_episode_and_show_the_window_again() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 2_500);
        probe.set_notification_state(calm());
        at(&mut w, &clock, 5_000);
        assert!(w.fsm.view().suppressed, "una lectura negativa no basta");
        assert!(!host.visible.load(Ordering::SeqCst));
        at(&mut w, &clock, 7_500);
        assert!(!w.fsm.view().suppressed);
        assert!(host.visible.load(Ordering::SeqCst), "reaparece");
    }

    #[test]
    fn states_that_do_not_suppress_never_hide_the_window() {
        for st in [NotifState::NotPresent, NotifState::QuietTime, NotifState::App, NotifState::Other(9)] {
            let (mut w, clock, host, probe) = dnd_worker();
            probe.set_notification_state(Ok(st));
            for k in 0..8 {
                at(&mut w, &clock, k * 2_500);
            }
            assert!(!w.fsm.view().suppressed && host.visible.load(Ordering::SeqCst), "{st:?}");
            assert_eq!(probe.notification_calls(), 8, "{st:?}: se sigue sondeando");
        }
    }

    #[test]
    fn a_probe_error_never_suppresses_and_two_errors_do_not_end_an_episode() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(Err(ProbeError::CallFailed));
        for k in 0..6 {
            at(&mut w, &clock, k * 2_500);
        }
        assert!(!w.fsm.view().suppressed && host.visible.load(Ordering::SeqCst), "errores: nunca se suprime");
        // episodio real; luego dos errores seguidos y una lectura válida positiva: sigue suprimida
        probe.set_notification_state(busy());
        at(&mut w, &clock, 15_000);
        at(&mut w, &clock, 17_500);
        assert!(w.fsm.view().suppressed);
        probe.set_notification_state(Err(ProbeError::CallFailed));
        at(&mut w, &clock, 20_000);
        at(&mut w, &clock, 22_500);
        assert!(w.fsm.view().suppressed, "dos errores no terminan el episodio");
        probe.set_notification_state(busy());
        at(&mut w, &clock, 25_000);
        assert!(w.fsm.view().suppressed);
    }

    /// Caso 10 de §5.2: 3 errores consecutivos durante un episodio lo terminan (falla hacia visible).
    #[test]
    fn three_consecutive_probe_errors_end_the_episode_and_make_the_window_visible() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 2_500);
        assert!(!host.visible.load(Ordering::SeqCst));
        probe.set_notification_state(Err(ProbeError::CallFailed));
        at(&mut w, &clock, 5_000);
        at(&mut w, &clock, 7_500);
        assert!(w.fsm.view().suppressed);
        at(&mut w, &clock, 10_000);
        assert!(!w.fsm.view().suppressed && host.visible.load(Ordering::SeqCst), "tercer error: termina y reaparece");
        // y si la sonda vuelve a dar Busy, hacen falta de nuevo dos lecturas
        probe.set_notification_state(busy());
        at(&mut w, &clock, 12_500);
        assert!(!w.fsm.view().suppressed);
        at(&mut w, &clock, 15_000);
        assert!(w.fsm.view().suppressed);
    }

    #[test]
    fn a_state_change_during_the_geometry_debounce_is_handled_without_losing_either() {
        let cfg = EngineConfig { dnd_poll_ms: 100, ..quiet_cfg() };
        let (mut w, clock, host, probe) = dnd_worker_with(cfg);
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0); // lectura 1
        at_with(&mut w, &clock, 100, vec![Cmd::GeometryChanged]); // lectura 2 → supresión; debounce vence a los 400
        assert!(w.fsm.view().suppressed);
        assert_eq!(next_deadline(&w, 100), Some(200), "mínimo entre el sondeo (200) y el debounce (400)");
        probe.set_notification_state(calm());
        at(&mut w, &clock, 200);
        at(&mut w, &clock, 300); // dos lecturas tranquilas DENTRO del debounce → reaparece
        assert!(!w.fsm.view().suppressed && host.visible.load(Ordering::SeqCst));
        assert!(w.geom_due.is_some(), "el aviso de geometría sigue pendiente");
        let before = host.monitor_calls.load(Ordering::SeqCst);
        at(&mut w, &clock, 400);
        assert!(w.geom_due.is_none(), "el debounce se procesa a su hora");
        assert!(host.monitor_calls.load(Ordering::SeqCst) > before, "y relee los monitores");
        assert!(!w.fsm.view().suppressed);
    }

    #[test]
    fn show_anyway_overrides_only_the_current_episode_and_the_next_one_suppresses_again() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 2_500);
        assert!(!host.visible.load(Ordering::SeqCst));
        at_with(&mut w, &clock, 3_000, vec![Cmd::DndOverride]);
        assert!(host.visible.load(Ordering::SeqCst), "anulada: visible");
        assert!(!w.fsm.view().suppressed, "ya no hay supresión en vigor");
        // el juego sigue en pantalla: más lecturas positivas no vuelven a ocultarla durante este episodio
        at(&mut w, &clock, 5_000);
        at(&mut w, &clock, 7_500);
        assert!(host.visible.load(Ordering::SeqCst));
        assert!(w.dnd.det.is_suppressing(), "el detector sigue viendo el episodio");
        // fin del episodio: se limpia la anulación
        probe.set_notification_state(calm());
        at(&mut w, &clock, 10_000);
        at(&mut w, &clock, 12_500);
        assert!(!w.dnd.det.is_suppressing() && !w.fsm.is_suppression_episode());
        // episodio nuevo: vuelve a suprimir
        probe.set_notification_state(busy());
        at(&mut w, &clock, 15_000);
        at(&mut w, &clock, 17_500);
        assert!(w.fsm.view().suppressed && !host.visible.load(Ordering::SeqCst), "la anulación no sobrevivió al episodio");
    }

    #[test]
    fn hiding_or_pausing_by_hand_stops_the_polling_completely() {
        for cmd in [Cmd::Hidden(true), Cmd::Pause(true), Cmd::TogglePause] {
            let (mut w, clock, _h, probe) = dnd_worker();
            at(&mut w, &clock, 0);
            let calls = probe.notification_calls();
            at_with(&mut w, &clock, 1_000, vec![cmd]);
            assert_eq!(next_deadline(&w, 1_000), None, "{cmd:?}: ningún plazo de sondeo");
            assert_eq!(w.wait_timeout(), None, "{cmd:?}: el hilo duerme indefinidamente");
            for k in 1..20 {
                at(&mut w, &clock, 1_000 + k * 60_000);
            }
            assert_eq!(probe.notification_calls(), calls, "{cmd:?}: cero lecturas mientras dure");
        }
    }

    #[test]
    fn showing_or_resuming_restarts_the_polling_with_an_immediate_read_and_fresh_streaks() {
        let (mut w, clock, _h, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0); // lectura positiva 1
        at_with(&mut w, &clock, 100, vec![Cmd::Pause(true)]);
        let calls = probe.notification_calls();
        at_with(&mut w, &clock, 50_000, vec![Cmd::Pause(false)]);
        assert_eq!(probe.notification_calls(), calls + 1, "lectura inmediata al reanudar");
        assert!(!w.fsm.view().suppressed, "la lectura anterior a la pausa no cuenta: hace falta una segunda");
        at(&mut w, &clock, 52_500);
        assert!(w.fsm.view().suppressed);
    }

    #[test]
    fn hiding_by_hand_during_an_episode_then_showing_overrides_it_and_paused_stays() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 2_500);
        at_with(&mut w, &clock, 3_000, vec![Cmd::Pause(true), Cmd::Hidden(true)]);
        let calls = probe.notification_calls();
        at(&mut w, &clock, 60_000);
        assert_eq!(probe.notification_calls(), calls, "oculta y pausada: sin sondeo aunque haya episodio");
        at_with(&mut w, &clock, 61_000, vec![Cmd::Hidden(false)]);
        let v = w.fsm.view();
        assert!(v.paused && !v.hidden, "Mostrar no quita la pausa");
        assert!(host.visible.load(Ordering::SeqCst), "Mostrar con supresión en vigor la anula (caso 8)");
        assert!(!v.suppressed);
    }

    #[test]
    fn turning_the_switch_off_ends_any_episode_immediately_and_stops_polling() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 2_500);
        assert!(!host.visible.load(Ordering::SeqCst));
        let calls = probe.notification_calls();
        at_with(&mut w, &clock, 3_000, vec![Cmd::DndAuto(false)]);
        assert!(!w.fsm.view().suppressed && host.visible.load(Ordering::SeqCst), "caso 9: suppressed=false inmediato");
        assert!(!w.dnd.det.is_suppressing());
        assert_eq!(next_deadline(&w, 3_000), Some(600_500), "sin sondeo: solo el plazo de Idle (retomado: 500 ms de supresión)");
        for k in 1..10 {
            at(&mut w, &clock, 3_000 + k * 2_500);
        }
        assert_eq!(probe.notification_calls(), calls, "apagado: cero lecturas");
        // encenderlo de nuevo: lectura inmediata y cadencia normal
        at_with(&mut w, &clock, 40_000, vec![Cmd::DndAuto(true)]);
        assert_eq!(probe.notification_calls(), calls + 1);
        assert_eq!(next_deadline(&w, 40_000), Some(42_500));
    }

    #[test]
    fn turning_the_switch_off_also_clears_an_override() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 2_500);
        at_with(&mut w, &clock, 3_000, vec![Cmd::DndOverride]);
        at_with(&mut w, &clock, 3_500, vec![Cmd::DndAuto(false)]);
        at_with(&mut w, &clock, 4_000, vec![Cmd::DndAuto(true)]);
        // activado otra vez con el juego aún en pantalla: se necesitan de nuevo dos lecturas y vuelve a suprimir
        at(&mut w, &clock, 6_500);
        assert!(w.fsm.view().suppressed && !host.visible.load(Ordering::SeqCst), "la anulación anterior no se arrastra");
    }

    #[test]
    fn a_diag_injected_suppression_is_not_ended_by_the_real_probe_but_the_switch_ends_it() {
        let (mut w, clock, host, probe) = dnd_worker();
        probe.set_notification_state(calm());
        at_with(&mut w, &clock, 0, vec![Cmd::Suppressed(true)]);
        for k in 1..6 {
            at(&mut w, &clock, k * 2_500);
        }
        assert!(w.fsm.view().suppressed, "el detector no abrió el episodio, así que no lo cierra");
        assert!(probe.notification_calls() >= 6, "pero sí sondea mientras está suprimida (coste medible)");
        at_with(&mut w, &clock, 20_000, vec![Cmd::DndAuto(false)]);
        assert!(!w.fsm.view().suppressed && host.visible.load(Ordering::SeqCst));
    }

    #[test]
    fn after_a_long_suppression_the_frozen_deadlines_resume_instead_of_firing_at_once() {
        // plazos por defecto (Idle 20–45 s) y mensajes activados
        let cfg = EngineConfig { seed: Some(3), ..EngineConfig::default() };
        let (mut w, clock, host, probe) = dnd_worker_with(cfg);
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0); // lectura positiva 1
        let before = w.fsm.next_wakeup().expect("hay un plazo de Idle");
        assert!(before > 2_500, "el plazo de Idle es de decenas de segundos: {before}");
        // supresión a partir de t=2,5 s; Idle habría vencido mucho antes de 10 minutos
        at(&mut w, &clock, 2_500); // lectura positiva 2
        assert!(w.fsm.view().suppressed);
        let rem = before - 2_500;
        let (b0, e0, v0) = (w.fsm.behavior(), w.fsm.view().expression, w.fsm.view().version);
        // 10 minutos suprimida: el sondeo corre y NADA más cambia
        let mut t = 2_500;
        while t < 600_000 {
            t += 2_500;
            at(&mut w, &clock, t);
        }
        assert!(w.fsm.view().suppressed);
        assert_eq!((w.fsm.behavior(), w.fsm.view().expression, w.fsm.view().version), (b0, e0, v0), "FSM congelada");
        assert_eq!(host.views.lock().unwrap().iter().filter(|v| v.message.is_some()).count(), 0, "sin mensajes");
        // fin del episodio
        probe.set_notification_state(calm());
        t += 2_500;
        at(&mut w, &clock, t);
        t += 2_500;
        at(&mut w, &clock, t); // dos lecturas tranquilas: reaparece
        assert!(!w.fsm.view().suppressed && host.visible.load(Ordering::SeqCst));
        assert_eq!(w.fsm.behavior(), b0, "no salta a otro comportamiento al reaparecer");
        assert_eq!(w.fsm.next_wakeup(), Some(t + rem), "el plazo congelado se retoma: queda lo que faltaba, no 0");
        assert!(w.fsm.view().message.is_none(), "sin mensaje inmediato al reaparecer");
        // y durante el segundo siguiente tampoco habla
        at(&mut w, &clock, t + 1_000);
        assert!(w.fsm.view().message.is_none() && w.talker.message().is_none(), "el cooldown del hablante no disparó de golpe");
        assert_eq!(w.fsm.behavior(), b0);
    }

    // ---- E5: sueño por inactividad, sesión y energía (FakeProbe + FakeClock, sin hilos ni esperas reales) ----

    use super::super::probe::idle_ms_from_ticks;
    use super::super::sleep as sl;
    use super::super::state::WAKE_SURPRISE_MS;

    const MIN: Ms = 60_000;

    /// Plazos de la FSM larguísimos (no interfieren) y hablante con el cooldown ya vencido (si pudiera hablar, hablaría).
    fn sleep_cfg(after: Ms) -> EngineConfig {
        EngineConfig {
            timings: Timings { idle: (7_200_000, 7_200_000), ..Timings::default() },
            talker: TalkerConfig { cooldown: 1_000, initial_delay: (1_000, 1_000), ..TalkerConfig::default() },
            sleep_enabled: true,
            sleep_after_ms: after,
            seed: Some(1),
            ..EngineConfig::default()
        }
    }

    fn sleep_worker(cfg: EngineConfig) -> DndWorker {
        let (mut w, clock, host) = worker(cfg);
        let probe = Arc::new(FakeProbe::new());
        w.probe = Some(probe.clone());
        (w, clock, host, probe)
    }

    /// Un usuario que no toca nada desde `last_input`: la sonda informa `t − last_input` y se da un paso en `t`.
    fn idle_step(w: &mut Worker<Fake>, clock: &FakeClock, probe: &FakeProbe, last_input: Ms, t: Ms) {
        probe.set_idle(Ok(t.saturating_sub(last_input)));
        at(w, clock, t);
    }

    /// Avanza de `from` a `to` de a `step` ms con la inactividad medida desde `last_input`.
    fn run_idle(w: &mut Worker<Fake>, clock: &FakeClock, probe: &FakeProbe, last_input: Ms, from: Ms, to: Ms, step: Ms) {
        let mut t = from;
        while t < to {
            t = (t + step).min(to);
            idle_step(w, clock, probe, last_input, t);
        }
    }

    fn visible_views(host: &Fake) -> Vec<View> {
        host.views.lock().unwrap().clone()
    }

    #[test]
    fn nothing_is_polled_without_a_probe_or_with_the_switch_off() {
        let (mut w, clock, _h) = worker(sleep_cfg(10 * MIN));
        at(&mut w, &clock, 0);
        assert!(w.sleep.next.is_none(), "sin sonda: ninguna tarea de sueño");
        assert_eq!(next_deadline(&w, 0), w.fsm.next_wakeup(), "y ningún plazo propio");
        let (mut w, clock, _h, probe) = sleep_worker(EngineConfig { sleep_enabled: false, ..sleep_cfg(10 * MIN) });
        probe.set_idle(Ok(99 * MIN));
        for k in 0..10 {
            at(&mut w, &clock, k * MIN);
        }
        assert_eq!(probe.idle_calls(), 0, "interruptor apagado: ninguna consulta");
        assert!(!w.fsm.view().sleeping);
    }

    #[test]
    fn the_first_check_is_immediate_and_the_next_one_lands_exactly_on_the_threshold() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        assert_eq!(probe.idle_calls(), 1);
        assert_eq!(w.sleep.next, Some(10 * MIN), "umbral − inactividad = 10 min − 0");
        assert_eq!(next_deadline(&w, 0), Some(10 * MIN), "entra en el planificador único, sin sondeo fijo");
        // durante 9 min 59 s no se consulta nada más: ni un despertar del hilo
        assert_eq!(probe.idle_calls(), 1);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN - 1);
        assert_eq!(probe.idle_calls(), 1, "un instante antes del umbral nadie despierta");
        assert!(!w.fsm.view().sleeping);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        assert_eq!(probe.idle_calls(), 2);
        assert!(w.fsm.view().sleeping, "inactividad == umbral: duerme");
    }

    #[test]
    fn one_millisecond_below_the_threshold_does_not_sleep_and_it_rechecks_soon() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        probe.set_idle(Ok(10 * MIN - 1));
        at(&mut w, &clock, 10 * MIN);
        assert!(!w.fsm.view().sleeping);
        assert_eq!(w.sleep.next, Some(10 * MIN + sl::MIN_CHECK_MS), "mínimo razonable, no un bucle ocupado");
    }

    #[test]
    fn sleeping_shows_a_stable_rest_that_never_leaves_by_rest_deadlines() {
        let (mut w, clock, host, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        let v = w.fsm.view();
        assert_eq!((v.behavior, v.expression, v.sleeping), (Behavior::Rest, Expression::Sleepy, true));
        run_idle(&mut w, &clock, &probe, 0, 10 * MIN, 10 * MIN + 5_000, 1_000);
        let v = w.fsm.view();
        assert_eq!((v.behavior, v.expression), (Behavior::Rest, Expression::Resting), "sleepy y luego resting");
        // una hora más: nada cambia (ni plazos de Rest ni reacciones ni mensajes)
        let n = visible_views(&host).len();
        run_idle(&mut w, &clock, &probe, 0, 10 * MIN + 5_000, 70 * MIN, 2_000);
        assert_eq!(visible_views(&host).len(), n, "durante el sueño no se emite ninguna vista nueva");
        assert!(visible_views(&host).iter().all(|v| v.message.is_none()), "ni un mensaje");
        assert!(host.visible.load(Ordering::SeqCst), "sueño no es suspensión: la ventana sigue visible");
    }

    #[test]
    fn the_wake_poll_is_light_and_has_the_planned_interval() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        let before = probe.idle_calls();
        run_idle(&mut w, &clock, &probe, 0, 10 * MIN, 10 * MIN + 60_000, 500);
        let calls = probe.idle_calls() - before;
        assert_eq!(calls as u64, 60_000 / sl::WAKE_POLL_MS, "una consulta cada {} ms durmiendo", sl::WAKE_POLL_MS);
    }

    #[test]
    fn activity_wakes_it_with_a_short_surprise_without_a_message_and_then_returns() {
        let (mut w, clock, host, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        run_idle(&mut w, &clock, &probe, 0, 10 * MIN, 30 * MIN, 2_000);
        assert!(w.fsm.view().sleeping);
        let rem = w.fsm.next_wakeup(); // solo el paso sleepy→resting (ya pasó): ninguno
        assert_eq!(rem, None);
        // la persona mueve el ratón en t = 30 min: la siguiente consulta (≤ 2 s) lo ve
        let t_act = 30 * MIN;
        probe.set_idle(Ok(500));
        at(&mut w, &clock, t_act + sl::WAKE_POLL_MS);
        let v = w.fsm.view();
        assert!(!v.sleeping);
        assert_eq!((v.behavior, v.expression), (Behavior::Reaction, Expression::Surprised), "sorpresa");
        assert!(v.message.is_none() && w.talker.message().is_none(), "sin mensaje");
        assert!(!w.fsm.messages_allowed(), "ni durante la sorpresa");
        // dura poco y vuelve a Idle
        probe.set_idle(Ok(2_000));
        at(&mut w, &clock, t_act + sl::WAKE_POLL_MS + WAKE_SURPRISE_MS - 1);
        assert_eq!(w.fsm.view().expression, Expression::Surprised);
        probe.set_idle(Ok(2_100));
        at(&mut w, &clock, t_act + sl::WAKE_POLL_MS + WAKE_SURPRISE_MS);
        let v = w.fsm.view();
        assert_eq!((v.behavior, v.expression, v.sleeping), (Behavior::Idle, Expression::Idle, false));
        assert!(visible_views(&host).iter().filter(|v| v.message.is_some()).count() == 0);
    }

    #[test]
    fn after_a_long_sleep_there_is_no_burst_of_accumulated_behaviors() {
        for total in [10 * MIN, 120 * MIN] {
            // plazos por defecto (Idle 20–45 s): durante el sueño vencerían decenas de veces
            let cfg = EngineConfig { timings: Timings::default(), ..sleep_cfg(30_000) };
            let (mut w, clock, host, probe) = sleep_worker(cfg);
            idle_step(&mut w, &clock, &probe, 0, 0);
            // el usuario deja el equipo en t = 0; a los 30 s duerme (si Miel estaba en pleno Follow/Rest, da igual)
            run_idle(&mut w, &clock, &probe, 0, 0, 30_000, 1_000);
            assert!(w.fsm.view().sleeping, "duerme a los 30 s (total {total})");
            let slept_at = 30_000;
            let n0 = visible_views(&host).len();
            run_idle(&mut w, &clock, &probe, 0, slept_at, slept_at + total, 2_000);
            // solo pudo cambiar sleepy→resting (una vista como mucho); ningún comportamiento nuevo
            let extra: Vec<_> = visible_views(&host)[n0..].to_vec();
            assert!(extra.len() <= 1, "total {total}: vistas durante el sueño = {}", extra.len());
            assert!(extra.iter().all(|v| v.sleeping && v.behavior == Behavior::Rest));
            // despierta: UNA sorpresa y luego se retoma lo congelado (no hay ráfaga)
            let t_wake = slept_at + total + sl::WAKE_POLL_MS;
            probe.set_idle(Ok(0));
            at(&mut w, &clock, t_wake);
            let n1 = visible_views(&host).len();
            assert_eq!(w.fsm.view().expression, Expression::Surprised);
            at(&mut w, &clock, t_wake + WAKE_SURPRISE_MS);
            let after: Vec<_> = visible_views(&host)[n1..].to_vec();
            assert!(after.len() <= 1, "total {total}: tras la sorpresa solo vuelve al estado previo ({})", after.len());
            let d = w.fsm.next_wakeup().expect("hay un plazo propio retomado");
            assert!(d > t_wake + WAKE_SURPRISE_MS, "el plazo se retoma hacia adelante, no vence de golpe: {d}");
            assert!(d - (t_wake + WAKE_SURPRISE_MS) <= 90_000, "es el tiempo que quedaba, no el sueño acumulado: {d}");
        }
    }

    #[test]
    fn sleeping_and_waking_never_draw_from_the_rng() {
        let cfg = EngineConfig { timings: Timings::default(), seed: Some(9), ..sleep_cfg(30_000) };
        let (mut w, clock, _h, probe) = sleep_worker(cfg);
        idle_step(&mut w, &clock, &probe, 0, 0);
        run_idle(&mut w, &clock, &probe, 0, 0, 30_000, 1_000);
        assert!(w.fsm.view().sleeping);
        let snapshot = w.rng.clone(); // estado del RNG justo al dormirse
        run_idle(&mut w, &clock, &probe, 0, 30_000, 90_000, 2_000);
        probe.set_idle(Ok(0));
        at(&mut w, &clock, 92_000);
        assert_eq!(w.fsm.view().expression, Expression::Surprised);
        at(&mut w, &clock, 92_000 + WAKE_SURPRISE_MS);
        assert!(!w.fsm.view().sleeping);
        let (mut a, mut b) = (snapshot, w.rng.clone());
        assert_eq!((a.u64(..), a.u64(..)), (b.u64(..), b.u64(..)), "ni dormir, ni despertar, ni la sorpresa sortean nada");
    }

    #[test]
    fn an_error_never_makes_it_sleep_over_two_hours_and_the_retries_back_off() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        probe.set_idle(Err(ProbeError::CallFailed));
        at(&mut w, &clock, 0);
        let mut t = 0;
        let mut stamps = vec![0u64];
        while t < 120 * MIN {
            let next = next_deadline(&w, t).expect("reintenta");
            assert!(next > t, "no hay bucle ocupado");
            t = next;
            let before = probe.idle_calls();
            at(&mut w, &clock, t);
            if probe.idle_calls() > before {
                stamps.push(t);
            }
            assert!(!w.fsm.view().sleeping, "un error nunca provoca sueño");
        }
        let gaps: Vec<_> = stamps.windows(2).map(|p| p[1] - p[0]).collect();
        assert_eq!(&gaps[..5], &[5_000, 10_000, 20_000, 40_000, 60_000]);
        assert!(gaps.iter().all(|g| *g <= sl::BACKOFF_MAX_MS));
        // y cuando la sonda vuelve, se recupera el plazo normal
        probe.set_idle(Ok(0));
        at(&mut w, &clock, t + 60_000);
        assert_eq!(w.sleep.errors, 0);
    }

    #[test]
    fn u32_wraparound_and_implausible_deltas_are_handled_before_they_reach_the_engine() {
        // dwTime justo antes de 2^32 y el tick ya envuelto: 21 ms de inactividad (10 + 11), no 49 días
        assert_eq!(idle_ms_from_ticks(10, Some(u32::MAX - 10)), Ok(21));
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        probe.set_idle(idle_ms_from_ticks(10, Some(u32::MAX - 10)));
        at(&mut w, &clock, 0);
        assert!(!w.fsm.view().sleeping);
        // delta > 2^31 (lectura adelantada por la carrera): cuenta como 0, no como 24 días
        probe.set_idle(idle_ms_from_ticks(100, Some(200)));
        assert_eq!(probe.idle_ms(), Ok(0));
        let t = w.sleep.next.unwrap();
        at(&mut w, &clock, t);
        assert!(!w.fsm.view().sleeping);
        // 24 días reales (< 2^31 ms): duerme; y nunca más de 30 días informados
        probe.set_idle(idle_ms_from_ticks(0x7FFF_FFFF, Some(0)));
        let t = w.sleep.next.unwrap();
        at(&mut w, &clock, t);
        assert!(w.fsm.view().sleeping);
    }

    #[test]
    fn a_changed_threshold_applies_live_in_both_directions() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        // 7 min de inactividad; umbral de 10 min → comprobará a los 3 min que faltan
        probe.set_idle(Ok(7 * MIN));
        at_with(&mut w, &clock, 7 * MIN, vec![Cmd::SleepAfter(15 * MIN)]);
        assert!(!w.fsm.view().sleeping);
        assert_eq!(w.sleep.next, Some(7 * MIN + 8 * MIN), "recalcula: 15 − 7 min");
        // bajar el umbral por debajo de la inactividad actual: duerme al instante
        at_with(&mut w, &clock, 7 * MIN + 1_000, vec![Cmd::SleepAfter(5 * MIN)]);
        assert!(w.fsm.view().sleeping);
        // durmiendo, subir el umbral por encima de la inactividad: despierta (la inactividad ya no basta)
        probe.set_idle(Ok(7 * MIN + 2_000));
        at_with(&mut w, &clock, 7 * MIN + 2_000, vec![Cmd::SleepAfter(30 * MIN)]);
        assert!(!w.fsm.view().sleeping);
        assert_eq!(w.fsm.view().expression, Expression::Surprised);
    }

    #[test]
    fn turning_the_switch_off_live_wakes_and_stops_polling_and_on_resumes_with_an_immediate_read() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        assert!(w.fsm.view().sleeping);
        probe.set_idle(Ok(11 * MIN));
        at_with(&mut w, &clock, 11 * MIN, vec![Cmd::SleepEnabled(false)]);
        assert!(!w.fsm.view().sleeping, "apagar despierta");
        assert_eq!(w.fsm.view().expression, Expression::Surprised);
        let calls = probe.idle_calls();
        run_idle(&mut w, &clock, &probe, 0, 11 * MIN, 60 * MIN, 5_000);
        assert_eq!(probe.idle_calls(), calls, "apagado: ni una consulta más");
        assert!(!w.fsm.view().sleeping, "y no vuelve a dormirse");
        // encender con la inactividad ya pasada: lee de inmediato y duerme
        at_with(&mut w, &clock, 60 * MIN + 1, vec![Cmd::SleepEnabled(true)]);
        assert_eq!(probe.idle_calls(), calls + 1);
        assert!(w.fsm.view().sleeping);
    }

    #[test]
    fn hidden_paused_or_suppressed_never_poll_and_nothing_wakes_the_engine() {
        for (name, on, off) in [
            ("oculta", Cmd::Hidden(true), Cmd::Hidden(false)),
            ("pausada", Cmd::Pause(true), Cmd::Pause(false)),
            ("suprimida", Cmd::Suppressed(true), Cmd::Suppressed(false)),
        ] {
            let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
            idle_step(&mut w, &clock, &probe, 0, 0);
            let calls = probe.idle_calls();
            probe.set_idle(Ok(99 * MIN));
            at_with(&mut w, &clock, 1_000, vec![on]);
            assert_eq!(next_deadline(&w, 1_000), None, "{name}: ni un plazo");
            for k in 1..50 {
                at(&mut w, &clock, 1_000 + k * MIN);
            }
            assert_eq!(probe.idle_calls(), calls, "{name}: no se consulta la inactividad");
            assert!(!w.fsm.view().sleeping, "{name}: no duerme sin que nadie lo mire");
            // al volver a correr se reconsulta de inmediato
            at_with(&mut w, &clock, 60 * MIN, vec![off]);
            assert!(probe.idle_calls() > calls, "{name}: reconsulta al volver");
        }
    }

    #[test]
    fn resuming_from_the_tray_ends_the_sleep_explicitly_without_waiting_for_a_poll() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        at_with(&mut w, &clock, 10 * MIN + 1_000, vec![Cmd::Pause(true)]);
        assert!(w.fsm.view().sleeping, "pausada conserva el estado, sin sondear");
        let calls = probe.idle_calls();
        // el equipo sigue sin actividad en la lectura de Windows (la persona usó el menú de la bandeja, otro hilo)
        probe.set_idle(Ok(40 * MIN));
        at_with(&mut w, &clock, 40 * MIN, vec![Cmd::Pause(false)]);
        let v = w.fsm.view();
        assert!(!v.sleeping && !v.paused, "reanudar termina el sueño ya");
        assert_eq!(v.expression, Expression::Surprised);
        assert!(probe.idle_calls() >= calls);
        // la acción cuenta como actividad: Windows aún dice 40 min, pero no vuelve a dormir hasta un umbral después
        at(&mut w, &clock, 40 * MIN + 5_000);
        assert!(!w.fsm.view().sleeping, "no se duerme otra vez de inmediato");
        probe.set_idle(Ok(40 * MIN + 10 * MIN));
        at(&mut w, &clock, 50 * MIN);
        assert!(w.fsm.view().sleeping, "un umbral después de la acción, duerme de nuevo");
    }

    #[test]
    fn showing_a_hidden_sleeping_mascot_and_toggling_follow_also_end_the_sleep() {
        // Mostrar
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        at_with(&mut w, &clock, 10 * MIN + 1_000, vec![Cmd::Hidden(true)]);
        assert!(w.fsm.view().sleeping && !w.fsm.is_visible());
        probe.set_idle(Ok(30 * MIN));
        at_with(&mut w, &clock, 30 * MIN, vec![Cmd::Hidden(false)]);
        assert!(!w.fsm.view().sleeping && w.fsm.is_visible());
        // Seguir
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        probe.set_idle(Ok(11 * MIN));
        at_with(&mut w, &clock, 11 * MIN, vec![Cmd::FollowEnabled(false)]);
        assert!(w.fsm.view().sleeping, "desactivar Seguir no despierta");
        at_with(&mut w, &clock, 11 * MIN + 1_000, vec![Cmd::ToggleFollow]);
        assert!(!w.fsm.view().sleeping, "activar Seguir despierta");
    }

    #[test]
    fn lock_and_unlock_sleep_at_once_and_wake_with_the_surprise() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        // bloqueo a los 20 s con inactividad ~0: duerme de inmediato, sin esperar el umbral
        probe.set_idle(Ok(20_000));
        at_with(&mut w, &clock, 20_000, vec![Cmd::Session(SessionEvent::Locked)]);
        assert!(w.fsm.view().sleeping);
        // mientras está bloqueada no se sondea (Windows no ve la entrada del escritorio seguro)
        let calls = probe.idle_calls();
        probe.set_idle(Ok(0)); // una lectura "con actividad" no la despierta: no se lee
        for k in 1..30 {
            at(&mut w, &clock, 20_000 + k * 2_000);
        }
        assert_eq!(probe.idle_calls(), calls);
        assert!(w.fsm.view().sleeping);
        at_with(&mut w, &clock, 120_000, vec![Cmd::Session(SessionEvent::Unlocked)]);
        let v = w.fsm.view();
        assert!(!v.sleeping);
        assert_eq!(v.expression, Expression::Surprised);
        // y el sondeo de inactividad se retoma
        at(&mut w, &clock, 125_000);
        assert!(probe.idle_calls() > calls);
    }

    #[test]
    fn suspend_and_resume_behave_like_lock_and_unlock_and_a_lock_survives_the_resume() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        at_with(&mut w, &clock, 5_000, vec![Cmd::Session(SessionEvent::Suspended)]);
        assert!(w.fsm.view().sleeping);
        at_with(&mut w, &clock, 3_600_000, vec![Cmd::Session(SessionEvent::Resumed)]);
        assert!(!w.fsm.view().sleeping && w.fsm.view().expression == Expression::Surprised);
        // el salto de una hora no se interpreta como inactividad: la reanudación cuenta como actividad reciente
        probe.set_idle(Ok(70 * MIN));
        at(&mut w, &clock, 3_605_000);
        assert!(!w.fsm.view().sleeping, "la inactividad de 70 min leída tras reanudar no la duerme de golpe");
        // bloqueada + suspendida: reanudar no despierta mientras siga bloqueada
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        at_with(&mut w, &clock, 1_000, vec![Cmd::Session(SessionEvent::Locked)]);
        at_with(&mut w, &clock, 2_000, vec![Cmd::Session(SessionEvent::Suspended)]);
        at_with(&mut w, &clock, 9_000, vec![Cmd::Session(SessionEvent::Resumed)]);
        assert!(w.fsm.view().sleeping, "sigue bloqueada: sigue dormida");
        at_with(&mut w, &clock, 9_500, vec![Cmd::Session(SessionEvent::Unlocked)]);
        assert!(!w.fsm.view().sleeping);
    }

    #[test]
    fn session_events_are_ignored_while_the_switch_is_off_and_applied_when_it_is_turned_on_away() {
        let (mut w, clock, _h, probe) = sleep_worker(EngineConfig { sleep_enabled: false, ..sleep_cfg(10 * MIN) });
        probe.set_idle(Ok(0));
        at_with(&mut w, &clock, 0, vec![Cmd::Session(SessionEvent::Locked)]);
        assert!(!w.fsm.view().sleeping, "con el interruptor apagado el bloqueo no duerme");
        at_with(&mut w, &clock, 1_000, vec![Cmd::SleepEnabled(true)]);
        assert!(w.fsm.view().sleeping, "se enciende con la sesión ausente: duerme");
        at_with(&mut w, &clock, 2_000, vec![Cmd::Session(SessionEvent::Unlocked)]);
        assert!(!w.fsm.view().sleeping);
    }

    #[test]
    fn the_end_of_a_suppression_episode_reappears_asleep_only_if_the_inactivity_passed_the_threshold() {
        for (idle_during, expect_asleep) in [(15 * MIN, true), (1_000u64, false)] {
            let cfg = EngineConfig { dnd_auto: true, ..sleep_cfg(10 * MIN) };
            let (mut w, clock, host, probe) = sleep_worker(cfg);
            probe.set_idle(Ok(0));
            probe.set_notification_state(busy());
            at(&mut w, &clock, 0);
            at(&mut w, &clock, 2_500); // 2 lecturas: suprimida
            assert!(w.fsm.view().suppressed && !host.visible.load(Ordering::SeqCst));
            // durante el episodio no se sondea la inactividad (ni dormiría ni despertaría)
            let calls = probe.idle_calls();
            probe.set_idle(Ok(idle_during));
            for k in 2..6 {
                at(&mut w, &clock, k * 2_500);
            }
            assert_eq!(probe.idle_calls(), calls, "suprimida: sin sondeo de inactividad");
            // fin del episodio
            probe.set_notification_state(calm());
            at(&mut w, &clock, 15_000);
            at(&mut w, &clock, 17_500);
            assert!(host.visible.load(Ordering::SeqCst) && !w.fsm.view().suppressed);
            assert_eq!(w.fsm.view().sleeping, expect_asleep, "inactividad {idle_during}");
            // y lo que Miel muestra en el primer instante visible ya es lo correcto (no hubo un parpadeo despierta)
            let first_visible = visible_views(&host).last().copied().unwrap();
            assert_eq!(first_visible.sleeping, expect_asleep);
        }
    }

    #[test]
    fn a_fullscreen_game_that_spans_the_threshold_does_not_sleep_it_but_it_reappears_asleep() {
        let cfg = EngineConfig { dnd_auto: true, ..sleep_cfg(10 * MIN) };
        let (mut w, clock, host, probe) = sleep_worker(cfg);
        probe.set_idle(Ok(0));
        probe.set_notification_state(busy());
        at(&mut w, &clock, 0);
        at(&mut w, &clock, 2_500);
        // el juego dura 30 min con el usuario "inactivo" (viendo una cinemática): el umbral pasa estando suprimida
        let mut t = 2_500;
        while t < 30 * MIN {
            t += 2_500;
            probe.set_idle(Ok(t));
            at(&mut w, &clock, t);
        }
        assert!(!w.fsm.view().sleeping, "oculta por No molestar: nada de sueño ni sondeo");
        probe.set_notification_state(calm());
        t += 2_500;
        at(&mut w, &clock, t);
        t += 2_500;
        at(&mut w, &clock, t);
        assert!(host.visible.load(Ordering::SeqCst) && w.fsm.view().sleeping);
    }

    #[test]
    fn streaks_of_activity_just_before_the_threshold_keep_it_awake_with_one_read_per_period() {
        let (mut w, clock, _h, probe) = sleep_worker(sleep_cfg(10 * MIN));
        let mut last_input = 0;
        probe.set_idle(Ok(0));
        at(&mut w, &clock, 0);
        // cada 9 min 50 s hay actividad (justo antes del vencimiento); durante 6 h nunca duerme
        let mut t = 0;
        let start_calls = probe.idle_calls();
        let mut seen_sleep = false;
        for _ in 0..40 {
            t = w.sleep.next.expect("próxima comprobación");
            // en cada comprobación ya hubo actividad hace 10 s
            last_input = t - 10_000;
            idle_step(&mut w, &clock, &probe, last_input, t);
            seen_sleep |= w.fsm.view().sleeping;
        }
        let _ = last_input;
        assert!(!seen_sleep);
        let calls = probe.idle_calls() - start_calls;
        assert_eq!(calls, 40, "una consulta por comprobación, no un sondeo continuo");
        assert!(t > 40 * 9 * MIN, "{t}");
    }

    #[test]
    fn flapping_readings_around_the_margin_do_not_make_it_flicker() {
        let (mut w, clock, host, probe) = sleep_worker(sleep_cfg(10 * MIN));
        idle_step(&mut w, &clock, &probe, 0, 0);
        idle_step(&mut w, &clock, &probe, 0, 10 * MIN);
        assert!(w.fsm.view().sleeping);
        let n = visible_views(&host).len();
        // lecturas que oscilan entre "justo en el margen" y "mucho más": nunca por debajo de umbral − margen
        for k in 1..=40u64 {
            let idle = if k % 2 == 0 { 10 * MIN - sl::WAKE_MARGIN_MS } else { 10 * MIN + 5_000 };
            probe.set_idle(Ok(idle));
            at(&mut w, &clock, 10 * MIN + k * sl::WAKE_POLL_MS);
        }
        assert!(w.fsm.view().sleeping);
        assert!(visible_views(&host).len() <= n + 1, "como mucho el paso sleepy→resting");
    }

    #[test]
    fn the_wake_up_scheduler_wakes_are_counted_per_state_label() {
        let (mut w, _c, _h, _p) = sleep_worker(sleep_cfg(10 * MIN));
        assert_eq!(w.wake_label(), "idle");
        w.fsm.set_sleeping(0, true);
        assert_eq!(w.wake_label(), "durmiendo");
        w.fsm.set_paused(0, true);
        assert_eq!(w.wake_label(), "pausada");
        w.fsm.set_hidden(0, true);
        assert_eq!(w.wake_label(), "oculta");
        w.fsm.set_hidden(0, false);
        w.fsm.set_paused(0, false);
        w.fsm.set_suppressed(0, true);
        assert_eq!(w.wake_label(), "suprimida");
    }
}
