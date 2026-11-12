//! Detector de «No molestar automático» (E4b): reglas puras, sin hilos ni reloj.
//!
//! El motor llama a `Detector::feed` con cada lectura de `SystemProbe::notification_state` (una lectura cada
//! `POLL_MS`, dentro de su único planificador) y aplica el cambio que devuelva sobre la FSM.
//!
//! Reglas (plan §6.1):
//! - Solo `Busy`, `RunningD3dFullScreen` y `PresentationMode` suprimen (lista blanca). `NotPresent`, `QuietTime`,
//!   `App`, `AcceptsNotifications` y cualquier valor desconocido NO suprimen.
//! - Histéresis: entrar exige `ENTER_READS` lecturas positivas CONSECUTIVAS; salir, `EXIT_READS` negativas consecutivas.
//! - Un error de la sonda nunca suprime ni cuenta como lectura: interrumpe las rachas. Si ya hay un episodio, tras
//!   `MAX_ERRORS` errores consecutivos el episodio termina (falla hacia VISIBLE: una sonda rota no deja a Miel oculta).
//!
//! Privacidad: solo se ve un entero de `SHQueryUserNotificationState`; no se sabe qué app es ni qué muestra.

use super::probe::{NotifState, ProbeError};
use super::state::Ms;

/// Cada cuánto se consulta mientras el sondeo está activo (plan: 2–3 s; ver `docs/TEST_LOG.md` para la medición).
pub const POLL_MS: Ms = 2_500;
/// Lecturas positivas consecutivas para entrar en supresión.
pub const ENTER_READS: u8 = 2;
/// Lecturas negativas consecutivas para salir.
pub const EXIT_READS: u8 = 2;
/// Errores consecutivos de la sonda que terminan un episodio.
pub const MAX_ERRORS: u8 = 3;

/// ¿Este estado de notificación suprime a Miel? Lista blanca explícita (nunca «todo lo distinto de 5»).
pub fn suppresses(s: NotifState) -> bool {
    matches!(s, NotifState::Busy | NotifState::RunningD3dFullScreen | NotifState::PresentationMode)
}

/// Qué cambió tras una lectura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Empieza un episodio de supresión.
    Enter,
    /// Termina por lecturas negativas.
    Exit,
    /// Termina por errores consecutivos de la sonda.
    ExitByErrors,
}

#[derive(Debug, Default, Clone)]
pub struct Detector {
    /// Hay un episodio abierto según este detector.
    suppressing: bool,
    pos: u8,
    neg: u8,
    errors: u8,
}

impl Detector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_suppressing(&self) -> bool {
        self.suppressing
    }

    /// Procesa una lectura. Devuelve `Some` solo cuando cambia el episodio.
    pub fn feed(&mut self, reading: Result<NotifState, ProbeError>) -> Option<Change> {
        match reading {
            Err(_) => {
                // Un error no es una lectura: rompe las rachas y solo puede TERMINAR un episodio.
                self.pos = 0;
                self.neg = 0;
                self.errors = self.errors.saturating_add(1);
                if self.suppressing && self.errors >= MAX_ERRORS {
                    self.suppressing = false;
                    self.errors = 0;
                    return Some(Change::ExitByErrors);
                }
                None
            }
            Ok(state) => {
                self.errors = 0;
                if suppresses(state) {
                    self.neg = 0;
                    self.pos = self.pos.saturating_add(1);
                    if !self.suppressing && self.pos >= ENTER_READS {
                        self.suppressing = true;
                        return Some(Change::Enter);
                    }
                } else {
                    self.pos = 0;
                    self.neg = self.neg.saturating_add(1);
                    if self.suppressing && self.neg >= EXIT_READS {
                        self.suppressing = false;
                        return Some(Change::Exit);
                    }
                }
                None
            }
        }
    }

    /// Olvida las rachas pero conserva si hay un episodio abierto (el sondeo se reanuda tras ocultar o pausar
    /// manualmente: las lecturas anteriores a la pausa ya no son «consecutivas» con las nuevas).
    pub fn forget_streaks(&mut self) {
        self.pos = 0;
        self.neg = 0;
        self.errors = 0;
    }

    /// Estado inicial (se apaga el interruptor).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use NotifState::*;

    const ERR: Result<NotifState, ProbeError> = Err(ProbeError::CallFailed);

    fn run(d: &mut Detector, seq: &[Result<NotifState, ProbeError>]) -> Vec<Option<Change>> {
        seq.iter().map(|r| d.feed(*r)).collect()
    }

    #[test]
    fn only_busy_d3d_and_presentation_suppress() {
        for s in [Busy, RunningD3dFullScreen, PresentationMode] {
            assert!(suppresses(s), "{s:?}");
        }
        for s in [NotPresent, AcceptsNotifications, QuietTime, App, Other(0), Other(8), Other(-1), Other(99)] {
            assert!(!suppresses(s), "{s:?} no debe suprimir");
        }
    }

    #[test]
    fn entering_needs_two_consecutive_positive_reads() {
        let mut d = Detector::new();
        assert_eq!(run(&mut d, &[Ok(Busy)]), [None], "una sola lectura no basta");
        assert!(!d.is_suppressing());
        assert_eq!(d.feed(Ok(Busy)), Some(Change::Enter));
        assert!(d.is_suppressing());
        assert_eq!(d.feed(Ok(Busy)), None, "ya dentro: sin más cambios");
    }

    #[test]
    fn a_single_positive_read_between_negatives_never_enters() {
        let mut d = Detector::new();
        let out = run(&mut d, &[Ok(Busy), Ok(AcceptsNotifications), Ok(Busy), Ok(AcceptsNotifications), Ok(RunningD3dFullScreen)]);
        assert!(out.iter().all(Option::is_none), "{out:?}");
        assert!(!d.is_suppressing());
    }

    #[test]
    fn any_suppressing_state_counts_towards_entering() {
        let mut d = Detector::new();
        assert_eq!(run(&mut d, &[Ok(Busy), Ok(PresentationMode)]), [None, Some(Change::Enter)]);
    }

    #[test]
    fn leaving_needs_two_consecutive_negative_reads() {
        let mut d = Detector::new();
        run(&mut d, &[Ok(Busy), Ok(Busy)]);
        assert_eq!(d.feed(Ok(AcceptsNotifications)), None, "una lectura negativa no basta");
        assert!(d.is_suppressing());
        assert_eq!(d.feed(Ok(AcceptsNotifications)), Some(Change::Exit));
        assert!(!d.is_suppressing());
    }

    #[test]
    fn a_positive_read_interrupts_the_negative_streak() {
        let mut d = Detector::new();
        run(&mut d, &[Ok(Busy), Ok(Busy)]);
        let out = run(&mut d, &[Ok(QuietTime), Ok(Busy), Ok(QuietTime), Ok(Busy), Ok(App)]);
        assert!(out.iter().all(Option::is_none), "{out:?}");
        assert!(d.is_suppressing());
        assert_eq!(d.feed(Ok(NotPresent)), Some(Change::Exit), "dos negativas seguidas (App y NotPresent)");
    }

    #[test]
    fn states_that_do_not_suppress_never_start_an_episode() {
        for s in [NotPresent, AcceptsNotifications, QuietTime, App, Other(42)] {
            let mut d = Detector::new();
            let out = run(&mut d, &[Ok(s); 10]);
            assert!(out.iter().all(Option::is_none) && !d.is_suppressing(), "{s:?}");
        }
    }

    #[test]
    fn errors_never_suppress_even_many_of_them() {
        let mut d = Detector::new();
        let out = run(&mut d, &[ERR; 20]);
        assert!(out.iter().all(Option::is_none) && !d.is_suppressing());
    }

    #[test]
    fn an_error_between_positive_reads_breaks_the_entering_streak() {
        let mut d = Detector::new();
        let out = run(&mut d, &[Ok(Busy), ERR, Ok(Busy)]);
        assert!(out.iter().all(Option::is_none), "las lecturas ya no son consecutivas: {out:?}");
        assert_eq!(d.feed(Ok(Busy)), Some(Change::Enter));
    }

    /// Caso 10 de §5.2: errores consecutivos durante un episodio.
    #[test]
    fn three_consecutive_errors_end_the_episode_and_two_do_not() {
        let mut d = Detector::new();
        run(&mut d, &[Ok(Busy), Ok(Busy)]);
        assert_eq!(run(&mut d, &[ERR, ERR]), [None, None]);
        assert!(d.is_suppressing(), "dos errores: sigue suprimida");
        assert_eq!(d.feed(ERR), Some(Change::ExitByErrors));
        assert!(!d.is_suppressing());
        // y no se vuelve a suprimir por errores
        assert_eq!(run(&mut d, &[ERR, ERR, ERR]), [None, None, None]);
        assert!(!d.is_suppressing());
        // un episodio nuevo sigue necesitando dos lecturas positivas
        assert_eq!(run(&mut d, &[Ok(Busy), Ok(Busy)]), [None, Some(Change::Enter)]);
    }

    #[test]
    fn intermittent_errors_do_not_end_the_episode() {
        let mut d = Detector::new();
        run(&mut d, &[Ok(Busy), Ok(Busy)]);
        // cada error va seguido de una lectura válida (positiva): nunca llegan 3 seguidos
        let out = run(&mut d, &[ERR, Ok(Busy), ERR, ERR, Ok(Busy), ERR, ERR, Ok(Busy)]);
        assert!(out.iter().all(Option::is_none), "{out:?}");
        assert!(d.is_suppressing());
    }

    #[test]
    fn errors_interleaved_with_negative_reads_delay_leaving_but_never_extend_the_episode_forever() {
        let mut d = Detector::new();
        run(&mut d, &[Ok(Busy), Ok(Busy)]);
        // negativa, error, negativa: no son consecutivas → no sale todavía
        assert_eq!(run(&mut d, &[Ok(AcceptsNotifications), ERR, Ok(AcceptsNotifications)]), [None, None, None]);
        assert!(d.is_suppressing());
        assert_eq!(d.feed(Ok(AcceptsNotifications)), Some(Change::Exit));
    }

    #[test]
    fn forgetting_streaks_keeps_the_open_episode_but_restarts_counting() {
        let mut d = Detector::new();
        d.feed(Ok(Busy));
        d.forget_streaks();
        assert_eq!(d.feed(Ok(Busy)), None, "la lectura previa a la pausa ya no cuenta");
        assert_eq!(d.feed(Ok(Busy)), Some(Change::Enter));
        d.feed(Ok(AcceptsNotifications));
        d.forget_streaks();
        assert!(d.is_suppressing(), "el episodio abierto se conserva");
        assert_eq!(d.feed(Ok(AcceptsNotifications)), None);
        assert_eq!(d.feed(Ok(AcceptsNotifications)), Some(Change::Exit));
    }

    #[test]
    fn reset_returns_to_the_initial_state() {
        let mut d = Detector::new();
        run(&mut d, &[Ok(Busy), Ok(Busy)]);
        d.reset();
        assert!(!d.is_suppressing());
        assert_eq!(d.feed(Ok(Busy)), None);
    }

    #[test]
    fn counters_saturate_instead_of_overflowing() {
        let mut d = Detector::new();
        for _ in 0..1000 {
            d.feed(Ok(Busy));
        }
        assert!(d.is_suppressing());
        for _ in 0..1000 {
            d.feed(ERR);
        }
        assert!(!d.is_suppressing());
    }
}
