//! Frontera con el estado del SISTEMA que no es cursor ni monitores (esos siguen en `Host`).
//!
//! E1a: solo la abstracción, su aritmética pura y los dobles de prueba. Nada de esto se invoca todavía
//! desde producción; `windows_probe::WindowsProbe` es la implementación real (solo Windows).
//!
//! Privacidad: `idle_ms` solo devuelve CUÁNTO tiempo lleva sin entrada el usuario (un número); no dice
//! qué tecla ni qué ratón, ni lo guarda. Ver `docs/PRIVACY.md`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Máxima inactividad que se informa (30 días en ms). Con contadores de 32 bits el delta nunca pasa
/// de `MAX_PLAUSIBLE_DELTA` (≈ 24,9 días), así que este tope solo actúa sobre entradas de 64 bits.
pub const MAX_IDLE_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// Un delta de contador de 32 bits por encima de 2^31 se interpreta como "lectura adelantada" (carrera
/// entre `GetLastInputInfo` y `GetTickCount`), no como más de 24,8 días de inactividad.
pub const MAX_PLAUSIBLE_DELTA: u32 = 1 << 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeError {
    /// La llamada del sistema falló (p. ej. `GetLastInputInfo` devolvió 0).
    CallFailed,
}

/// Estado de notificaciones del usuario (`SHQueryUserNotificationState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifState {
    /// Salvapantallas, equipo bloqueado o cambio rápido de usuario.
    NotPresent,
    /// Aplicación a pantalla completa o similar que no quiere interrupciones.
    Busy,
    /// Aplicación Direct3D a pantalla completa (juego).
    RunningD3dFullScreen,
    PresentationMode,
    AcceptsNotifications,
    /// Horas de silencio / No molestar de Windows.
    QuietTime,
    /// Aplicación de la Tienda a pantalla completa.
    App,
    /// Valor que esta versión no conoce.
    Other(i32),
}

/// Traduce el valor crudo de `QUERY_USER_NOTIFICATION_STATE`. Pura.
pub fn notif_from_raw(raw: i32) -> NotifState {
    match raw {
        1 => NotifState::NotPresent,
        2 => NotifState::Busy,
        3 => NotifState::RunningD3dFullScreen,
        4 => NotifState::PresentationMode,
        5 => NotifState::AcceptsNotifications,
        6 => NotifState::QuietTime,
        7 => NotifState::App,
        other => NotifState::Other(other),
    }
}

/// Aritmética de inactividad, pura (separada de la llamada a Windows).
///
/// - `tick_now`: `GetTickCount()` (u32, ms, vuelve a 0 cada ≈ 49,7 días).
/// - `last_input`: `LASTINPUTINFO.dwTime`, o `None` si `GetLastInputInfo` falló.
///
/// `delta = tick_now.wrapping_sub(last_input)`; si `delta > 2^31` ⇒ 0 (lectura adelantada); el
/// resultado se limita a `MAX_IDLE_MS`; si la llamada falló ⇒ `Err`.
pub fn idle_ms_from_ticks(tick_now: u32, last_input: Option<u32>) -> Result<u64, ProbeError> {
    let last = last_input.ok_or(ProbeError::CallFailed)?;
    let delta = tick_now.wrapping_sub(last);
    if delta > MAX_PLAUSIBLE_DELTA {
        return Ok(0);
    }
    Ok(clamp_idle(u64::from(delta)))
}

/// Limita una inactividad a `MAX_IDLE_MS`.
pub fn clamp_idle(ms: u64) -> u64 {
    ms.min(MAX_IDLE_MS)
}

/// Sonda del sistema. Implementaciones: `windows_probe::WindowsProbe` y `FakeProbe`.
pub trait SystemProbe: Send + Sync {
    /// Milisegundos sin entrada del usuario en esta sesión.
    fn idle_ms(&self) -> Result<u64, ProbeError>;
    fn notification_state(&self) -> Result<NotifState, ProbeError>;
}

/// Sonda de pruebas con respuestas programables y contadores de llamadas.
#[derive(Debug)]
pub struct FakeProbe {
    idle: Mutex<Result<u64, ProbeError>>,
    notif: Mutex<Result<NotifState, ProbeError>>,
    idle_calls: AtomicUsize,
    notif_calls: AtomicUsize,
}

impl FakeProbe {
    pub fn new() -> Self {
        Self {
            idle: Mutex::new(Ok(0)),
            notif: Mutex::new(Ok(NotifState::AcceptsNotifications)),
            idle_calls: AtomicUsize::new(0),
            notif_calls: AtomicUsize::new(0),
        }
    }

    pub fn set_idle(&self, r: Result<u64, ProbeError>) {
        *self.idle.lock().unwrap_or_else(|e| e.into_inner()) = r;
    }

    pub fn set_notification_state(&self, r: Result<NotifState, ProbeError>) {
        *self.notif.lock().unwrap_or_else(|e| e.into_inner()) = r;
    }

    pub fn idle_calls(&self) -> usize {
        self.idle_calls.load(Ordering::SeqCst)
    }

    pub fn notification_calls(&self) -> usize {
        self.notif_calls.load(Ordering::SeqCst)
    }
}

impl Default for FakeProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemProbe for FakeProbe {
    fn idle_ms(&self) -> Result<u64, ProbeError> {
        self.idle_calls.fetch_add(1, Ordering::SeqCst);
        *self.idle.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn notification_state(&self) -> Result<NotifState, ProbeError> {
        self.notif_calls.fetch_add(1, Ordering::SeqCst);
        *self.notif.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn plain_delta() {
        assert_eq!(idle_ms_from_ticks(10_000, Some(4_000)), Ok(6_000));
        assert_eq!(idle_ms_from_ticks(5, Some(5)), Ok(0), "lectura igual a la anterior");
    }

    #[test]
    fn dw_time_near_u32_max_with_tick_already_wrapped() {
        // última entrada a 0xFFFF_FF00; el tick ya dio la vuelta y vale 0x100 → 0x200 ms de inactividad
        assert_eq!(idle_ms_from_ticks(0x100, Some(0xFFFF_FF00)), Ok(0x200));
        assert_eq!(idle_ms_from_ticks(0, Some(u32::MAX)), Ok(1));
        assert_eq!(idle_ms_from_ticks(u32::MAX, Some(u32::MAX)), Ok(0));
        // misma situación sin envolver: tick 0xFFFF_FFF0, entrada 0xFFFF_FF00
        assert_eq!(idle_ms_from_ticks(0xFFFF_FFF0, Some(0xFFFF_FF00)), Ok(0xF0));
    }

    #[test]
    fn tick_slightly_below_dw_time_is_zero_not_a_huge_idle() {
        // carrera: la entrada es unos ms POSTERIOR al tick leído → el delta envuelve a ~2^32 − k
        assert_eq!(idle_ms_from_ticks(1_000, Some(1_015)), Ok(0));
        assert_eq!(idle_ms_from_ticks(0, Some(1)), Ok(0));
        assert_eq!(idle_ms_from_ticks(0xFFFF_FFF0, Some(0xFFFF_FFFF)), Ok(0));
    }

    #[test]
    fn boundary_at_two_to_the_31() {
        assert_eq!(idle_ms_from_ticks(MAX_PLAUSIBLE_DELTA, Some(0)), Ok(u64::from(MAX_PLAUSIBLE_DELTA)));
        assert_eq!(idle_ms_from_ticks(MAX_PLAUSIBLE_DELTA + 1, Some(0)), Ok(0), "delta 2^31+1 es adelanto aparente");
        assert_eq!(idle_ms_from_ticks(0, Some(MAX_PLAUSIBLE_DELTA)), Ok(u64::from(MAX_PLAUSIBLE_DELTA)), "delta exacto 2^31 se acepta");
    }

    #[test]
    fn failed_call_is_an_error() {
        assert_eq!(idle_ms_from_ticks(123, None), Err(ProbeError::CallFailed));
        assert_eq!(idle_ms_from_ticks(0, None), Err(ProbeError::CallFailed));
    }

    #[test]
    fn large_jumps_are_kept_up_to_the_plausible_delta() {
        // tras una suspensión larga: ≈ 24 días de inactividad legítimos
        let day = 86_400_000u32;
        assert_eq!(idle_ms_from_ticks(24 * day, Some(0)), Ok(u64::from(24 * day)));
        assert_eq!(idle_ms_from_ticks(0, Some(0xFFFF_0000)), Ok(0x1_0000));
    }

    #[test]
    fn clamp_limits_to_thirty_days() {
        assert_eq!(MAX_IDLE_MS, 2_592_000_000);
        assert_eq!(clamp_idle(0), 0);
        assert_eq!(clamp_idle(MAX_IDLE_MS), MAX_IDLE_MS);
        assert_eq!(clamp_idle(MAX_IDLE_MS + 1), MAX_IDLE_MS);
        assert_eq!(clamp_idle(u64::MAX), MAX_IDLE_MS);
        // con u32 el tope nunca se alcanza: el máximo plausible (2^31 ms ≈ 24,9 d) < 30 d
        assert!(u64::from(MAX_PLAUSIBLE_DELTA) < MAX_IDLE_MS);
    }

    #[test]
    fn notification_state_mapping() {
        use NotifState::*;
        let all = [
            (1, NotPresent),
            (2, Busy),
            (3, RunningD3dFullScreen),
            (4, PresentationMode),
            (5, AcceptsNotifications),
            (6, QuietTime),
            (7, App),
        ];
        for (raw, want) in all {
            assert_eq!(notif_from_raw(raw), want);
        }
        assert_eq!(notif_from_raw(0), Other(0));
        assert_eq!(notif_from_raw(99), Other(99));
        assert_eq!(notif_from_raw(-1), Other(-1));
    }

    #[test]
    fn fake_probe_returns_programmed_values_and_counts_calls() {
        let p = FakeProbe::new();
        assert_eq!(p.idle_ms(), Ok(0));
        p.set_idle(Ok(42_000));
        assert_eq!(p.idle_ms(), Ok(42_000));
        p.set_idle(Err(ProbeError::CallFailed));
        assert_eq!(p.idle_ms(), Err(ProbeError::CallFailed));
        assert_eq!(p.idle_calls(), 3);

        assert_eq!(p.notification_state(), Ok(NotifState::AcceptsNotifications));
        p.set_notification_state(Ok(NotifState::QuietTime));
        assert_eq!(p.notification_state(), Ok(NotifState::QuietTime));
        p.set_notification_state(Err(ProbeError::CallFailed));
        assert!(p.notification_state().is_err());
        assert_eq!(p.notification_calls(), 3);
    }

    #[test]
    fn fake_probe_works_as_a_trait_object_across_threads() {
        let p = Arc::new(FakeProbe::new());
        p.set_idle(Ok(7));
        let dynp: Arc<dyn SystemProbe> = p.clone();
        let r = std::thread::spawn(move || dynp.idle_ms()).join().unwrap();
        assert_eq!(r, Ok(7));
    }
}
