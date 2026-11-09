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
use super::geometry::{
    anchor_of, clamp_anchor, ensure_center_visible, follow_target, fully_visible, nearest_area,
    rest_spot, window_origin, Frame, Rect, Vec2,
};
use super::messages::{Talker, TalkerConfig};
use super::motion::{Motion, MotionParams};
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

    /// Como `spawn`, con el reloj del motor inyectado (pruebas deterministas con `FakeClock`).
    pub fn spawn_with_clock<H: Host>(host: Arc<H>, cfg: EngineConfig, clock: Arc<dyn Clock>) -> Engine {
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
        let worker = Worker::new(host, cfg, fsm, talker, rng, clock);
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
    (a.behavior, a.expression, a.paused, a.hidden, a.follow_enabled, a.message)
        == (b.behavior, b.expression, b.paused, b.hidden, b.follow_enabled, b.message)
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
        }
    }

    fn now(&self) -> Ms {
        self.clock.now_ms()
    }

    fn run(mut self, shared: &Shared) {
        let _stop = StopGuard(shared);
        self.refresh(true);
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
            let mut changed = false;
            for c in cmds {
                changed |= self.apply(c, now);
            }
            changed |= self.fsm.step(now, &mut self.rng);
            changed |= self.talker.step(now);
            changed |= self.talker.observe(now, &self.fsm.view(), &mut self.rng);
            if changed {
                // La FSM puede cambiar algo que el mensaje visible tapa (p. ej. sleepy→resting con
                // la cara del mensaje activa): solo se emite si la vista compuesta es distinta.
                let view = compose(&self.fsm, &self.talker);
                if !same_view(&view, &self.last_emitted) {
                    self.last_emitted = view;
                    lock(&shared.inner).view = view;
                    self.host.emit_view(&view);
                }
            }
            if self.geom_due.is_some_and(|d| self.now() >= d) {
                self.geom_due = None;
                self.on_geometry_changed();
            }
            self.drive_motion();
        }
    }

    /// Cuánto puede dormir el hilo. `None` = indefinidamente (suspendido y sin avisos pendientes).
    fn wait_timeout(&self) -> Option<Duration> {
        let now = self.now();
        let by_geom = self.geom_due.map(|d| Duration::from_millis(d.saturating_sub(now)));
        if self.fsm.is_suspended() {
            return by_geom; // pausado/oculto: solo un aviso de geometría pendiente puede despertarlo
        }
        let by_ms = [self.fsm.next_wakeup(), self.talker.next_wakeup()]
            .into_iter()
            .flatten()
            .min()
            .map(|t| Duration::from_millis(t.saturating_sub(now)));
        let by_motion = if self.moving {
            Some(Duration::from_secs_f64(1.0 / self.cfg.follow_hz))
        } else if self.fsm.behavior() == Behavior::Follow {
            Some(Duration::from_secs_f64(1.0 / self.cfg.follow_settled_hz))
        } else {
            None
        };
        [by_ms, by_motion, by_geom].into_iter().flatten().min()
    }

    fn apply(&mut self, cmd: Cmd, now: Ms) -> bool {
        match cmd {
            Cmd::Pause(p) => {
                let changed = self.fsm.set_paused(now, p);
                self.talker.set_suspended(now, self.fsm.is_suspended()) | changed
            }
            Cmd::Hidden(h) => {
                let changed = self.fsm.set_hidden(now, h);
                if changed {
                    self.host.set_visible(!h);
                    if !h {
                        // al mostrar: relee monitores y reubica si la posición dejó de ser válida
                        self.on_geometry_changed();
                    }
                }
                self.talker.set_suspended(now, self.fsm.is_suspended()) | changed
            }
            Cmd::FollowEnabled(e) => self.fsm.set_follow_enabled(now, &mut self.rng, e),
            Cmd::TogglePause => {
                let changed = self.fsm.toggle_paused(now);
                self.talker.set_suspended(now, self.fsm.is_suspended()) | changed
            }
            Cmd::ToggleFollow => self.fsm.toggle_follow(now, &mut self.rng),
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
}
