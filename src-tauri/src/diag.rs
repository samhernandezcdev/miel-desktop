//! Herramienta de diagnóstico (SOLO builds de depuración o con `--features diag`): ejecuta un
//! guion temporal de órdenes para medir rendimiento sin automatizar la interfaz de Windows.
//!
//! `MIEL_DIAG_SCRIPT="pause@60,resume@120,hide@180,show@240,follow_off@300,follow_on@360"`
//! (también `suppress`, `unsuppress` y `override`: simulan un episodio de supresión; E1b; y E5: `lock`, `unlock`,
//! `suspend` y `power_resume` inyectan en el motor REAL los avisos de bloqueo de sesión y de suspensión/reanudación,
//! sin tocar el sistema)
//! (segundos desde el arranque). Sin la variable, no se crea ningún hilo.
//!
//! `MIEL_DIAG_NOTIF="busy@10,ok@25,err@40,d3d@50,quiet@60"` (E4b) sustituye la sonda del sistema por una sonda
//! PROGRAMADA: devuelve ese estado de notificación a partir de cada segundo indicado (antes del primero, `ok`).
//! El motor y su planificador son los reales; solo cambia lo que «lee» la sonda, así que se puede comprobar en la
//! aplicación real que la ventana se oculta, reaparece, que «Mostrar de todos modos» funciona o que apagar el
//! interruptor termina el episodio, sin tocar el sistema. Estados: `busy`, `d3d`, `presentation`, `ok`, `notpresent`,
//! `quiet`, `app`, `other:<entero>` y `err` (la llamada falla).
//!
//! `MIEL_DIAG_IDLE="0@0,600@20,0@90,err@120"` (E5) hace lo mismo con la INACTIVIDAD (`SystemProbe::idle_ms`): a partir del
//! segundo indicado la inactividad «era» esos segundos y SIGUE CRECIENDO con el tiempo (como la real); `0@t` es una
//! actividad en ese instante; `err` hace fallar la llamada. Antes del primer paso: 0. Con `MIEL_DIAG_SLEEP_SECS=<n>` el
//! umbral de sueño inicial son `n` SEGUNDOS (en producción es de 5 a 60 minutos). Nada de esto genera ni mueve entradas
//! reales del ratón ni del teclado.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::lifecycle::Lifecycle;
use crate::mascot::engine::{Cmd, Engine, SessionEvent};
use crate::mascot::probe::{NotifState, ProbeError, SystemProbe};

/// Interpreta el guion. Pura y probada.
pub fn parse_script(text: &str) -> Result<Vec<(u64, Cmd)>, String> {
    let mut out = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, at) = item.split_once('@').ok_or_else(|| format!("falta '@' en {item:?}"))?;
        let secs: u64 = at.trim().parse().map_err(|_| format!("tiempo inválido en {item:?}"))?;
        let cmd = match name.trim() {
            "pause" => Cmd::Pause(true),
            "resume" => Cmd::Pause(false),
            "hide" => Cmd::Hidden(true),
            "show" => Cmd::Hidden(false),
            "follow_off" => Cmd::FollowEnabled(false),
            "follow_on" => Cmd::FollowEnabled(true),
            "suppress" => Cmd::Suppressed(true),
            "unsuppress" => Cmd::Suppressed(false),
            "override" => Cmd::DndOverride,
            "lock" => Cmd::Session(SessionEvent::Locked),
            "unlock" => Cmd::Session(SessionEvent::Unlocked),
            "suspend" => Cmd::Session(SessionEvent::Suspended),
            "power_resume" => Cmd::Session(SessionEvent::Resumed),
            other => return Err(format!("orden desconocida {other:?}")),
        };
        out.push((secs, cmd));
    }
    out.sort_by_key(|(t, _)| *t);
    Ok(out)
}

/// Lanza el guion si `MIEL_DIAG_SCRIPT` está definida.
pub fn spawn_script(app: &AppHandle) {
    let Some(text) = std::env::var_os("MIEL_DIAG_SCRIPT") else { return };
    let script = match parse_script(&text.to_string_lossy()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[miel] MIEL_DIAG_SCRIPT inválido: {e}");
            return;
        }
    };
    let app = app.clone();
    let start = Instant::now();
    let _ = std::thread::Builder::new().name("miel-diag".into()).spawn(move || {
        for (secs, cmd) in script {
            loop {
                let closing = app.try_state::<Lifecycle>().is_none_or(|l| l.is_closing());
                if closing {
                    return;
                }
                let target = Duration::from_secs(secs);
                let now = start.elapsed();
                if now >= target {
                    break;
                }
                std::thread::sleep((target - now).min(Duration::from_millis(250)));
            }
            if let Some(engine) = app.try_state::<Engine>() {
                eprintln!("[miel] diag: {cmd:?} a los {secs} s");
                engine.send(cmd);
            }
        }
    });
}

/// Un estado de la sonda programada: lo que devuelve `notification_state`.
pub type NotifReading = Result<NotifState, ProbeError>;

/// Interpreta `MIEL_DIAG_NOTIF`. Pura y probada.
pub fn parse_notif_script(text: &str) -> Result<Vec<(u64, NotifReading)>, String> {
    let mut out = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, at) = item.rsplit_once('@').ok_or_else(|| format!("falta '@' en {item:?}"))?;
        let secs: u64 = at.trim().parse().map_err(|_| format!("tiempo inválido en {item:?}"))?;
        let name = name.trim();
        let reading: NotifReading = match name {
            "busy" => Ok(NotifState::Busy),
            "d3d" => Ok(NotifState::RunningD3dFullScreen),
            "presentation" => Ok(NotifState::PresentationMode),
            "ok" => Ok(NotifState::AcceptsNotifications),
            "notpresent" => Ok(NotifState::NotPresent),
            "quiet" => Ok(NotifState::QuietTime),
            "app" => Ok(NotifState::App),
            "err" => Err(ProbeError::CallFailed),
            other => match other.strip_prefix("other:") {
                Some(n) => Ok(NotifState::Other(n.trim().parse().map_err(|_| format!("entero inválido en {item:?}"))?)),
                None => return Err(format!("estado desconocido {other:?}")),
            },
        };
        out.push((secs, reading));
    }
    out.sort_by_key(|(t, _)| *t);
    Ok(out)
}

/// Estado vigente tras `elapsed_secs` de script (el último cuyo instante ya pasó; `ok` si ninguno).
pub fn notif_at(script: &[(u64, NotifReading)], elapsed_secs: u64) -> NotifReading {
    script.iter().rev().find(|(t, _)| *t <= elapsed_secs).map_or(Ok(NotifState::AcceptsNotifications), |(_, r)| *r)
}

/// Un paso de la inactividad programada: ms de inactividad en ese instante, o `Err` (la llamada falla).
pub type IdleReading = Result<u64, ProbeError>;

/// Interpreta `MIEL_DIAG_IDLE="<segundos de inactividad>@<segundo>,err@<segundo>"`. Pura y probada.
pub fn parse_idle_script(text: &str) -> Result<Vec<(u64, IdleReading)>, String> {
    let mut out = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (what, at) = item.rsplit_once('@').ok_or_else(|| format!("falta '@' en {item:?}"))?;
        let secs: u64 = at.trim().parse().map_err(|_| format!("tiempo inválido en {item:?}"))?;
        let reading: IdleReading = match what.trim() {
            "err" => Err(ProbeError::CallFailed),
            n => Ok(n.parse::<u64>().map_err(|_| format!("segundos de inactividad inválidos en {item:?}"))?.saturating_mul(1000)),
        };
        out.push((secs, reading));
    }
    out.sort_by_key(|(t, _)| *t);
    Ok(out)
}

/// Inactividad vigente a los `elapsed_ms` de script: la del último paso cuyo instante ya pasó, MÁS el tiempo transcurrido
/// desde él (la inactividad crece); `0` antes del primer paso.
pub fn idle_at(script: &[(u64, IdleReading)], elapsed_ms: u64) -> IdleReading {
    match script.iter().rev().find(|(t, _)| t * 1000 <= elapsed_ms) {
        None => Ok(0),
        Some((_, Err(e))) => Err(*e),
        Some((t, Ok(base))) => Ok(base.saturating_add(elapsed_ms - t * 1000)),
    }
}

/// `MIEL_DIAG_SLEEP_SECS`: segundos (> 0) → ms; cualquier otra cosa = sin cambio. Pura y probada.
pub fn sleep_secs_to_ms(v: Option<&str>) -> Option<u64> {
    v.and_then(|s| s.trim().parse::<u64>().ok()).filter(|n| *n > 0).map(|n| n.saturating_mul(1000))
}

/// Sonda del sistema PROGRAMADA (solo diagnóstico): notificaciones (E4b) e inactividad (E5).
pub struct ScriptedProbe {
    start: Instant,
    script: Vec<(u64, NotifReading)>,
    idle: Vec<(u64, IdleReading)>,
    /// Índice del último estado registrado (para registrar solo los cambios).
    last_logged: AtomicUsize,
    last_idle_logged: AtomicUsize,
}

impl ScriptedProbe {
    #[cfg(test)]
    pub fn new(script: Vec<(u64, NotifReading)>) -> Self {
        Self::with_idle(script, Vec::new())
    }

    pub fn with_idle(script: Vec<(u64, NotifReading)>, idle: Vec<(u64, IdleReading)>) -> Self {
        Self {
            start: Instant::now(),
            script,
            idle,
            last_logged: AtomicUsize::new(usize::MAX),
            last_idle_logged: AtomicUsize::new(usize::MAX),
        }
    }
}

impl SystemProbe for ScriptedProbe {
    fn idle_ms(&self) -> Result<u64, ProbeError> {
        let elapsed = self.start.elapsed().as_millis() as u64;
        let idx = self.idle.iter().rposition(|(t, _)| t * 1000 <= elapsed).map_or(usize::MAX - 1, |i| i);
        if self.last_idle_logged.swap(idx, Ordering::SeqCst) != idx {
            let unix_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            eprintln!(
                "[miel] t={unix_ms} diag: la sonda simulada informa inactividad {:?} (a los {} s)",
                idle_at(&self.idle, elapsed),
                elapsed / 1000
            );
        }
        idle_at(&self.idle, elapsed)
    }

    fn notification_state(&self) -> NotifReading {
        let secs = self.start.elapsed().as_secs();
        let idx = self.script.iter().rposition(|(t, _)| *t <= secs).map_or(usize::MAX - 1, |i| i);
        if self.last_logged.swap(idx, Ordering::SeqCst) != idx {
            let unix_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            eprintln!("[miel] t={unix_ms} diag: la sonda simulada devuelve {:?} (a los {secs} s)", notif_at(&self.script, secs));
        }
        notif_at(&self.script, secs)
    }
}

/// Sonda programada desde `MIEL_DIAG_NOTIF` y/o `MIEL_DIAG_IDLE`, o `None` si ninguna variable existe (o son inválidas:
/// se avisa y se ignoran). Con solo una de ellas, la otra señal responde «sin novedad» (notificaciones `ok`, inactividad 0).
pub fn scripted_probe_from_env() -> Option<Arc<dyn SystemProbe>> {
    let notif = std::env::var_os("MIEL_DIAG_NOTIF");
    let idle = std::env::var_os("MIEL_DIAG_IDLE");
    if notif.is_none() && idle.is_none() {
        return None;
    }
    let script = match notif.map(|t| parse_notif_script(&t.to_string_lossy())).transpose() {
        Ok(s) => s.unwrap_or_default(),
        Err(e) => {
            eprintln!("[miel] MIEL_DIAG_NOTIF inválido: {e}");
            return None;
        }
    };
    let idle_script = match idle.map(|t| parse_idle_script(&t.to_string_lossy())).transpose() {
        Ok(s) => s.unwrap_or_default(),
        Err(e) => {
            eprintln!("[miel] MIEL_DIAG_IDLE inválido: {e}");
            return None;
        }
    };
    eprintln!(
        "[miel] diag: sonda SIMULADA ({} pasos de notificación, {} de inactividad); no se consulta a Windows",
        script.len(),
        idle_script.len()
    );
    Some(Arc::new(ScriptedProbe::with_idle(script, idle_script)))
}

/// Acciones de interfaz del guion de diagnóstico (E3): no son órdenes del motor.
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    OpenSettings,
    CloseSettings,
    /// `set:<clave>=<valor JSON>`: llama a la misma lógica que el comando `set_setting` (valida y avisa
    /// a la ventana con `settings://changed`), pero desde Rust, sin pasar por la página.
    Set(String, serde_json::Value),
}

/// Interpreta `MIEL_DIAG_UI_SCRIPT="open_settings@12,set:message_interval_minutes=30@20,close_settings@40"`.
pub fn parse_ui_script(text: &str) -> Result<Vec<(u64, UiAction)>, String> {
    let mut out = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, at) = item.rsplit_once('@').ok_or_else(|| format!("falta '@' en {item:?}"))?;
        let secs: u64 = at.trim().parse().map_err(|_| format!("tiempo inválido en {item:?}"))?;
        let name = name.trim();
        let action = if let Some(rest) = name.strip_prefix("set:") {
            let (key, raw) = rest.split_once('=').ok_or_else(|| format!("falta '=' en {item:?}"))?;
            let value = serde_json::from_str(raw).map_err(|e| format!("valor JSON inválido en {item:?}: {e}"))?;
            UiAction::Set(key.trim().to_string(), value)
        } else {
            match name {
                "open_settings" => UiAction::OpenSettings,
                "close_settings" => UiAction::CloseSettings,
                other => return Err(format!("acción desconocida {other:?}")),
            }
        };
        out.push((secs, action));
    }
    out.sort_by_key(|(t, _)| *t);
    Ok(out)
}

/// Lanza el guion de interfaz si `MIEL_DIAG_UI_SCRIPT` está definida (mismo reloj que `spawn_script`).
pub fn spawn_ui_script(app: &AppHandle) {
    let Some(text) = std::env::var_os("MIEL_DIAG_UI_SCRIPT") else { return };
    let script = match parse_ui_script(&text.to_string_lossy()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[miel] MIEL_DIAG_UI_SCRIPT inválido: {e}");
            return;
        }
    };
    let app = app.clone();
    let start = Instant::now();
    let _ = std::thread::Builder::new().name("miel-diag-ui".into()).spawn(move || {
        for (secs, action) in script {
            loop {
                if app.try_state::<Lifecycle>().is_none_or(|l| l.is_closing()) {
                    return;
                }
                let target = Duration::from_secs(secs);
                let now = start.elapsed();
                if now >= target {
                    break;
                }
                std::thread::sleep((target - now).min(Duration::from_millis(250)));
            }
            // marca de tiempo absoluta (ms Unix, como las del motor) para delimitar fases en las mediciones
            let unix_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            eprintln!("[miel] t={unix_ms} diag: {action:?} a los {secs} s");
            match action {
                UiAction::OpenSettings => match crate::settings_window::open(&app) {
                    Ok(r) => eprintln!("[miel] diag: ajustes {r:?}"),
                    Err(e) => eprintln!("[miel] diag: no se pudo abrir ajustes: {e}"),
                },
                UiAction::CloseSettings => {
                    let closed = crate::settings_window::close(&app);
                    eprintln!("[miel] diag: ajustes cerrada={closed}");
                }
                UiAction::Set(key, value) => {
                    if let Some(store) = app.try_state::<std::sync::Arc<crate::settings::SettingsStore>>() {
                        match crate::apply_setting(&app, &store, &key, &value) {
                            Ok(_) => eprintln!("[miel] diag: set {key} aplicado"),
                            Err(e) => eprintln!("[miel] diag: set {key} rechazado: {e}"),
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_ui_script() {
        let s = parse_ui_script("close_settings@40, open_settings@12 ,set:message_interval_minutes=30@20,set:pet_size=\"x@y\"@25").unwrap();
        assert_eq!(
            s,
            [
                (12, UiAction::OpenSettings),
                (20, UiAction::Set("message_interval_minutes".into(), serde_json::json!(30))),
                (25, UiAction::Set("pet_size".into(), serde_json::json!("x@y"))),
                (40, UiAction::CloseSettings),
            ]
        );
        for bad in ["open_settings", "open_settings@x", "set:k@5", "set:k=@5", "abrir@3"] {
            assert!(parse_ui_script(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_the_notification_script() {
        let s = parse_notif_script("ok@25, busy@10 ,err@40,other:42@50,d3d@60,presentation@70,quiet@80,app@90,notpresent@100").unwrap();
        assert_eq!(
            s,
            [
                (10, Ok(NotifState::Busy)),
                (25, Ok(NotifState::AcceptsNotifications)),
                (40, Err(ProbeError::CallFailed)),
                (50, Ok(NotifState::Other(42))),
                (60, Ok(NotifState::RunningD3dFullScreen)),
                (70, Ok(NotifState::PresentationMode)),
                (80, Ok(NotifState::QuietTime)),
                (90, Ok(NotifState::App)),
                (100, Ok(NotifState::NotPresent)),
            ]
        );
        for bad in ["busy", "busy@x", "pantalla@3", "other:x@3", "@5"] {
            assert!(parse_notif_script(bad).is_err(), "{bad}");
        }
        assert!(parse_notif_script("").unwrap().is_empty());
    }

    #[test]
    fn the_scripted_state_is_the_last_one_whose_time_has_passed() {
        let s = parse_notif_script("busy@10,err@20,ok@30").unwrap();
        assert_eq!(notif_at(&s, 0), Ok(NotifState::AcceptsNotifications), "antes del primero: ok");
        assert_eq!(notif_at(&s, 9), Ok(NotifState::AcceptsNotifications));
        assert_eq!(notif_at(&s, 10), Ok(NotifState::Busy));
        assert_eq!(notif_at(&s, 19), Ok(NotifState::Busy));
        assert_eq!(notif_at(&s, 20), Err(ProbeError::CallFailed));
        assert_eq!(notif_at(&s, 3600), Ok(NotifState::AcceptsNotifications));
        assert_eq!(notif_at(&[], 5), Ok(NotifState::AcceptsNotifications));
    }

    #[test]
    fn the_scripted_probe_is_a_probe_that_never_touches_windows() {
        let p = ScriptedProbe::new(parse_notif_script("busy@0").unwrap());
        assert_eq!(p.notification_state(), Ok(NotifState::Busy));
        assert_eq!(p.idle_ms(), Ok(0), "sin guion de inactividad: 0");
    }

    #[test]
    fn parses_the_idle_script_and_the_idle_grows_with_time() {
        let s = parse_idle_script("0@0, 600@20 ,err@40,0@50").unwrap();
        assert_eq!(s, [(0, Ok(0)), (20, Ok(600_000)), (40, Err(ProbeError::CallFailed)), (50, Ok(0))]);
        assert_eq!(idle_at(&[], 99_000), Ok(0), "sin guion: 0");
        assert_eq!(idle_at(&s, 5_000), Ok(5_000), "0@0 crece con el tiempo");
        assert_eq!(idle_at(&s, 20_000), Ok(600_000));
        assert_eq!(idle_at(&s, 25_500), Ok(605_500), "sigue creciendo desde el paso");
        assert_eq!(idle_at(&s, 40_000), Err(ProbeError::CallFailed));
        assert_eq!(idle_at(&s, 50_000), Ok(0), "una actividad reinicia la cuenta");
        for bad in ["600", "x@5", "600@x", "-1@5", "@5"] {
            assert!(parse_idle_script(bad).is_err(), "{bad}");
        }
        assert!(parse_idle_script("").unwrap().is_empty());
        let p = ScriptedProbe::with_idle(vec![], parse_idle_script("90@0").unwrap());
        assert!(p.idle_ms().unwrap() >= 90_000, "la sonda programada nunca consulta a Windows");
    }

    #[test]
    fn the_diag_sleep_threshold_is_in_seconds_and_garbage_is_ignored() {
        assert_eq!(sleep_secs_to_ms(Some("30")), Some(30_000));
        assert_eq!(sleep_secs_to_ms(Some(" 5 ")), Some(5_000));
        for bad in [None, Some(""), Some("0"), Some("-3"), Some("1.5"), Some("abc")] {
            assert_eq!(sleep_secs_to_ms(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn parses_the_session_and_power_commands() {
        let s = parse_script("lock@5,unlock@10,suspend@15,power_resume@20,resume@25").unwrap();
        assert_eq!(
            s,
            [
                (5, Cmd::Session(SessionEvent::Locked)),
                (10, Cmd::Session(SessionEvent::Unlocked)),
                (15, Cmd::Session(SessionEvent::Suspended)),
                (20, Cmd::Session(SessionEvent::Resumed)),
                (25, Cmd::Pause(false)),
            ],
            "`resume` sigue siendo reanudar la pausa (v0.1.0)"
        );
    }

    #[test]
    fn parses_and_sorts_a_script() {
        let s = parse_script("hide@180, pause@60 ,resume@120,show@240").unwrap();
        assert_eq!(
            s,
            [(60, Cmd::Pause(true)), (120, Cmd::Pause(false)), (180, Cmd::Hidden(true)), (240, Cmd::Hidden(false))]
        );
        assert!(parse_script("").unwrap().is_empty());
    }

    #[test]
    fn parses_the_suppression_commands() {
        let s = parse_script("suppress@10,override@20,unsuppress@30").unwrap();
        assert_eq!(s, [(10, Cmd::Suppressed(true)), (20, Cmd::DndOverride), (30, Cmd::Suppressed(false))]);
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["pause", "pause@x", "volar@3", "@5"] {
            assert!(parse_script(bad).is_err(), "{bad}");
        }
    }
}
