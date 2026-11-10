//! Herramienta de diagnóstico (SOLO builds de depuración o con `--features diag`): ejecuta un
//! guion temporal de órdenes para medir rendimiento sin automatizar la interfaz de Windows.
//!
//! `MIEL_DIAG_SCRIPT="pause@60,resume@120,hide@180,show@240,follow_off@300,follow_on@360"`
//! (también `suppress`, `unsuppress` y `override`: simulan un episodio de supresión; E1b)
//! (segundos desde el arranque). Sin la variable, no se crea ningún hilo.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::lifecycle::Lifecycle;
use crate::mascot::engine::{Cmd, Engine};

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

#[cfg(test)]
mod tests {
    use super::*;

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
