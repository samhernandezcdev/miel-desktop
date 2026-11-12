//! Máquina de estados de comportamiento. Pura y determinista: el tiempo (`Ms`) y el azar
//! (`fastrand::Rng`, sembrable) se inyectan; no toca ventanas ni hilos.
//!
//! Distingue dos conceptos:
//! - `Behavior`: qué está haciendo Miel (Idle / Follow / Rest / Reaction).
//! - `Expression`: qué cara muestra (los 8 estados del rig). Se deriva del comportamiento.
//!
//! `paused` y `hidden` son banderas ortogonales que SUSPENDEN todo (prioridad máxima): congelan
//! los temporizadores y al reanudar se continúa con el tiempo restante.
//!
//! Infraestructura de supresión (E1b; sin detección automática todavía, en producción siempre falsas):
//! `suppressed` (supresión automática, p. ej. No molestar), `dnd_override` (anulación del episodio) y
//! `sleeping` (comportamiento, no suspensión). Ver `visible`, `running` y `messages_allowed`, y
//! `docs/plan/v0.2.0.md` §4.1 y §5.

use serde::Serialize;

use super::messages::MessageView;

pub type Ms = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Behavior {
    Idle,
    Follow,
    Rest,
    Reaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Expression {
    Idle,
    Happy,
    Curious,
    Thinking,
    Sleepy,
    Surprised,
    /// Reservada para la Etapa 5 (mensajes).
    #[allow(dead_code)]
    Talking,
    Resting,
}

/// Reacciones posibles (subconjunto de expresiones). `Talking` queda para la Etapa 5.
const REACTION_EXPRESSIONS: [Expression; 4] =
    [Expression::Happy, Expression::Curious, Expression::Thinking, Expression::Surprised];

/// Rangos (mín, máx) en ms; el valor se sortea uniformemente.
#[derive(Debug, Clone, Copy)]
pub struct Timings {
    pub idle: (Ms, Ms),
    pub follow: (Ms, Ms),
    pub rest: (Ms, Ms),
    pub reaction: (Ms, Ms),
    /// En Rest se muestra `sleepy` este tiempo y luego `resting`.
    pub rest_settle: Ms,
    /// Tiempo mínimo entre el fin de un Follow y el inicio del siguiente.
    pub follow_cooldown: Ms,
    /// Tiempo mínimo entre el fin de una Reaction y el inicio de la siguiente.
    pub reaction_cooldown: Ms,
    /// Pesos relativos (Follow, Rest, Reaction) al decidir qué hacer tras un Idle.
    pub weights: (u32, u32, u32),
}

impl Default for Timings {
    fn default() -> Self {
        const S: Ms = 1000;
        Self {
            idle: (20 * S, 45 * S),
            follow: (6 * S, 12 * S),
            rest: (40 * S, 90 * S),
            reaction: (5 * S, 7 * S),  // ≥ duración de un mensaje (4–6 s)
            rest_settle: 4 * S,
            follow_cooldown: 90 * S,
            reaction_cooldown: 40 * S,
            weights: (40, 25, 35),
        }
    }
}

/// Supresión automática EN VIGOR: hay un episodio (`suppressed`) y la persona no lo ha anulado.
pub fn suppression_active(suppressed: bool, dnd_override: bool) -> bool {
    suppressed && !dnd_override
}

/// ¿Se ve la ventana? No si la persona la ocultó (`hidden`) ni si hay supresión en vigor.
pub fn visible(hidden: bool, suppressed: bool, dnd_override: bool) -> bool {
    !hidden && !suppression_active(suppressed, dnd_override)
}

/// ¿Avanza la FSM y planifica el motor? Visible y no pausada.
pub fn running(hidden: bool, paused: bool, suppressed: bool, dnd_override: bool) -> bool {
    visible(hidden, suppressed, dnd_override) && !paused
}

/// ¿Puede hablar Miel? Corriendo y despierta.
pub fn messages_allowed(hidden: bool, paused: bool, suppressed: bool, dnd_override: bool, sleeping: bool) -> bool {
    running(hidden, paused, suppressed, dnd_override) && !sleeping
}

/// Foto observable del estado; `version` crece con cada cambio observable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub version: u64,
    pub behavior: Behavior,
    pub expression: Expression,
    pub paused: bool,
    pub hidden: bool,
    /// ¿Se permite entrar en Follow? (la bandeja lee su casilla de aquí: única fuente de verdad)
    pub follow_enabled: bool,
    /// Mensaje visible (lo rellena el motor a partir del `Talker`; la FSM no sabe de mensajes).
    pub message: Option<MessageView>,
    /// Supresión automática EN VIGOR (episodio sin anular). Aditivo: `hidden`/`paused` conservan el
    /// significado de v0.1.0. En producción siempre `false` hasta E4.
    pub suppressed: bool,
    /// Miel duerme (comportamiento, no suspensión). Aditivo; en producción siempre `false` hasta E5.
    pub sleeping: bool,
}

/// Tabla de transiciones válidas entre comportamientos.
pub fn can_transition(from: Behavior, to: Behavior) -> bool {
    use Behavior::*;
    matches!(
        (from, to),
        (Idle, Follow)
            | (Idle, Rest)
            | (Idle, Reaction)
            | (Follow, Idle)
            | (Rest, Idle)
            | (Rest, Reaction)
            | (Reaction, Idle)
            | (Reaction, Rest)
    )
}

#[derive(Debug, Clone, Copy)]
struct Timers {
    /// Fin del comportamiento actual.
    deadline: Ms,
    /// Cambio sleepy → resting dentro de Rest.
    settle_at: Option<Ms>,
}

#[derive(Debug, Clone, Copy)]
struct Resume {
    behavior: Behavior,
    /// Tiempo restante del comportamiento interrumpido por la reacción.
    remaining: Ms,
    settled: bool,
}

#[derive(Debug)]
pub struct Fsm {
    t: Timings,
    behavior: Behavior,
    expression: Expression,
    paused: bool,
    hidden: bool,
    /// Episodio de supresión automática (E4); anulable con `dnd_override`.
    suppressed: bool,
    /// Anulación del episodio actual ("Mostrar de todos modos" / Mostrar con supresión en vigor).
    dnd_override: bool,
    /// Sueño (E5): comportamiento visible como Rest estable; los plazos propios se congelan.
    sleeping: bool,
    /// Sueño: ya se muestra `resting` (pasó `rest_settle`).
    sleep_resting: bool,
    /// Sueño: instante en que `sleepy` pasa a `resting` (solo corre mientras `running`).
    sleep_settle_at: Option<Ms>,
    /// Sueño: tiempo restante hasta `resting` mientras está suspendida.
    sleep_settle_rem: Option<Ms>,
    follow_enabled: bool,
    timers: Timers,
    /// Tiempo restante congelado mientras los plazos están detenidos (`!running` o `sleeping`).
    frozen: Option<(Ms, Option<Ms>)>,
    resume: Option<Resume>,
    last_follow_end: Option<Ms>,
    last_reaction_end: Option<Ms>,
    last_reaction_expr: Option<Expression>,
    version: u64,
}

impl Fsm {
    pub fn new(t: Timings, now: Ms, rng: &mut fastrand::Rng) -> Self {
        let d = range(rng, t.idle);
        Self {
            t,
            behavior: Behavior::Idle,
            expression: Expression::Idle,
            paused: false,
            hidden: false,
            suppressed: false,
            dnd_override: false,
            sleeping: false,
            sleep_resting: false,
            sleep_settle_at: None,
            sleep_settle_rem: None,
            follow_enabled: true,
            timers: Timers { deadline: now + d, settle_at: None },
            frozen: None,
            resume: None,
            last_follow_end: None,
            last_reaction_end: None,
            last_reaction_expr: None,
            version: 0,
        }
    }

    pub fn view(&self) -> View {
        // Durmiendo, Miel se presenta como Rest estable (sleepy → resting); el comportamiento real
        // (`self.behavior`) y sus plazos quedan congelados y se retoman al despertar.
        let (behavior, expression) = if self.sleeping {
            (Behavior::Rest, if self.sleep_resting { Expression::Resting } else { Expression::Sleepy })
        } else {
            (self.behavior, self.expression)
        };
        View {
            version: self.version,
            behavior,
            expression,
            paused: self.paused,
            hidden: self.hidden,
            follow_enabled: self.follow_enabled,
            message: None,
            suppressed: self.suppression_active(),
            sleeping: self.sleeping,
        }
    }

    /// Comportamiento EFECTIVO (el que se ve): Rest mientras duerme.
    pub fn behavior(&self) -> Behavior {
        if self.sleeping {
            Behavior::Rest
        } else {
            self.behavior
        }
    }

    pub fn suppression_active(&self) -> bool {
        suppression_active(self.suppressed, self.dnd_override)
    }

    /// Ventana visible (ni oculta por la persona ni suprimida).
    pub fn is_visible(&self) -> bool {
        visible(self.hidden, self.suppressed, self.dnd_override)
    }

    /// El FSM avanza y el motor planifica.
    pub fn is_running(&self) -> bool {
        running(self.hidden, self.paused, self.suppressed, self.dnd_override)
    }

    /// La persona ocultó a Miel (`hidden` de v0.1.0).
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// La persona pausó a Miel (`paused` de v0.1.0).
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Hay un episodio de supresión abierto (haya sido anulado o no).
    pub fn is_suppression_episode(&self) -> bool {
        self.suppressed
    }

    pub fn is_sleeping(&self) -> bool {
        self.sleeping
    }

    pub fn messages_allowed(&self) -> bool {
        messages_allowed(self.hidden, self.paused, self.suppressed, self.dnd_override, self.sleeping)
    }

    /// `!running`; con `suppressed = false` es exactamente `paused || hidden` (v0.1.0).
    pub fn is_suspended(&self) -> bool {
        !self.is_running()
    }

    /// Alterna la pausa (atómico respecto al estado real: sin booleanos duplicados en la UI).
    pub fn toggle_paused(&mut self, now: Ms) -> bool {
        let p = !self.paused;
        self.set_paused(now, p)
    }

    pub fn toggle_follow(&mut self, now: Ms, rng: &mut fastrand::Rng) -> bool {
        let e = !self.follow_enabled;
        self.set_follow_enabled(now, rng, e)
    }

    /// Próximo instante en que `step` puede cambiar algo; `None` si no hay nada pendiente
    /// (suspendido): el motor puede dormir indefinidamente.
    pub fn next_wakeup(&self) -> Option<Ms> {
        if self.is_suspended() {
            return None;
        }
        if self.sleeping {
            return self.sleep_settle_at; // solo el paso sleepy → resting; Rest no vence por sí mismo
        }
        Some(match self.timers.settle_at {
            Some(s) => s.min(self.timers.deadline),
            None => self.timers.deadline,
        })
    }

    // ---- entradas externas -------------------------------------------------------------

    pub fn set_paused(&mut self, now: Ms, paused: bool) -> bool {
        self.mutate(|s| {
            s.paused = paused;
            s.sync_suspension(now);
        })
    }

    pub fn set_hidden(&mut self, now: Ms, hidden: bool) -> bool {
        self.mutate(|s| {
            let was_hidden = s.hidden;
            s.hidden = hidden;
            // Mostrar con supresión en vigor: además de quitar `hidden`, anula el episodio actual.
            if was_hidden && !hidden && s.suppression_active() {
                s.dnd_override = true;
            }
            s.sync_suspension(now);
        })
    }

    /// Empieza o termina un episodio de supresión. Al terminar se limpia la anulación.
    pub fn set_suppressed(&mut self, now: Ms, suppressed: bool) -> bool {
        self.mutate(|s| {
            s.suppressed = suppressed;
            if !suppressed {
                s.dnd_override = false;
            }
            s.sync_suspension(now);
        })
    }

    /// "Mostrar de todos modos": solo existe con supresión en vigor (si no, no hace nada). Anula el
    /// episodio actual, quita `hidden` y deja `paused` intacto.
    pub fn set_dnd_override(&mut self, now: Ms) -> bool {
        self.mutate(|s| {
            if s.suppression_active() {
                s.dnd_override = true;
                s.hidden = false;
                s.sync_suspension(now);
            }
        })
    }

    /// Entra o sale del sueño. Los plazos propios se congelan (no se descartan) mientras duerme.
    pub fn set_sleeping(&mut self, now: Ms, sleeping: bool) -> bool {
        self.mutate(|s| {
            if s.sleeping == sleeping {
                return;
            }
            s.sleeping = sleeping;
            s.sleep_resting = false;
            s.sleep_settle_at = None;
            s.sleep_settle_rem = sleeping.then_some(s.t.rest_settle);
            s.sync_suspension(now);
        })
    }

    /// Deshabilitar el seguimiento cancela un Follow en curso.
    pub fn set_follow_enabled(&mut self, now: Ms, rng: &mut fastrand::Rng, enabled: bool) -> bool {
        self.mutate(|s| {
            s.follow_enabled = enabled;
            if !enabled && s.behavior == Behavior::Follow {
                s.go_idle(now, rng);
            }
        })
    }

    /// Avanza la FSM hasta `now`. Devuelve `true` si cambió algo observable.
    pub fn step(&mut self, now: Ms, rng: &mut fastrand::Rng) -> bool {
        if self.is_suspended() {
            return false;
        }
        if self.sleeping {
            return self.mutate(|s| {
                if s.sleep_settle_at.is_some_and(|at| now >= at) {
                    s.sleep_settle_at = None;
                    s.sleep_resting = true;
                }
            });
        }
        self.mutate(|s| {
            if let Some(settle) = s.timers.settle_at {
                if now >= settle {
                    s.timers.settle_at = None;
                    s.expression = Expression::Resting;
                }
            }
            if now >= s.timers.deadline {
                s.on_deadline(now, rng);
            }
        })
    }

    // ---- internos ----------------------------------------------------------------------

    /// Ejecuta `f` y, si el estado observable cambió, incrementa la versión.
    fn mutate(&mut self, f: impl FnOnce(&mut Self)) -> bool {
        let before = self.view();
        f(self);
        let after = self.view();
        let changed = before != after;
        if changed {
            self.version += 1;
        }
        changed
    }

    fn sync_suspension(&mut self, now: Ms) {
        // Los plazos propios se detienen si no corre (pausa, oculta, suprimida) o si duerme.
        let halted = self.is_suspended() || self.sleeping;
        match (halted, self.frozen) {
            (true, None) => {
                let rem = self.timers.deadline.saturating_sub(now);
                let settle = self.timers.settle_at.map(|s| s.saturating_sub(now));
                self.frozen = Some((rem, settle));
            }
            (false, Some((rem, settle))) => {
                self.timers.deadline = now + rem;
                self.timers.settle_at = settle.map(|s| now + s);
                self.frozen = None;
            }
            _ => {}
        }
        // Reloj de presentación del sueño (sleepy → resting): corre solo durmiendo y corriendo.
        if self.sleeping && !self.is_suspended() {
            if let Some(rem) = self.sleep_settle_rem.take() {
                self.sleep_settle_at = Some(now + rem);
            }
        } else if let Some(at) = self.sleep_settle_at.take() {
            self.sleep_settle_rem = Some(at.saturating_sub(now));
        }
    }

    fn on_deadline(&mut self, now: Ms, rng: &mut fastrand::Rng) {
        match self.behavior {
            Behavior::Idle => self.decide_from_idle(now, rng),
            Behavior::Follow => {
                self.last_follow_end = Some(now);
                self.go_idle(now, rng);
            }
            Behavior::Rest => self.go_idle(now, rng),
            Behavior::Reaction => self.end_reaction(now, rng),
        }
    }

    fn go_idle(&mut self, now: Ms, rng: &mut fastrand::Rng) {
        self.behavior = Behavior::Idle;
        self.expression = Expression::Idle;
        let idle = range(rng, self.t.idle);
        self.timers = Timers { deadline: now + idle, settle_at: None };
        if self.frozen.is_some() {
            // suspendido (pausa/oculto): al reanudar debe correr el plazo de Idle, no el del Follow cancelado
            self.frozen = Some((idle, None));
        }
        self.resume = None;
    }

    fn off_cooldown(last_end: Option<Ms>, now: Ms, cooldown: Ms) -> bool {
        last_end.is_none_or(|e| now.saturating_sub(e) >= cooldown)
    }

    /// Tras Idle se elige el siguiente comportamiento con pesos; si nada es elegible
    /// (cooldowns / seguimiento desactivado) se permanece en Idle con un nuevo plazo.
    fn decide_from_idle(&mut self, now: Ms, rng: &mut fastrand::Rng) {
        let can_follow =
            self.follow_enabled && Self::off_cooldown(self.last_follow_end, now, self.t.follow_cooldown);
        let can_react = Self::off_cooldown(self.last_reaction_end, now, self.t.reaction_cooldown);
        let (w_follow, w_rest, w_react) = self.t.weights;
        let wf = if can_follow { w_follow } else { 0 };
        let wa = if can_react { w_react } else { 0 };
        let total = wf + w_rest + wa;
        if total == 0 {
            self.timers.deadline = now + range(rng, self.t.idle);
            return;
        }
        let roll = rng.u32(0..total);
        if roll < wf {
            self.enter(Behavior::Follow, now, rng);
        } else if roll < wf + w_rest {
            self.enter(Behavior::Rest, now, rng);
        } else {
            self.enter(Behavior::Reaction, now, rng);
        }
    }

    fn enter(&mut self, to: Behavior, now: Ms, rng: &mut fastrand::Rng) {
        if !can_transition(self.behavior, to) {
            return; // transición inválida: se ignora (estado intacto)
        }
        match to {
            Behavior::Follow => {
                self.behavior = to;
                self.expression = Expression::Curious;
                self.timers = Timers { deadline: now + range(rng, self.t.follow), settle_at: None };
            }
            Behavior::Rest => {
                self.behavior = to;
                self.expression = Expression::Sleepy;
                self.timers = Timers {
                    deadline: now + range(rng, self.t.rest),
                    settle_at: Some(now + self.t.rest_settle),
                };
            }
            Behavior::Reaction => {
                self.resume = Some(Resume {
                    behavior: self.behavior,
                    remaining: self.timers.deadline.saturating_sub(now),
                    settled: self.behavior == Behavior::Rest && self.timers.settle_at.is_none(),
                });
                self.behavior = to;
                self.expression = self.pick_reaction(rng);
                self.last_reaction_expr = Some(self.expression);
                self.timers = Timers { deadline: now + range(rng, self.t.reaction), settle_at: None };
            }
            Behavior::Idle => self.go_idle(now, rng),
        }
    }

    fn pick_reaction(&self, rng: &mut fastrand::Rng) -> Expression {
        let pool: Vec<Expression> = REACTION_EXPRESSIONS
            .iter()
            .copied()
            .filter(|e| Some(*e) != self.last_reaction_expr)
            .collect();
        pool[rng.usize(0..pool.len())]
    }

    /// Fin de la reacción: vuelve al comportamiento anterior con el tiempo que le quedaba.
    fn end_reaction(&mut self, now: Ms, rng: &mut fastrand::Rng) {
        self.last_reaction_end = Some(now);
        match self.resume.take() {
            Some(Resume { behavior: Behavior::Rest, remaining, settled }) => {
                self.behavior = Behavior::Rest;
                self.timers = Timers { deadline: now + remaining.max(1), settle_at: None };
                self.expression = if settled { Expression::Resting } else { Expression::Sleepy };
                if !settled {
                    self.timers.settle_at = Some(now + self.t.rest_settle.min(remaining));
                }
            }
            Some(Resume { remaining, .. }) => {
                self.behavior = Behavior::Idle;
                self.expression = Expression::Idle;
                self.timers = Timers { deadline: now + remaining.max(1), settle_at: None };
            }
            None => self.go_idle(now, rng),
        }
    }
}

fn range(rng: &mut fastrand::Rng, (lo, hi): (Ms, Ms)) -> Ms {
    if hi <= lo {
        lo
    } else {
        rng.u64(lo..=hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(ms: Ms) -> (Ms, Ms) {
        (ms, ms)
    }

    fn timings() -> Timings {
        Timings {
            idle: fixed(1000),
            follow: fixed(500),
            rest: fixed(2000),
            reaction: fixed(300),
            rest_settle: 400,
            follow_cooldown: 5000,
            reaction_cooldown: 3000,
            weights: (40, 25, 35),
        }
    }

    fn rng() -> fastrand::Rng {
        fastrand::Rng::with_seed(7)
    }

    #[test]
    fn transition_table_valid_and_invalid() {
        use Behavior::*;
        for (a, b) in [(Idle, Follow), (Idle, Rest), (Idle, Reaction), (Follow, Idle), (Rest, Idle), (Rest, Reaction), (Reaction, Idle), (Reaction, Rest)] {
            assert!(can_transition(a, b), "{a:?}->{b:?}");
        }
        for (a, b) in [(Follow, Rest), (Follow, Reaction), (Rest, Follow), (Reaction, Follow), (Idle, Idle), (Follow, Follow)] {
            assert!(!can_transition(a, b), "{a:?}->{b:?}");
        }
    }

    #[test]
    fn invalid_transition_is_ignored() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Follow, 0, &mut r);
        let v = f.view();
        f.enter(Behavior::Rest, 10, &mut r); // Follow→Rest no es válido
        assert_eq!(f.view().behavior, v.behavior);
        assert_eq!(f.view().expression, Expression::Curious);
    }

    #[test]
    fn starts_in_idle_and_stays_until_deadline() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        assert_eq!(f.view().behavior, Behavior::Idle);
        assert!(!f.step(999, &mut r));
        assert_eq!(f.next_wakeup(), Some(1000));
    }

    #[test]
    fn behavior_and_expression_are_separate_and_mapped() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Follow, 0, &mut r);
        assert_eq!((f.view().behavior, f.view().expression), (Behavior::Follow, Expression::Curious));
        f.go_idle(10, &mut r);
        f.enter(Behavior::Rest, 10, &mut r);
        assert_eq!((f.view().behavior, f.view().expression), (Behavior::Rest, Expression::Sleepy));
        f.step(10 + 400, &mut r);
        assert_eq!(f.view().expression, Expression::Resting);
    }

    #[test]
    fn follow_ends_after_duration_and_returns_go_idle() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Follow, 0, &mut r);
        assert!(!f.step(499, &mut r));
        assert!(f.step(500, &mut r));
        assert_eq!(f.view().behavior, Behavior::Idle);
    }

    #[test]
    fn follow_cooldown_blocks_immediate_refollow() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Follow, 0, &mut r);
        f.step(500, &mut r); // fin de Follow, last_follow_end = 500
        // con el cooldown activo, ninguna decisión debe producir Follow durante 5 s
        let mut now = 500;
        while now < 500 + 5000 - 50 {
            now += 50;
            f.step(now, &mut r);
            assert_ne!(f.behavior(), Behavior::Follow, "Follow durante cooldown en t={now}");
        }
    }

    #[test]
    fn follow_never_chosen_when_disabled() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.set_follow_enabled(0, &mut r, false);
        let mut now = 0;
        for _ in 0..20_000 {
            now += 50;
            f.step(now, &mut r);
            assert_ne!(f.behavior(), Behavior::Follow);
        }
    }

    #[test]
    fn disabling_follow_cancels_active_follow() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Follow, 0, &mut r);
        assert!(f.set_follow_enabled(100, &mut r, false));
        assert_eq!(f.behavior(), Behavior::Idle);
    }

    #[test]
    fn reaction_returns_to_previous_behavior_with_remaining_time() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        // Idle con plazo 1000; reacción a t=400 (quedan 600 ms) que dura 300
        f.enter(Behavior::Reaction, 400, &mut r);
        assert_eq!(f.behavior(), Behavior::Reaction);
        assert!(REACTION_EXPRESSIONS.contains(&f.view().expression));
        assert!(f.step(700, &mut r));
        assert_eq!(f.behavior(), Behavior::Idle);
        assert_eq!(f.next_wakeup(), Some(700 + 600));
    }

    #[test]
    fn reaction_from_rest_restores_rest_and_settled_expression() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Rest, 0, &mut r);
        f.step(400, &mut r); // ya asentada → resting
        f.enter(Behavior::Reaction, 1000, &mut r);
        f.step(1300, &mut r);
        assert_eq!((f.behavior(), f.view().expression), (Behavior::Rest, Expression::Resting));
        assert_eq!(f.next_wakeup(), Some(1300 + 1000));
    }

    #[test]
    fn consecutive_reactions_never_repeat_expression() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        let mut last = None;
        let mut now = 0;
        for _ in 0..30 {
            f.enter(Behavior::Reaction, now, &mut r);
            let e = f.view().expression;
            assert_ne!(Some(e), last);
            last = Some(e);
            now += 300;
            f.step(now, &mut r);
            now += 10;
            f.go_idle(now, &mut r);
        }
    }

    #[test]
    fn reaction_cooldown_prevents_chaining() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.set_follow_enabled(0, &mut r, false);
        f.enter(Behavior::Reaction, 0, &mut r);
        f.step(300, &mut r); // termina reacción en t=300
        let mut now = 300;
        let mut reactions_in_cooldown = 0;
        while now < 300 + 3000 {
            now += 50;
            f.step(now, &mut r);
            if f.behavior() == Behavior::Reaction {
                reactions_in_cooldown += 1;
            }
        }
        assert_eq!(reactions_in_cooldown, 0);
    }

    #[test]
    fn pause_freezes_timers_and_resume_continues_with_remaining() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        assert!(f.set_paused(400, true)); // quedan 600
        assert_eq!(f.next_wakeup(), None, "pausado: el motor puede dormir indefinidamente");
        assert!(!f.step(10_000, &mut r), "pausado no avanza");
        assert_eq!(f.behavior(), Behavior::Idle);
        assert!(f.set_paused(10_000, false));
        assert_eq!(f.next_wakeup(), Some(10_600));
    }

    #[test]
    fn follow_change_while_paused_replaces_the_frozen_follow_deadline() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Follow, 0, &mut r); // Follow dura 500
        f.set_paused(100, true); // quedan 400 congelados
        assert!(f.set_follow_enabled(150, &mut r, false), "desactivar cancela el Follow aun en pausa");
        assert_eq!(f.behavior(), Behavior::Idle);
        assert!(f.view().paused && !f.view().follow_enabled);
        f.set_paused(9000, false);
        // corre el plazo de Idle (1000), no los 400 ms sobrantes del Follow cancelado
        assert_eq!(f.next_wakeup(), Some(9000 + 1000));
    }

    #[test]
    fn follow_flag_is_part_of_the_observable_view() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        let v0 = f.view().version;
        assert!(f.view().follow_enabled);
        assert!(f.toggle_follow(10, &mut r));
        assert!(!f.view().follow_enabled && f.view().version == v0 + 1);
        assert!(f.toggle_follow(20, &mut r));
        assert!(f.view().follow_enabled);
        assert!(f.toggle_paused(30) && f.view().paused);
        assert!(f.toggle_paused(40) && !f.view().paused);
    }

    #[test]
    fn hidden_and_paused_are_independent_suspensions() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.set_paused(100, true);
        f.set_hidden(200, true);
        f.set_paused(300, false);
        assert!(f.is_suspended(), "sigue oculto");
        assert_eq!(f.next_wakeup(), None);
        f.set_hidden(5000, false);
        assert_eq!(f.next_wakeup(), Some(5000 + 900));
    }

    #[test]
    fn rest_settle_timer_survives_pause() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Rest, 0, &mut r);
        f.set_paused(100, true); // faltan 300 para asentarse
        f.set_paused(5000, false);
        assert!(!f.step(5299, &mut r));
        assert!(f.step(5300, &mut r));
        assert_eq!(f.view().expression, Expression::Resting);
    }

    #[test]
    fn version_increments_only_on_observable_change() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        let v0 = f.view().version;
        f.step(10, &mut r);
        assert_eq!(f.view().version, v0);
        f.set_paused(20, true);
        assert_eq!(f.view().version, v0 + 1);
        assert!(!f.set_paused(30, true), "sin cambio: no sube la versión");
        assert_eq!(f.view().version, v0 + 1);
    }

    #[test]
    fn long_simulation_visits_all_behaviors_without_invalid_jumps() {
        let mut r = fastrand::Rng::with_seed(42);
        let t = Timings { follow_cooldown: 1000, reaction_cooldown: 1000, ..Timings::default() };
        let mut f = Fsm::new(t, 0, &mut r);
        let mut seen = std::collections::HashSet::new();
        let mut prev = f.behavior();
        let mut now = 0;
        for _ in 0..200_000 {
            now += 100;
            f.step(now, &mut r);
            let cur = f.behavior();
            if cur != prev {
                assert!(can_transition(prev, cur), "salto inválido {prev:?}->{cur:?}");
                prev = cur;
            }
            seen.insert(format!("{cur:?}"));
        }
        assert_eq!(seen.len(), 4, "{seen:?}");
    }

    #[test]
    fn decision_weights_are_respected() {
        // Cooldowns nulos para que los tres comportamientos sean siempre elegibles.
        let t = Timings {
            idle: fixed(100),
            follow: fixed(100),
            rest: fixed(100),
            reaction: fixed(100),
            rest_settle: 50,
            follow_cooldown: 0,
            reaction_cooldown: 0,
            weights: (50, 25, 25),
        };
        let mut r = fastrand::Rng::with_seed(99);
        let mut f = Fsm::new(t, 0, &mut r);
        let (mut follow, mut rest, mut react) = (0u32, 0u32, 0u32);
        let (mut now, mut prev) = (0, f.behavior());
        for _ in 0..60_000 {
            now += 10;
            f.step(now, &mut r);
            let cur = f.behavior();
            if cur != prev {
                match cur {
                    Behavior::Follow => follow += 1,
                    Behavior::Rest => rest += 1,
                    Behavior::Reaction => react += 1,
                    Behavior::Idle => {}
                }
                prev = cur;
            }
        }
        let total = (follow + rest + react) as f64;
        assert!(total > 500.0, "pocas decisiones: {total}");
        let share = |n: u32| n as f64 / total;
        assert!((share(follow) - 0.50).abs() < 0.06, "follow {:.2}", share(follow));
        assert!((share(rest) - 0.25).abs() < 0.06, "rest {:.2}", share(rest));
        assert!((share(react) - 0.25).abs() < 0.06, "reaction {:.2} (follow={follow} rest={rest} react={react})", share(react));
    }

    // ---- E1b: infraestructura de supresión (suppressed / dnd_override / sleeping) ----

    /// FSM en t=0 con las banderas pedidas aplicadas por la API pública.
    fn with_flags(hidden: bool, paused: bool, suppressed: bool, sleeping: bool) -> (Fsm, fastrand::Rng) {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.set_suppressed(0, suppressed);
        f.set_sleeping(0, sleeping);
        f.set_paused(0, paused);
        f.set_hidden(0, hidden);
        (f, r)
    }

    #[test]
    fn pure_visibility_functions_cover_the_whole_space() {
        for bits in 0..32u8 {
            let (hidden, paused, suppressed, dnd_override, sleeping) =
                (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0, bits & 16 != 0);
            let active = suppressed && !dnd_override;
            assert_eq!(suppression_active(suppressed, dnd_override), active);
            assert_eq!(visible(hidden, suppressed, dnd_override), !hidden && !active);
            assert_eq!(running(hidden, paused, suppressed, dnd_override), !hidden && !active && !paused);
            assert_eq!(
                messages_allowed(hidden, paused, suppressed, dnd_override, sleeping),
                !hidden && !active && !paused && !sleeping
            );
            // equivalencia con v0.1.0: sin supresión en vigor, "no corre" == paused || hidden
            if !active {
                assert_eq!(!running(hidden, paused, suppressed, dnd_override), paused || hidden);
            }
        }
    }

    #[test]
    fn is_suspended_equals_paused_or_hidden_when_not_suppressed_over_the_whole_space() {
        for bits in 0..8u8 {
            let (hidden, paused, sleeping) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
            let (f, _) = with_flags(hidden, paused, false, sleeping);
            assert_eq!(f.is_suspended(), paused || hidden, "hidden={hidden} paused={paused} sleeping={sleeping}");
            assert_eq!(f.is_running(), !(paused || hidden));
        }
        // también tras un episodio completo con anulación (override limpio al terminar)
        let (mut f, _) = with_flags(false, false, true, false);
        f.set_dnd_override(10);
        f.set_suppressed(20, false);
        assert!(!f.is_suspended());
    }

    /// Las 16 combinaciones hidden × paused × suppressed × sleeping (sin anulación) y sus invariantes (§5.1).
    #[test]
    fn matrix_of_16_combinations_keeps_every_invariant() {
        for bits in 0..16u8 {
            let (hidden, paused, suppressed, sleeping) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            let tag = format!("hidden={hidden} paused={paused} suppressed={suppressed} sleeping={sleeping}");
            let (mut f, mut r) = with_flags(hidden, paused, suppressed, sleeping);
            let v = f.view();

            // View.hidden / View.paused: exactamente los de v0.1.0
            assert_eq!((v.hidden, v.paused), (hidden, paused), "{tag}");
            assert_eq!((v.suppressed, v.sleeping), (suppressed, sleeping), "{tag}");
            // visibilidad, ejecución y mensajes
            let visible_ = !hidden && !suppressed;
            let running_ = visible_ && !paused;
            assert_eq!(f.is_visible(), visible_, "{tag}");
            assert_eq!(f.is_running(), running_, "{tag}");
            assert_eq!(f.is_suspended(), !running_, "{tag}");
            assert_eq!(f.messages_allowed(), running_ && !sleeping, "{tag}");
            // expresión: durmiendo, sleepy (aún sin pasar rest_settle) como Rest; si no, la de la FSM
            if sleeping {
                assert_eq!((v.behavior, v.expression), (Behavior::Rest, Expression::Sleepy), "{tag}");
            } else {
                assert_eq!((v.behavior, v.expression), (Behavior::Idle, Expression::Idle), "{tag}");
            }
            // próximo despertar: ninguno si no corre; durmiendo solo el paso sleepy→resting; si no, el plazo de Idle
            let wake = f.next_wakeup();
            match (running_, sleeping) {
                (false, _) => assert_eq!(wake, None, "{tag}"),
                (true, true) => assert_eq!(wake, Some(400), "{tag}"),
                (true, false) => assert_eq!(wake, Some(1000), "{tag}"),
            }
            // avance del reloj muy por encima de todos los plazos
            let version = f.view().version;
            let changed = f.step(1_000_000, &mut r);
            let after = f.view();
            match (running_, sleeping) {
                (false, _) => {
                    assert!(!changed && after == v, "no corre → nada cambia: {tag}");
                    assert_eq!(after.version, version, "{tag}");
                }
                (true, true) => {
                    // Sleep es estable: solo pasa de sleepy a resting y NO sale por plazos de Rest
                    assert_eq!((after.behavior, after.expression), (Behavior::Rest, Expression::Resting), "{tag}");
                    assert!(!f.step(2_000_000, &mut r), "sin más transiciones: {tag}");
                    assert_eq!(f.next_wakeup(), None, "{tag}");
                }
                (true, false) => assert!(changed && after.behavior != Behavior::Idle, "decide al vencer Idle: {tag}"),
            }
        }
    }

    /// Recuperación (§5.1): al quedar libres todas las causas, los plazos congelados se rebasan al instante de
    /// la recuperación y la FSM vuelve a lo que estaba, sin haber destruido nada.
    #[test]
    fn matrix_recovery_rebases_the_frozen_deadlines() {
        for bits in 1..16u8 {
            let (hidden, paused, suppressed, sleeping) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            let tag = format!("hidden={hidden} paused={paused} suppressed={suppressed} sleeping={sleeping}");
            let (mut f, mut r) = with_flags(hidden, paused, suppressed, sleeping);
            f.step(500_000, &mut r); // el tiempo pasa mientras tanto (solo cambia si corre y no duerme)
            let t = 600_000;
            f.set_suppressed(t, false);
            f.set_sleeping(t, false);
            f.set_paused(t, false);
            f.set_hidden(t, false);
            let v = f.view();
            assert!(f.is_running() && !v.sleeping && !v.suppressed, "{tag}");
            assert_eq!((v.behavior, v.expression), (Behavior::Idle, Expression::Idle), "{tag}");
            assert_eq!(f.next_wakeup(), Some(t + 1000), "el Idle congelado (1000 ms) se rebasa a la recuperación: {tag}");
        }
    }

    // §5.2 casos aparte

    #[test]
    fn case1_end_of_suppression_after_the_idle_threshold_reappears_asleep() {
        let (mut f, _) = with_flags(false, false, true, false);
        f.set_sleeping(100, true); // el motor recalcula el sueño ANTES de mostrar
        assert!(!f.is_visible() && f.view().sleeping);
        f.set_suppressed(200, false);
        let v = f.view();
        assert!(f.is_visible() && f.is_running() && v.sleeping && !v.suppressed);
        assert_eq!((v.behavior, v.expression), (Behavior::Rest, Expression::Sleepy));
        assert!(!f.messages_allowed());
    }

    #[test]
    fn case2_resuming_after_the_threshold_recomputes_sleep_immediately() {
        let (mut f, _) = with_flags(false, true, false, false);
        f.set_sleeping(1000, true); // reconsulta al volver a running...
        f.set_paused(1000, false); // ...y se reanuda ya dormida
        let v = f.view();
        assert!(!v.paused && v.sleeping);
        assert_eq!(v.behavior, Behavior::Rest);
        assert_eq!(f.next_wakeup(), Some(1400), "el reloj sleepy→resting corre desde la reanudación");
    }

    #[test]
    fn case3_show_anyway_during_suppression_keeps_dnd_on_for_the_episode() {
        let (mut f, _) = with_flags(false, false, true, false);
        assert!(!f.is_visible());
        assert!(f.set_dnd_override(10));
        let v = f.view();
        assert!(f.is_visible() && f.is_running() && !v.suppressed && !v.hidden);
        assert!(f.suppressed && f.dnd_override, "el episodio sigue marcado; solo se anula");
    }

    #[test]
    fn case4_end_of_episode_clears_the_override_and_a_new_episode_suppresses_again() {
        let (mut f, _) = with_flags(false, false, true, false);
        f.set_dnd_override(10);
        f.set_suppressed(20, false);
        assert!(!f.dnd_override && !f.suppressed && f.is_visible());
        f.set_suppressed(30, true);
        assert!(f.view().suppressed && !f.is_visible(), "episodio nuevo: vuelve a suprimir");
    }

    #[test]
    fn case5_hiding_while_asleep_then_showing_recomputes() {
        let (mut f, mut r) = with_flags(false, false, false, true);
        f.set_hidden(50, true);
        assert!(!f.is_visible() && f.view().sleeping && f.next_wakeup().is_none());
        assert!(!f.step(10_000, &mut r), "oculta: nada avanza");
        f.set_hidden(20_000, false);
        let v = f.view();
        assert!(v.sleeping && !v.hidden && f.is_running());
        assert_eq!(f.next_wakeup(), Some(20_350), "sleepy→resting: quedaban 350 ms al ocultar");
    }

    #[test]
    fn case6_resuming_with_suppression_still_active_does_not_show() {
        let (mut f, _) = with_flags(false, true, true, false);
        f.set_paused(100, false);
        assert!(!f.is_visible() && !f.is_running() && f.view().suppressed, "sigue suprimida");
        assert!(f.next_wakeup().is_none());
        // salvo anulación
        f.set_dnd_override(200);
        assert!(f.is_visible() && f.is_running());
    }

    #[test]
    fn case7_override_does_not_clear_a_manual_pause() {
        let (mut f, _) = with_flags(false, true, true, false);
        f.set_dnd_override(100);
        let v = f.view();
        assert!(v.paused && f.is_visible() && !f.is_running(), "visible pero pausada");
    }

    #[test]
    fn case8_show_with_hidden_and_active_suppression_also_overrides_the_episode() {
        // "Mostrar" (set_hidden(false)) con hidden=true y supresión en vigor
        let (mut f, _) = with_flags(true, true, true, false);
        f.set_hidden(100, false);
        let v = f.view();
        assert!(!v.hidden && v.paused, "quita hidden y deja paused intacto");
        assert!(f.is_visible() && !v.suppressed && f.dnd_override, "anulación del episodio activada");
        assert!(f.suppressed, "DND automático no se desactiva");
        f.set_suppressed(200, false); // fin del episodio
        assert!(!f.dnd_override && f.is_visible());
        // "Mostrar de todos modos" parte del mismo estado y deja lo mismo
        let (mut g, _) = with_flags(true, true, true, false);
        assert!(g.set_dnd_override(100));
        assert_eq!((g.view().hidden, g.view().paused, g.view().suppressed), (false, true, false));
        assert!(g.dnd_override);
    }

    #[test]
    fn case8b_show_with_hidden_and_no_suppression_only_clears_hidden() {
        let (mut f, _) = with_flags(true, false, false, false);
        assert!(!f.set_dnd_override(50), "'Mostrar de todos modos' no existe sin supresión: no hace nada");
        assert!(f.view().hidden && !f.dnd_override);
        f.set_hidden(100, false);
        assert!(!f.view().hidden && !f.dnd_override && !f.suppressed);
    }

    #[test]
    fn case9_turning_dnd_off_during_an_episode_unsuppresses_immediately() {
        let (mut f, _) = with_flags(false, false, true, false);
        assert!(f.set_suppressed(10, false));
        assert!(f.is_visible() && !f.view().suppressed);
        assert!(!f.set_suppressed(11, false), "idempotente: sin cambio ni versión nueva");
    }

    #[test]
    fn unused_flags_do_not_bump_the_version() {
        let (mut f, _) = with_flags(false, false, false, false);
        let v0 = f.view().version;
        assert!(!f.set_suppressed(1, false) && !f.set_sleeping(1, false) && !f.set_dnd_override(1));
        assert_eq!(f.view().version, v0);
    }

    // §4.2 Sleep estable

    #[test]
    fn sleep_holds_resting_and_does_not_leave_by_rest_deadlines() {
        let (mut f, mut r) = with_flags(false, false, false, false);
        f.set_sleeping(10, true);
        assert_eq!(f.view().expression, Expression::Sleepy);
        f.step(409, &mut r);
        assert_eq!(f.view().expression, Expression::Sleepy);
        f.step(410, &mut r);
        let v = f.view();
        assert_eq!((v.behavior, v.expression, v.sleeping), (Behavior::Rest, Expression::Resting, true));
        let version = v.version;
        // muy por encima del plazo de Rest (2000) y de Idle (1000): sigue igual
        assert!(!f.step(10_000_000, &mut r));
        assert_eq!(f.view().version, version);
        assert_eq!(f.behavior(), Behavior::Rest);
    }

    #[test]
    fn sleep_does_not_consume_the_rng() {
        let (mut f, mut r) = with_flags(false, false, false, true);
        let mut twin = r.clone();
        for t in [500, 5_000, 50_000, 5_000_000] {
            f.step(t, &mut r);
        }
        assert_eq!(r.u64(..), twin.u64(..), "dormida no sortea nada");
    }

    #[test]
    fn waking_restores_the_real_behavior_with_its_remaining_time() {
        let mut r = rng();
        let mut f = Fsm::new(timings(), 0, &mut r);
        f.enter(Behavior::Follow, 0, &mut r); // Follow de 500 ms
        f.set_sleeping(100, true); // quedaban 400
        assert_eq!(f.behavior(), Behavior::Rest, "efectivo: Rest");
        f.step(100_000, &mut r);
        assert_eq!(f.behavior(), Behavior::Rest);
        f.set_sleeping(9_000, false);
        assert_eq!(f.behavior(), Behavior::Follow);
        assert_eq!(f.next_wakeup(), Some(9_400));
        assert!(!f.view().sleeping);
    }

    #[test]
    fn pausing_while_asleep_freezes_the_sleepy_to_resting_clock() {
        let (mut f, mut r) = with_flags(false, false, false, false);
        f.set_sleeping(0, true); // resting a los 400
        f.set_paused(100, true); // quedaban 300
        assert!(f.next_wakeup().is_none());
        f.set_paused(10_000, false);
        assert_eq!(f.next_wakeup(), Some(10_300));
        assert!(!f.step(10_299, &mut r));
        assert!(f.step(10_300, &mut r));
        assert_eq!(f.view().expression, Expression::Resting);
    }

    #[test]
    fn view_json_keeps_the_v0_1_0_fields_and_adds_the_new_ones() {
        let (f, _) = with_flags(false, true, false, false);
        let j = serde_json::to_value(f.view()).unwrap();
        // campos de v0.1.0, con los mismos nombres y valores
        for k in ["version", "behavior", "expression", "paused", "hidden", "followEnabled", "message"] {
            assert!(j.get(k).is_some(), "falta {k}");
        }
        assert_eq!((j["paused"].as_bool(), j["hidden"].as_bool()), (Some(true), Some(false)));
        // campos nuevos, aditivos y falsos
        assert_eq!((j["suppressed"].as_bool(), j["sleeping"].as_bool()), (Some(false), Some(false)));
        assert_eq!(j.as_object().unwrap().len(), 9, "exactamente los 7 de v0.1.0 + suppressed + sleeping");
    }
}
