//! Herramienta de diagnóstico (SOLO builds de depuración o con `--features diag`): ejecuta un
//! guion temporal de órdenes para medir rendimiento sin automatizar la interfaz de Windows.
//!
//! `MIEL_DIAG_SCRIPT="pause@60,resume@120,hide@180,show@240,follow_off@300,follow_on@360"`
//! (también `suppress`, `unsuppress` y `override`: simulan un episodio de supresión; E1b)
//! (segundos desde el arranque). Sin la variable, no se crea ningún hilo.
//!
//! `MIEL_DIAG_NOTIF="busy@10,ok@25,err@40,d3d@50,quiet@60"` (E4b) sustituye la sonda del sistema por una sonda
//! PROGRAMADA: devuelve ese estado de notificación a partir de cada segundo indicado (antes del primero, `ok`).
//! El motor y su planificador son los reales; solo cambia lo que «lee» la sonda, así que se puede comprobar en la
//! aplicación real que la ventana se oculta, reaparece, que «Mostrar de todos modos» funciona o que apagar el
//! interruptor termina el episodio, sin tocar el sistema. Estados: `busy`, `d3d`, `presentation`, `ok`, `notpresent`,
//! `quiet`, `app`, `other:<entero>` y `err` (la llamada falla).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::lifecycle::Lifecycle;
use crate::mascot::engine::{Cmd, Engine};
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

/// Sonda del sistema PROGRAMADA (solo diagnóstico). `idle_ms` devuelve 0 (sin actividad de sueño que simular).
pub struct ScriptedProbe {
    start: Instant,
    script: Vec<(u64, NotifReading)>,
    /// Índice del último estado registrado (para registrar solo los cambios).
    last_logged: AtomicUsize,
}

impl ScriptedProbe {
    pub fn new(script: Vec<(u64, NotifReading)>) -> Self {
        Self { start: Instant::now(), script, last_logged: AtomicUsize::new(usize::MAX) }
    }
}

impl SystemProbe for ScriptedProbe {
    fn idle_ms(&self) -> Result<u64, ProbeError> {
        Ok(0)
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

/// Sonda programada desde `MIEL_DIAG_NOTIF`, o `None` si la variable no existe (o es inválida: se avisa y se ignora).
pub fn scripted_probe_from_env() -> Option<Arc<dyn SystemProbe>> {
    let text = std::env::var_os("MIEL_DIAG_NOTIF")?;
    match parse_notif_script(&text.to_string_lossy()) {
        Ok(script) => {
            eprintln!("[miel] diag: sonda de notificaciones SIMULADA ({} pasos); no se consulta a Windows", script.len());
            Some(Arc::new(ScriptedProbe::new(script)))
        }
        Err(e) => {
            eprintln!("[miel] MIEL_DIAG_NOTIF inválido: {e}");
            None
        }
    }
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
        assert_eq!(p.idle_ms(), Ok(0));
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
