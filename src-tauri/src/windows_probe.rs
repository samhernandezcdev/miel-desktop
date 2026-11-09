//! Implementación real de `SystemProbe` para Windows. E1a: NO se invoca desde producción todavía.
//!
//! - `idle_ms`: `GetLastInputInfo` + `GetTickCount`. Solo lee un contador de tiempo de la sesión
//!   (cuándo fue la última entrada), nunca qué se pulsó ni dónde; sin ganchos globales ni Raw Input.
//! - `notification_state`: `SHQueryUserNotificationState` (solo lectura, un entero).
//!
//! La aritmética está en `mascot::probe::idle_ms_from_ticks` (pura y probada); aquí solo se llama a Windows.

use std::mem::size_of;

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::SHQueryUserNotificationState;

use crate::mascot::probe::{idle_ms_from_ticks, notif_from_raw, NotifState, ProbeError, SystemProbe};

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsProbe;

impl WindowsProbe {
    pub fn new() -> Self {
        Self
    }
}

impl SystemProbe for WindowsProbe {
    fn idle_ms(&self) -> Result<u64, ProbeError> {
        let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        // SAFETY: `info` es un LASTINPUTINFO válido con `cbSize` correcto; GetTickCount no tiene precondiciones.
        let (ok, tick) = unsafe { (GetLastInputInfo(&mut info).as_bool(), GetTickCount()) };
        idle_ms_from_ticks(tick, ok.then_some(info.dwTime))
    }

    fn notification_state(&self) -> Result<NotifState, ProbeError> {
        // SAFETY: llamada sin precondiciones; devuelve el estado por valor.
        unsafe { SHQueryUserNotificationState() }
            .map(|s| notif_from_raw(s.0))
            .map_err(|_| ProbeError::CallFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Llamadas reales de solo lectura. No se asume el resultado (una sesión sin escritorio interactivo puede
    /// hacer fallar la llamada): si responde, el valor debe ser plausible; si falla, es `CallFailed`.
    #[test]
    fn real_calls_return_plausible_values_or_a_clean_error() {
        let p = WindowsProbe::new();
        match p.idle_ms() {
            Ok(ms) => assert!(ms <= crate::mascot::probe::MAX_IDLE_MS),
            Err(e) => assert_eq!(e, ProbeError::CallFailed),
        }
        match p.notification_state() {
            Ok(s) => assert!(!matches!(s, NotifState::Other(_)), "estado desconocido: {s:?}"),
            Err(e) => assert_eq!(e, ProbeError::CallFailed),
        }
    }
}
