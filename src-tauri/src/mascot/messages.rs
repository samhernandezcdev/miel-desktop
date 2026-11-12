//! Mensajes locales de Miel: catálogo, selección con historial y planificador (`Talker`).
//!
//! Todo es puro y determinista (tiempo `Ms` y azar inyectados; reloj monotónico del motor).
//! Sin red, sin lectura del sistema: las frases no suponen conocer lo que hace la persona.
//!
//! Cuándo habla: cuando vence el cooldown, Miel espera al PRÓXIMO cambio de comportamiento y
//! entonces elige una frase coherente con él (p. ej. "Voy a descansar un ratito" al entrar en
//! Rest). Así no hacen falta temporizadores propios para "decidir hablar": el único plazo que el
//! motor debe vigilar es el fin de la burbuja visible.

use std::collections::VecDeque;

use serde::Serialize;

use super::state::{Behavior, Expression, Ms, View};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Presence,
    Calm,
    Curiosity,
    Mood,
    Energy,
    Movement,
    Rest,
    Thought,
    Playful,
}

/// Entrada del catálogo. `expression` es la cara que acompaña a la frase mientras es visible.
#[derive(Debug, Clone, Copy)]
pub struct Message {
    /// Identificador estable (no cambia aunque cambie el texto).
    pub id: &'static str,
    pub text: &'static str,
    pub category: Category,
    pub expression: Expression,
    /// Comportamientos durante los que puede decirse. En `Reaction` solo se elige si su
    /// expresión coincide con la de la reacción en curso (coherencia texto ↔ cara).
    pub behaviors: &'static [Behavior],
    /// Rango de duración visible (ms).
    pub duration: (Ms, Ms),
    pub weight: u32,
    /// No volver a decirla antes de este tiempo (ms).
    pub min_repeat: Ms,
}

const S: Ms = 1000;
const MIN_REPEAT: Ms = 60 * 60 * S;
const DUR: (Ms, Ms) = (4 * S, 6 * S);

use Behavior::{Follow, Idle, Reaction, Rest};
use Category::*;
use Expression as E;

const fn msg(
    id: &'static str,
    text: &'static str,
    category: Category,
    expression: Expression,
    behaviors: &'static [Behavior],
) -> Message {
    Message { id, text, category, expression, behaviors, duration: DUR, weight: 10, min_repeat: MIN_REPEAT }
}

/// Catálogo inicial. Tono: tierno, discreto, nada de regaños ni de "saber" qué hace la persona.
pub const CATALOG: &[Message] = &[
    msg("presence.here", "Aquí sigo.", Presence, E::Idle, &[Idle]),
    msg("presence.stay", "Me quedaré por aquí.", Presence, E::Idle, &[Idle]),
    msg("calm.quiet", "Qué día tan tranquilo.", Calm, E::Idle, &[Idle]),
    msg("curious.today", "¿Qué estaremos haciendo hoy?", Curiosity, E::Curious, &[Idle, Reaction]),
    msg("mood.nice", "¡Qué bonito día!", Mood, E::Happy, &[Idle, Reaction]),
    msg("energy.lots", "Hoy tengo mucha energía.", Energy, E::Happy, &[Idle, Reaction]),
    msg("vanity.wings", "¿Ya viste qué bonitas son mis alas?", Mood, E::Happy, &[Idle, Reaction]),
    msg("thought.flowers", "Mmm… estaba pensando en flores.", Thought, E::Thinking, &[Idle, Reaction]),
    msg("oops.distracted", "¡Uy, me distraje!", Thought, E::Surprised, &[Reaction]),
    msg("playful.buzz", "¡Bzz, bzz!", Playful, E::Talking, &[Idle]),
    msg("move.stroll", "Voy a dar una vueltecita.", Movement, E::Curious, &[Follow]),
    msg("move.company", "Te acompaño un ratito.", Movement, E::Curious, &[Follow]),
    msg("rest.little", "Voy a descansar un ratito.", Category::Rest, E::Sleepy, &[Rest]),
    msg("rest.break", "Creo que tomaré un descanso.", Category::Rest, E::Sleepy, &[Rest]),
    msg("rest.wings", "Mis alitas necesitan descansar.", Category::Rest, E::Sleepy, &[Rest]),
    msg("rest.sleepy", "Ya me está dando sueñito.", Category::Rest, E::Sleepy, &[Rest]),
];

/// Lo mínimo que React necesita; el resto de metadatos no sale de Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MessageView {
    pub id: &'static str,
    pub text: &'static str,
}

// ---- configuración ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct TalkerConfig {
    pub enabled: bool,
    /// Tiempo mínimo entre el FIN de un mensaje y el inicio del siguiente.
    pub cooldown: Ms,
    /// Retardo (mín, máx) antes del primer mensaje tras arrancar.
    pub initial_delay: (Ms, Ms),
    /// Cuántas frases recientes se excluyen de la selección.
    pub history_len: usize,
}

impl Default for TalkerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown: 12 * 60 * S,
            initial_delay: (2 * 60 * S, 4 * 60 * S),
            history_len: 6,
        }
    }
}

impl TalkerConfig {
    /// Límites razonables para valores que vengan de configuración local (Etapa 6+):
    /// cooldown 1 min – 24 h, historial 0 – (catálogo − 2).
    pub fn validated(mut self) -> Self {
        self.cooldown = self.cooldown.clamp(60 * S, 24 * 60 * 60 * S);
        let (a, b) = self.initial_delay;
        let a = a.min(24 * 60 * 60 * S);
        self.initial_delay = (a, b.clamp(a, 24 * 60 * 60 * S));
        self.history_len = self.history_len.min(CATALOG.len().saturating_sub(2));
        self
    }
}

// ---- selección -------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct Selector {
    history: VecDeque<&'static str>,
    last_shown: Vec<(&'static str, Ms)>,
}

impl Selector {
    /// Elige una frase elegible. Siempre excluye: otras conductas, expresión incoherente en
    /// Reaction y las que no cumplen su `min_repeat`. Prefiere además no repetir ninguna del
    /// historial reciente ni la categoría de la última; si eso deja la lista vacía se relajan, por
    /// este orden, la categoría y el historial (nunca repite la última frase ni rompe `min_repeat`).
    pub fn pick(
        &mut self,
        behavior: Behavior,
        expression: Expression,
        now: Ms,
        history_len: usize,
        rng: &mut fastrand::Rng,
    ) -> Option<&'static Message> {
        let last_category = self
            .history
            .back()
            .and_then(|id| CATALOG.iter().find(|m| m.id == *id))
            .map(|m| m.category);
        // nivel 0: historial + categoría · nivel 1: solo historial · nivel 2: solo "no la última"
        let eligible = |m: &&'static Message, level: u8| -> bool {
            m.behaviors.contains(&behavior)
                && (behavior != Behavior::Reaction || m.expression == expression)
                && self.last_shown.iter().find(|(id, _)| *id == m.id).is_none_or(|(_, t)| now.saturating_sub(*t) >= m.min_repeat)
                && match level {
                    0 => {
                        !self.history.iter().rev().take(history_len).any(|h| *h == m.id)
                            && Some(m.category) != last_category
                    }
                    1 => !self.history.iter().rev().take(history_len).any(|h| *h == m.id),
                    _ => self.history.back() != Some(&m.id),
                }
        };
        let mut pool: Vec<&'static Message> = Vec::new();
        for level in 0..=2 {
            pool = CATALOG.iter().filter(|m| eligible(m, level)).collect();
            if !pool.is_empty() {
                break;
            }
        }
        let total: u32 = pool.iter().map(|m| m.weight).sum();
        if total == 0 {
            return None;
        }
        let mut roll = rng.u32(0..total);
        let chosen = pool.iter().find(|m| {
            if roll < m.weight {
                true
            } else {
                roll -= m.weight;
                false
            }
        })?;
        self.history.push_back(chosen.id);
        while self.history.len() > CATALOG.len() {
            self.history.pop_front();
        }
        match self.last_shown.iter_mut().find(|(id, _)| *id == chosen.id) {
            Some(e) => e.1 = now,
            None => self.last_shown.push((chosen.id, now)),
        }
        Some(*chosen)
    }
}

// ---- planificador ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Showing {
    msg: &'static Message,
    until: Ms,
}

#[derive(Debug)]
pub struct Talker {
    cfg: TalkerConfig,
    selector: Selector,
    /// A partir de este instante puede empezar un mensaje (en el próximo cambio de comportamiento).
    next_due: Ms,
    /// Cooldown restante congelado mientras está suspendido.
    frozen: Option<Ms>,
    showing: Option<Showing>,
    last_behavior: Behavior,
    suspended: bool,
    version: u64,
}

impl Talker {
    pub fn new(cfg: TalkerConfig, now: Ms, rng: &mut fastrand::Rng) -> Self {
        let (a, b) = cfg.initial_delay;
        let delay = if b <= a { a } else { rng.u64(a..=b) };
        Self {
            cfg,
            selector: Selector::default(),
            next_due: now + delay,
            frozen: None,
            showing: None,
            last_behavior: Behavior::Idle,
            suspended: false,
            version: 0,
        }
    }

    /// Contador monótono de cambios observables (inicio / fin / cancelación de un mensaje).
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn message(&self) -> Option<MessageView> {
        self.showing.map(|s| MessageView { id: s.msg.id, text: s.msg.text })
    }

    /// Expresión que acompaña al mensaje visible (sustituye a la del comportamiento).
    pub fn expression_override(&self) -> Option<Expression> {
        self.showing.map(|s| s.msg.expression)
    }

    /// Único plazo que el motor debe vigilar: el fin del mensaje visible.
    pub fn next_wakeup(&self) -> Option<Ms> {
        if self.suspended {
            return None;
        }
        self.showing.map(|s| s.until)
    }

    /// Pausa u ocultación: cancela el mensaje visible (no se retoma) y congela el cooldown.
    pub fn set_suspended(&mut self, now: Ms, suspended: bool) -> bool {
        if suspended == self.suspended {
            return false;
        }
        self.suspended = suspended;
        let mut changed = false;
        if suspended {
            self.frozen = Some(self.next_due.saturating_sub(now));
            changed = self.cancel();
        } else if let Some(rem) = self.frozen.take() {
            self.next_due = now + rem;
        }
        changed
    }

    /// Impide hablar durante al menos `ms` más (E5: respiro tras despertar). No acorta un cooldown mayor y funciona
    /// igual con el hablante suspendido (el plazo congelado se alarga) que activo.
    pub fn hold_off(&mut self, now: Ms, ms: Ms) {
        match self.frozen.as_mut() {
            Some(rem) => *rem = (*rem).max(ms),
            None => self.next_due = self.next_due.max(now + ms),
        }
    }

    /// Termina el mensaje cuando vence su duración.
    pub fn step(&mut self, now: Ms) -> bool {
        match self.showing {
            Some(s) if !self.suspended && now >= s.until => self.cancel(),
            _ => false,
        }
    }

    /// Debe llamarse tras cada actualización de la FSM. Si cambió el comportamiento:
    /// cancela un mensaje que ya no encaja y, si venció el cooldown, puede iniciar uno nuevo.
    pub fn observe(&mut self, now: Ms, fsm: &View, rng: &mut fastrand::Rng) -> bool {
        let behavior_changed = fsm.behavior != self.last_behavior;
        self.last_behavior = fsm.behavior;
        if self.suspended || !behavior_changed {
            return false;
        }
        let mut changed = false;
        if let Some(s) = self.showing {
            if !s.msg.behaviors.contains(&fsm.behavior) {
                changed |= self.cancel();
            }
        }
        if self.showing.is_none() && self.cfg.enabled && now >= self.next_due {
            if let Some(m) =
                self.selector.pick(fsm.behavior, fsm.expression, now, self.cfg.history_len, rng)
            {
                let dur = if m.duration.1 <= m.duration.0 { m.duration.0 } else { rng.u64(m.duration.0..=m.duration.1) };
                self.showing = Some(Showing { msg: m, until: now + dur });
                // el cooldown cuenta desde el FIN del mensaje → nunca se solapan
                self.next_due = now + dur + self.cfg.cooldown;
                self.version += 1;
                changed = true;
            }
        }
        changed
    }

    fn cancel(&mut self) -> bool {
        if self.showing.take().is_some() {
            self.version += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(b: Behavior, e: Expression) -> View {
        View { version: 0, behavior: b, expression: e, paused: false, hidden: false, follow_enabled: true, message: None, suppressed: false, sleeping: false }
    }

    fn cfg(cooldown: Ms) -> TalkerConfig {
        TalkerConfig { enabled: true, cooldown, initial_delay: (1000, 1000), history_len: 6 }
    }

    fn rng() -> fastrand::Rng {
        fastrand::Rng::with_seed(11)
    }

    // ---- catálogo ----

    #[test]
    fn catalog_is_well_formed() {
        let mut ids = std::collections::HashSet::new();
        for m in CATALOG {
            assert!(ids.insert(m.id), "id duplicado {}", m.id);
            assert!(!m.text.is_empty() && m.text.chars().count() <= 48, "{}", m.id);
            assert!(!m.behaviors.is_empty());
            assert!(m.duration.0 >= 3000 && m.duration.1 <= 7000 && m.duration.0 <= m.duration.1);
            assert!(m.weight > 0);
        }
        assert!(CATALOG.len() >= 14);
    }

    #[test]
    fn catalog_expressions_are_consistent_with_behaviors() {
        for m in CATALOG {
            for b in m.behaviors {
                match b {
                    Behavior::Rest => assert!(matches!(m.expression, E::Sleepy | E::Resting), "{}", m.id),
                    Behavior::Follow => assert_eq!(m.expression, E::Curious, "{}", m.id),
                    Behavior::Reaction => assert!(
                        matches!(m.expression, E::Happy | E::Curious | E::Thinking | E::Surprised),
                        "{}",
                        m.id
                    ),
                    Behavior::Idle => {}
                }
            }
            if m.expression == E::Talking {
                assert_eq!(m.behaviors, &[Idle], "talking solo en Idle: {}", m.id);
            }
        }
    }

    #[test]
    fn every_reaction_expression_has_at_least_one_message() {
        for e in [E::Happy, E::Curious, E::Thinking, E::Surprised] {
            assert!(
                CATALOG.iter().any(|m| m.behaviors.contains(&Reaction) && m.expression == e),
                "sin frase para {e:?}"
            );
        }
        for b in [Idle, Follow, Rest] {
            assert!(CATALOG.iter().filter(|m| m.behaviors.contains(&b)).count() >= 2);
        }
    }

    // ---- selección ----

    #[test]
    fn pick_only_returns_messages_for_the_behavior() {
        let mut r = rng();
        let mut sel = Selector::default();
        for (i, b) in [Idle, Follow, Rest].into_iter().cycle().take(60).enumerate() {
            let m = sel.pick(b, E::Idle, i as Ms * 10_000_000, 3, &mut r).expect("hay frase");
            assert!(m.behaviors.contains(&b), "{} en {b:?}", m.id);
        }
    }

    #[test]
    fn reaction_picks_message_matching_the_reaction_expression() {
        let mut r = rng();
        let mut sel = Selector::default();
        for (i, e) in [E::Happy, E::Curious, E::Thinking, E::Surprised].into_iter().cycle().take(40).enumerate() {
            let m = sel.pick(Reaction, e, i as Ms * 10_000_000, 2, &mut r).expect("hay frase");
            assert_eq!(m.expression, e, "{}", m.id);
        }
    }

    #[test]
    fn history_prevents_repeats_within_window_and_consecutive_ones() {
        let mut r = rng();
        let mut sel = Selector::default();
        let mut seq = Vec::new();
        for i in 0..200 {
            // min_repeat satisfecho (separación enorme) para aislar el efecto del historial
            let m = sel.pick(Idle, E::Idle, i as Ms * 10_000_000, 3, &mut r).unwrap();
            seq.push(m.id);
        }
        for w in seq.windows(2) {
            assert_ne!(w[0], w[1], "repetición consecutiva");
        }
        let idle_count = CATALOG.iter().filter(|m| m.behaviors.contains(&Idle)).count();
        assert!(idle_count > 4);
        for w in seq.windows(4) {
            let set: std::collections::HashSet<_> = w.iter().collect();
            assert_eq!(set.len(), 4, "repetición dentro de la ventana de historial: {w:?}");
        }
    }

    #[test]
    fn avoids_same_category_back_to_back_when_possible() {
        let mut r = rng();
        let mut sel = Selector::default();
        let mut prev: Option<Category> = None;
        for i in 0..100 {
            let m = sel.pick(Idle, E::Idle, i as Ms * 10_000_000, 3, &mut r).unwrap();
            if let Some(p) = prev {
                assert_ne!(p, m.category, "misma categoría seguida: {}", m.id);
            }
            prev = Some(m.category);
        }
    }

    #[test]
    fn min_repeat_blocks_recent_message_even_without_history() {
        let mut r = rng();
        let mut sel = Selector::default();
        let first = sel.pick(Follow, E::Curious, 0, 0, &mut r).unwrap().id;
        // Follow tiene 2 frases: la segunda aparece, y luego ninguna hasta pasar min_repeat
        let second = sel.pick(Follow, E::Curious, 1000, 0, &mut r).unwrap().id;
        assert_ne!(first, second);
        assert!(sel.pick(Follow, E::Curious, 2000, 0, &mut r).is_none(), "ambas dentro de min_repeat");
        assert!(sel.pick(Follow, E::Curious, MIN_REPEAT + 5000, 0, &mut r).is_some());
    }

    #[test]
    fn weighted_pick_is_deterministic_for_a_seed() {
        let run = |seed| {
            let mut r = fastrand::Rng::with_seed(seed);
            let mut sel = Selector::default();
            (0..10).map(|i| sel.pick(Idle, E::Idle, i * 10_000_000, 3, &mut r).unwrap().id).collect::<Vec<_>>()
        };
        assert_eq!(run(5), run(5));
        assert_ne!(run(5), run(6));
    }

    // ---- planificador ----

    #[test]
    fn nothing_before_initial_delay_then_first_transition_speaks() {
        let mut r = rng();
        let mut t = Talker::new(cfg(60_000), 0, &mut r);
        assert!(!t.observe(500, &view(Rest, E::Sleepy), &mut r), "antes del retardo inicial");
        assert!(t.message().is_none());
        // sin cambio de comportamiento aunque venza: espera al próximo cambio
        assert!(!t.observe(2000, &view(Rest, E::Sleepy), &mut r));
        assert!(t.observe(3000, &view(Idle, E::Idle), &mut r));
        assert!(t.message().is_some());
    }

    #[test]
    fn message_lasts_4_to_6_seconds_and_ends_exactly_once() {
        let mut r = rng();
        let mut t = Talker::new(cfg(60_000), 0, &mut r);
        t.observe(1000, &view(Idle, E::Idle), &mut r); // last_behavior ya es Idle → sin cambio
        assert!(t.observe(1000, &view(Rest, E::Sleepy), &mut r));
        let until = t.next_wakeup().unwrap();
        assert!((5000..=7000).contains(&until), "until={until}");
        assert!(!t.step(until - 1));
        assert!(t.step(until));
        assert!(t.message().is_none() && t.expression_override().is_none());
        assert!(!t.step(until + 1), "no genera otro evento");
        assert_eq!(t.next_wakeup(), None);
    }

    #[test]
    fn cooldown_is_minimum_between_messages_and_counts_from_the_end() {
        let mut r = rng();
        let mut t = Talker::new(cfg(10_000), 0, &mut r);
        assert!(t.observe(1000, &view(Rest, E::Sleepy), &mut r));
        let until = t.next_wakeup().unwrap();
        t.step(until);
        // muchas transiciones durante el cooldown: ninguna produce mensaje
        let mut b = Idle;
        for k in 1..=9 {
            b = if b == Idle { Follow } else { Idle };
            assert!(!t.observe(until + k * 1000 - 1, &view(b, E::Curious), &mut r), "k={k}");
        }
        b = if b == Idle { Follow } else { Idle };
        assert!(t.observe(until + 10_000, &view(b, E::Curious), &mut r), "pasado el cooldown");
    }

    #[test]
    fn default_cooldown_is_12_minutes_and_first_message_2_to_4() {
        let c = TalkerConfig::default();
        assert_eq!(c.cooldown, 12 * 60_000);
        assert_eq!(c.initial_delay, (120_000, 240_000));
        assert!(c.enabled);
    }

    #[test]
    fn no_simultaneous_or_accumulated_messages() {
        let mut r = rng();
        // cooldown 0 y todo vencido de sobra: aun así, nunca dos a la vez ni cola de pendientes
        let mut t = Talker::new(cfg(0), 0, &mut r);
        let (mut starts, mut last_start): (Vec<Ms>, Option<Ms>) = (vec![], None);
        let mut prev_id: Option<&str> = None;
        for k in 0..400u64 {
            let now = 10_000_000 + k * 500;
            let b = [Idle, Follow, Rest, Idle][(k % 4) as usize];
            t.step(now);
            t.observe(now, &view(b, E::Curious), &mut r);
            let cur = t.message().map(|m| m.id);
            if cur.is_some() && cur != prev_id {
                if let (Some(ls), Some(_)) = (last_start, prev_id) {
                    panic!("un mensaje sustituyó a otro sin terminar (iniciado en {ls})");
                }
                starts.push(now);
                last_start = Some(now);
            }
            if cur.is_none() {
                last_start = None;
            }
            prev_id = cur;
        }
        assert!(starts.len() > 3, "debe hablar varias veces: {}", starts.len());
        for w in starts.windows(2) {
            assert!(w[1] - w[0] >= 4000, "mensajes demasiado juntos: {w:?}");
        }
    }

    #[test]
    fn behavior_change_cancels_an_inconsistent_message() {
        let mut r = rng();
        let mut t = Talker::new(cfg(60_000), 0, &mut r);
        assert!(t.observe(1000, &view(Rest, E::Sleepy), &mut r));
        assert_eq!(t.expression_override(), Some(E::Sleepy));
        // Rest → Idle: la frase de descanso ya no encaja
        assert!(t.observe(2000, &view(Idle, E::Idle), &mut r));
        assert!(t.message().is_none(), "frase de Rest cancelada al salir de Rest");
    }

    #[test]
    fn expression_override_matches_message_and_restores_after() {
        let mut r = rng();
        let mut t = Talker::new(cfg(60_000), 0, &mut r);
        t.observe(1000, &view(Follow, E::Curious), &mut r);
        let m = CATALOG.iter().find(|m| Some(m.id) == t.message().map(|x| x.id)).unwrap();
        assert_eq!(t.expression_override(), Some(m.expression));
        assert_eq!(m.expression, E::Curious);
        t.step(t.next_wakeup().unwrap());
        assert_eq!(t.expression_override(), None, "la FSM recupera el control de la cara");
    }

    #[test]
    fn suspension_cancels_message_freezes_cooldown_and_resume_continues() {
        let mut r = rng();
        let mut t = Talker::new(cfg(10_000), 0, &mut r);
        t.observe(1000, &view(Rest, E::Sleepy), &mut r);
        let due = t.next_due; // = 1000 + dur + 10000
        assert!(t.set_suspended(2000, true), "cancelar un mensaje visible es un cambio");
        assert!(t.message().is_none());
        assert_eq!(t.next_wakeup(), None, "suspendido: sin plazos que vigilar");
        // durante la suspensión no se inicia nada, aunque pase muchísimo tiempo
        assert!(!t.observe(1_000_000, &view(Idle, E::Idle), &mut r));
        assert!(!t.set_suspended(1_000_001, true), "idempotente");
        t.set_suspended(1_000_000, false);
        assert_eq!(t.next_due, 1_000_000 + (due - 2000), "cooldown restante intacto");
        // tras reanudar no habla hasta que venza el cooldown restante
        assert!(!t.observe(1_000_100, &view(Follow, E::Curious), &mut r));
    }

    #[test]
    fn suspension_without_message_reports_no_change() {
        let mut r = rng();
        let mut t = Talker::new(cfg(10_000), 0, &mut r);
        assert!(!t.set_suspended(100, true));
        assert!(!t.set_suspended(200, false));
        assert_eq!(t.version(), 0);
    }

    #[test]
    fn disabled_never_speaks() {
        let mut r = rng();
        let mut t = Talker::new(TalkerConfig { enabled: false, ..cfg(0) }, 0, &mut r);
        for k in 0..100 {
            t.observe(10_000_000 + k, &view([Idle, Rest][(k % 2) as usize], E::Idle), &mut r);
            assert!(t.message().is_none());
        }
    }

    #[test]
    fn version_increments_only_on_start_and_end() {
        let mut r = rng();
        let mut t = Talker::new(cfg(60_000), 0, &mut r);
        assert!(!t.observe(10, &view(Idle, E::Idle), &mut r));
        assert_eq!(t.version(), 0);
        t.observe(1000, &view(Rest, E::Sleepy), &mut r);
        assert_eq!(t.version(), 1);
        t.step(t.next_wakeup().unwrap());
        assert_eq!(t.version(), 2);
    }

    #[test]
    fn config_validation_clamps_values() {
        let c = TalkerConfig { enabled: true, cooldown: 5, initial_delay: (9_999_999_999, 1), history_len: 999 }.validated();
        assert_eq!(c.cooldown, 60_000);
        assert!(c.initial_delay.0 <= c.initial_delay.1);
        assert!(c.history_len <= CATALOG.len() - 2);
        let big = TalkerConfig { cooldown: u64::MAX, ..TalkerConfig::default() }.validated();
        assert_eq!(big.cooldown, 24 * 3600 * 1000);
    }
}
