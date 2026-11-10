//! Implementación de `Host` sobre las APIs oficiales de Tauri 2.
//! Solo se usan: posición del cursor, monitores, rectángulo/posición/visibilidad de la ventana
//! y emisión de eventos propios. Nada de hooks globales ni lectura de otras ventanas.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, WebviewWindow};

use super::engine::{Host, MonitorInfo};
use super::geometry::{Rect, Vec2};
use super::state::View;
use crate::settings::SettingsStore;

fn unix_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// Nombre del evento que recibe el frontend (solo cuando el estado cambia).
pub const VIEW_EVENT: &str = "mascot://view";

pub struct TauriHost {
    app: AppHandle,
    win: WebviewWindow,
    /// Las preferencias que cambia la bandeja (pausa y seguimiento) se reflejan aquí, solo si cambian.
    settings: Arc<SettingsStore>,
}

impl TauriHost {
    pub fn new(app: AppHandle, win: WebviewWindow, settings: Arc<SettingsStore>) -> Self {
        Self { app, win, settings }
    }
}

impl Host for TauriHost {
    fn cursor(&self) -> Option<Vec2> {
        self.app.cursor_position().ok().map(|p| Vec2::new(p.x, p.y))
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        if crate::DIAG {
            eprintln!("[miel] t={} relectura de monitores", unix_ms());
        }
        match self.app.available_monitors() {
            Ok(list) => list
                .iter()
                .map(|m| {
                    let wa = m.work_area();
                    MonitorInfo {
                        work: Rect::new(wa.position.x, wa.position.y, wa.size.width, wa.size.height),
                        scale: m.scale_factor(),
                    }
                })
                .collect(),
            Err(e) => {
                eprintln!("[miel] no se pudieron leer los monitores: {e}");
                Vec::new()
            }
        }
    }

    fn window_rect(&self) -> Option<Rect> {
        let p = self.win.outer_position().ok()?;
        let s = self.win.outer_size().ok()?;
        Some(Rect::new(p.x, p.y, s.width, s.height))
    }

    fn set_position(&self, x: i32, y: i32) {
        if let Err(e) = self.win.set_position(tauri::PhysicalPosition::new(x, y)) {
            eprintln!("[miel] set_position falló: {e}");
        }
    }

    fn set_visible(&self, visible: bool) {
        let r = if visible { self.win.show() } else { self.win.hide() };
        if let Err(e) = r {
            eprintln!("[miel] cambiar visibilidad falló: {e}");
        }
    }

    fn emit_view(&self, view: &View) {
        if crate::DIAG {
            eprintln!(
                "[miel] t={} vista v{} {:?}/{:?} paused={} hidden={} suppressed={} sleeping={} follow={} msg={:?}",
                unix_ms(),
                view.version,
                view.behavior,
                view.expression,
                view.paused,
                view.hidden,
                view.suppressed,
                view.sleeping,
                view.follow_enabled,
                view.message.map(|m| m.id)
            );
        }
        if let Err(e) = self.app.emit(VIEW_EVENT, view) {
            eprintln!("[miel] emit falló: {e}");
        }
        // Preferencias persistentes: pausa y seguimiento (solo escribe si cambiaron; `Hidden` no se guarda).
        self.settings.mirror_runtime(view.paused, view.follow_enabled);
        // Menú de la bandeja: el hilo del motor NUNCA toca el menú (las operaciones de menú esperan
        // al hilo principal); solo encola la actualización, sin bloquear.
        let (app, v) = (self.app.clone(), *view);
        if let Err(e) = self.app.run_on_main_thread(move || crate::tray::sync_on_main(&app, &v)) {
            if crate::DIAG {
                eprintln!("[miel] no se pudo encolar la sincronización de la bandeja: {e}");
            }
            let _ = e; // durante el cierre el bucle de eventos ya no acepta tareas
        }
    }
}
