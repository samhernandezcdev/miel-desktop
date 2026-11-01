//! Máquina de estados de comportamiento. Pura y determinista: el tiempo (`Ms`) y el azar
//! (`fastrand::Rng`, sembrable) se inyectan; no toca ventanas ni hilos.
//!
//! Distingue dos conceptos:
//! - `Behavior`: qué está haciendo Miel (Idle / Follow / Rest / Reaction).
//! - `Expression`: qué cara muestra (los 8 estados del rig). Se deriva del comportamiento.
//!
//! `paused` y `hidden` son banderas ortogonales que SUSPENDEN todo (prioridad máxima): congelan
//! los temporizadores y al reanudar se continúa con el tiempo restante.

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
    follow_enabled: bool,
    timers: Timers,
    /// Tiempo restante congelado mientras está suspendido (paused || hidden).
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
        View {
            version: self.version,
            behavior: self.behavior,
            expression: self.expression,
            paused: self.paused,
            hidden: self.hidden,
            follow_enabled: self.follow_enabled,
            message: None,
        }
    }

    pub fn behavior(&self) -> Behavior {
        self.behavior
    }

    pub fn is_suspended(&self) -> bool {
        self.paused || self.hidden
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
            s.hidden = hidden;
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
        let changed = (before.behavior, before.expression, before.paused, before.hidden, before.follow_enabled)
            != (after.behavior, after.expression, after.paused, after.hidden, after.follow_enabled);
        if changed {
            self.version += 1;
        }
        changed
    }

    fn sync_suspension(&mut self, now: Ms) {
        match (self.is_suspended(), self.frozen) {
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
}
