//! Sueño por inactividad (E5): la parte PURA de la decisión (sin hilos, sin Windows, sin reloj propio).
//!
//! El motor (`engine.rs`) es quien llama a la sonda (`SystemProbe::idle_ms`) y quien planifica: aquí solo se
//! decide, dada una lectura de inactividad, si hay que dormir, despertar o esperar, y cuándo volver a mirar.
//!
//! - **Sin sondeo fijo despierta:** el próximo instante de comprobación es `umbral − inactividad` (con un mínimo).
//! - **Durmiendo:** sondeo ligero cada `WAKE_POLL_MS`; despierta si la inactividad cae por debajo de `umbral − margen`.
//! - **Un error nunca provoca sueño:** despierta reintenta con retroceso exponencial; durmiendo, tres errores
//!   seguidos la despiertan (falla hacia despierta).
//! - **Actividad sintética** (desbloqueo, reanudación, acción de la persona en la bandeja): `activity_at` actúa como
//!   suelo de la inactividad (`min(lectura, ahora − activity_at)`), de modo que «se trata como actividad reciente»
//!   aunque el contador de Windows aún no lo refleje.
//!
//! Privacidad: solo interviene UN número (cuánto lleva sin entrada la sesión); no se sabe qué tecla ni qué ratón.

use super::probe::ProbeError;
use super::state::Ms;

/// Intervalo del sondeo ligero mientras duerme (plan §4.4: ~1–2 s; se toma el extremo alto).
pub const WAKE_POLL_MS: Ms = 2_000;
/// Margen de histéresis: durmiendo, solo despierta si `inactividad + margen < umbral`.
pub const WAKE_MARGIN_MS: Ms = 2_000;
/// Mínimo entre dos comprobaciones (evita un bucle ocupado cerca del umbral).
pub const MIN_CHECK_MS: Ms = 1_000;
/// Primer reintento tras un error (despierta) y tope del retroceso.
pub const BACKOFF_FIRST_MS: Ms = 5_000;
pub const BACKOFF_MAX_MS: Ms = 60_000;
/// Errores seguidos durmiendo que la despiertan.
pub const MAX_ERRORS_ASLEEP: u32 = 3;
/// Tras despertar, el hablante no puede empezar un mensaje durante este respiro (sin mensaje al despertar).
pub const WAKE_QUIET_MS: Ms = 10_000;
/// Umbral por defecto (10 min).
pub const DEFAULT_AFTER_MS: Ms = 10 * 60_000;

/// Lo que el motor debe hacer tras una lectura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Sleep,
    Wake,
}

/// Bloqueo de sesión y suspensión. Mientras alguno vale, la sesión «está ausente»: duerme sin sondear.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Away {
    pub locked: bool,
    pub suspended: bool,
}

impl Away {
    pub fn any(self) -> bool {
        self.locked || self.suspended
    }
}

/// Retroceso tras `errors` errores seguidos (1 → 5 s, 2 → 10 s, 3 → 20 s … tope 60 s).
pub fn backoff_ms(errors: u32) -> Ms {
    let shift = errors.saturating_sub(1).min(8);
    (BACKOFF_FIRST_MS << shift).min(BACKOFF_MAX_MS)
}

/// Estado de la tarea de sueño del planificador.
#[derive(Debug, Clone, Copy)]
pub struct SleepTask {
    /// Interruptor `sleep_enabled`.
    pub enabled: bool,
    /// Umbral de inactividad (ms).
    pub after: Ms,
    /// Próxima comprobación (`idle_check` despierta, `wake_poll` durmiendo). `Some` solo con la puerta abierta.
    pub next: Option<Ms>,
    /// Errores seguidos de la sonda.
    pub errors: u32,
    pub away: Away,
    /// Última actividad sintética (suelo de la inactividad); ver el módulo.
    pub activity_at: Option<Ms>,
}

impl SleepTask {
    pub fn new(enabled: bool, after: Ms) -> Self {
        Self { enabled, after: after.max(1), next: None, errors: 0, away: Away::default(), activity_at: None }
    }

    /// Inactividad efectiva: la lectura, limitada por el tiempo desde la última actividad sintética.
    pub fn effective_idle(&self, now: Ms, reading: u64) -> u64 {
        match self.activity_at {
            Some(a) => reading.min(now.saturating_sub(a)),
            None => reading,
        }
    }

    /// Procesa una lectura. Actualiza `next` y `errors` y devuelve la acción. `asleep`: ¿duerme ahora mismo?
    pub fn on_reading(&mut self, now: Ms, asleep: bool, reading: Result<u64, ProbeError>) -> Action {
        match reading {
            Err(_) => {
                self.errors += 1;
                if asleep {
                    if self.errors >= MAX_ERRORS_ASLEEP {
                        self.errors = 0;
                        self.next = Some(now + backoff_ms(1));
                        return Action::Wake; // falla hacia despierta
                    }
                    self.next = Some(now + WAKE_POLL_MS);
                } else {
                    self.next = Some(now + backoff_ms(self.errors));
                }
                Action::None
            }
            Ok(raw) => {
                self.errors = 0;
                let idle = self.effective_idle(now, raw);
                let until = self.after.saturating_sub(idle).max(MIN_CHECK_MS);
                if asleep {
                    if idle.saturating_add(WAKE_MARGIN_MS) < self.after {
                        self.next = Some(now + until);
                        Action::Wake
                    } else {
                        self.next = Some(now + WAKE_POLL_MS);
                        Action::None
                    }
                } else if idle >= self.after {
                    self.next = Some(now + WAKE_POLL_MS);
                    Action::Sleep
                } else {
                    self.next = Some(now + until);
                    Action::None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Ms = 60_000;

    fn task() -> SleepTask {
        SleepTask::new(true, 10 * MIN)
    }

    #[test]
    fn the_next_check_is_threshold_minus_idle_with_a_floor() {
        let mut t = task();
        assert_eq!(t.on_reading(1_000, false, Ok(0)), Action::None);
        assert_eq!(t.next, Some(1_000 + 10 * MIN));
        assert_eq!(t.on_reading(5_000, false, Ok(4 * MIN)), Action::None);
        assert_eq!(t.next, Some(5_000 + 6 * MIN));
        // a 300 ms del umbral: no se mira antes de MIN_CHECK_MS
        assert_eq!(t.on_reading(7_000, false, Ok(10 * MIN - 300)), Action::None);
        assert_eq!(t.next, Some(7_000 + MIN_CHECK_MS));
    }

    #[test]
    fn exactly_the_threshold_sleeps_and_one_ms_less_does_not() {
        let mut t = task();
        assert_eq!(t.on_reading(0, false, Ok(10 * MIN - 1)), Action::None);
        assert_eq!(t.on_reading(0, false, Ok(10 * MIN)), Action::Sleep);
        assert_eq!(t.next, Some(WAKE_POLL_MS), "durmiendo: sondeo ligero");
        assert_eq!(t.on_reading(0, false, Ok(u64::MAX)), Action::Sleep, "sin desbordar");
    }

    #[test]
    fn asleep_it_wakes_only_below_threshold_minus_margin() {
        let mut t = task();
        assert_eq!(t.on_reading(0, true, Ok(10 * MIN)), Action::None);
        assert_eq!(t.on_reading(0, true, Ok(10 * MIN - WAKE_MARGIN_MS)), Action::None, "justo en el margen: sigue dormida");
        assert_eq!(t.on_reading(0, true, Ok(10 * MIN - WAKE_MARGIN_MS - 1)), Action::Wake);
        assert_eq!(t.on_reading(0, true, Ok(0)), Action::Wake);
        assert_eq!(t.next, Some(10 * MIN), "tras despertar: próxima comprobación al umbral");
    }

    #[test]
    fn an_error_never_causes_sleep_and_retries_with_backoff() {
        let mut t = task();
        let mut prev = 0;
        let mut gaps = vec![];
        for i in 1..=6 {
            assert_eq!(t.on_reading(100_000, false, Err(ProbeError::CallFailed)), Action::None, "error {i}");
            let gap = t.next.unwrap() - 100_000;
            assert!(gap >= prev, "el retroceso no baja");
            prev = gap;
            gaps.push(gap);
        }
        assert_eq!(gaps, [5_000, 10_000, 20_000, 40_000, 60_000, 60_000]);
        // una lectura buena reinicia el retroceso
        assert_eq!(t.on_reading(0, false, Ok(0)), Action::None);
        assert_eq!(t.errors, 0);
        t.on_reading(0, false, Err(ProbeError::CallFailed));
        assert_eq!(t.next, Some(5_000));
    }

    #[test]
    fn asleep_two_errors_keep_sleeping_and_three_wake() {
        let mut t = task();
        assert_eq!(t.on_reading(0, true, Err(ProbeError::CallFailed)), Action::None);
        assert_eq!(t.on_reading(0, true, Err(ProbeError::CallFailed)), Action::None);
        assert_eq!(t.next, Some(WAKE_POLL_MS));
        assert_eq!(t.on_reading(0, true, Err(ProbeError::CallFailed)), Action::Wake);
        assert_eq!(t.errors, 0);
    }

    #[test]
    fn errors_between_good_reads_do_not_accumulate_towards_waking() {
        let mut t = task();
        for _ in 0..10 {
            assert_eq!(t.on_reading(0, true, Err(ProbeError::CallFailed)), Action::None);
            assert_eq!(t.on_reading(0, true, Ok(10 * MIN)), Action::None);
        }
    }

    #[test]
    fn synthetic_activity_floors_the_idle_reading() {
        let mut t = task();
        t.activity_at = Some(50 * MIN);
        // Windows aún dice "una hora sin entrada", pero hubo una acción hace 1 s
        assert_eq!(t.effective_idle(50 * MIN + 1_000, 60 * MIN), 1_000);
        assert_eq!(t.on_reading(50 * MIN + 1_000, false, Ok(60 * MIN)), Action::None);
        assert_eq!(t.next, Some(50 * MIN + 1_000 + 10 * MIN - 1_000));
        // pasado el umbral desde esa actividad, vuelve a dormir
        assert_eq!(t.on_reading(60 * MIN, false, Ok(70 * MIN)), Action::Sleep);
        // una lectura menor que el suelo manda la lectura
        assert_eq!(t.effective_idle(60 * MIN, 5), 5);
    }

    #[test]
    fn a_new_threshold_changes_the_next_deadline_and_the_verdict() {
        let mut t = task();
        assert_eq!(t.on_reading(0, false, Ok(7 * MIN)), Action::None);
        assert_eq!(t.next, Some(3 * MIN));
        t.after = 5 * MIN;
        assert_eq!(t.on_reading(0, false, Ok(7 * MIN)), Action::Sleep, "el umbral nuevo ya está superado");
        t.after = 30 * MIN;
        assert_eq!(t.on_reading(0, true, Ok(7 * MIN)), Action::Wake, "durmiendo con un umbral mayor: despierta");
    }

    #[test]
    fn away_is_the_union_of_lock_and_suspend() {
        assert!(!Away::default().any());
        assert!(Away { locked: true, suspended: false }.any());
        assert!(Away { locked: false, suspended: true }.any());
    }

    #[test]
    fn backoff_is_monotone_and_capped() {
        let v: Vec<_> = (0..12).map(backoff_ms).collect();
        assert!(v.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(*v.last().unwrap(), BACKOFF_MAX_MS);
        assert_eq!(backoff_ms(1), BACKOFF_FIRST_MS);
        assert_eq!(backoff_ms(u32::MAX), BACKOFF_MAX_MS);
    }
}
