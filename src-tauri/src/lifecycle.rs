//! Ciclo de vida: arranque listo (bandeja + ventana) y cierre controlado.
//!
//! Hechos verificados en Tauri 2.12 (código fuente instalado):
//! - `AppHandle::exit(code)` llama a `cleanup_before_exit()` (vacía los iconos de bandeja y oculta
//!   las ventanas en Windows) y luego emite `RunEvent::ExitRequested` → `RunEvent::Exit`.
//! - Las consultas de ventana (`outer_position`, `cursor_position`, monitores…) usan `recv()` SIN
//!   timeout y esperan al hilo principal. Por eso el hilo principal NUNCA hace `join` del motor
//!   mientras el bucle de eventos está vivo: primero `request_stop()` (no bloqueante) y la espera
//!   acotada ocurre en `RunEvent::Exit`, cuando el bucle ya terminó.
//!
//! Secuencia de "Cerrar Miel": `quit` → `begin_shutdown` (motor: bandera + Condvar; subclase nativa
//! retirada) → `app.exit(0)` → `Exit` → `on_exit` espera (≤2 s) a que el motor termine. Todo es
//! idempotente: se puede invocar cuantas veces se quiera, desde cualquier ruta de salida.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::mascot::engine::Engine;

/// Plazo máximo de espera al motor tras el fin del bucle de eventos.
const ENGINE_STOP_TIMEOUT: Duration = Duration::from_secs(2);

/// Estado compartido del arranque y del cierre.
#[derive(Default)]
pub struct Lifecycle {
    /// La bandeja y el motor existen: ya hay forma de salir.
    ready: AtomicBool,
    /// La página terminó de cargarse.
    loaded: AtomicBool,
    /// La ventana ya se mostró la primera vez (recargas posteriores no la "des-ocultan").
    shown: AtomicBool,
    closing: AtomicBool,
}

impl Lifecycle {
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }
    pub fn mark_loaded(&self) {
        self.loaded.store(true, Ordering::SeqCst);
    }
    /// ¿Toca mostrar la ventana por primera vez? Solo con bandeja lista Y página cargada, y una sola vez.
    pub fn take_first_show(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
            && self.loaded.load(Ordering::SeqCst)
            && !self.closing.load(Ordering::SeqCst)
            && !self.shown.swap(true, Ordering::SeqCst)
    }
    /// Marca el cierre; `true` solo la primera vez.
    pub fn start_closing(&self) -> bool {
        !self.closing.swap(true, Ordering::SeqCst)
    }
    #[cfg(any(test, debug_assertions, feature = "diag"))]
    pub fn is_closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }
}

/// Muestra la ventana la primera vez, cuando todo está listo (no roba el foco: ventana no activable).
pub fn try_first_show(app: &AppHandle) {
    let Some(lc) = app.try_state::<Lifecycle>() else { return };
    if !lc.take_first_show() {
        return;
    }
    match app.get_webview_window("main") {
        Some(win) => {
            if let Err(e) = win.show() {
                eprintln!("[miel] no se pudo mostrar la ventana: {e}");
            }
        }
        None => eprintln!("[miel] ventana 'main' no encontrada al mostrarla"),
    }
}

/// Asa de la subclase nativa de pantalla (si se pudo instalar).
#[cfg(windows)]
#[derive(Default)]
pub struct WatchSlot(pub Mutex<Option<crate::display_watch::DisplayWatch>>);

#[cfg(not(windows))]
#[derive(Default)]
pub struct WatchSlot(pub Mutex<Option<()>>);

/// Primera parte del cierre, segura desde el hilo principal y repetible: ordena parar el motor
/// (sin esperar) y retira la subclase nativa de la ventana. Devuelve `true` la primera vez.
pub fn begin_shutdown(app: &AppHandle) -> bool {
    let first = app.try_state::<Lifecycle>().is_none_or(|l| l.start_closing());
    if let Some(engine) = app.try_state::<Engine>() {
        engine.request_stop(); // idempotente
    }
    #[cfg(windows)]
    if let Some(slot) = app.try_state::<WatchSlot>() {
        let watch = slot.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(w) = watch.as_ref() {
            let removed = w.remove(); // idempotente
            if crate::DIAG {
                eprintln!("[miel] cierre: subclase retirada ahora={removed}");
            }
        }
    }
    first
}

/// "Cerrar Miel": cierre controlado.
pub fn quit(app: &AppHandle) {
    begin_shutdown(app);
    app.exit(0);
}

/// `RunEvent::Exit`: el bucle de eventos ya terminó → es seguro esperar al motor (con plazo).
pub fn on_exit(app: &AppHandle) {
    begin_shutdown(app);
    if let Some(engine) = app.try_state::<Engine>() {
        let stopped = engine.shutdown_timeout(ENGINE_STOP_TIMEOUT);
        if crate::DIAG {
            eprintln!("[miel] cierre: motor detenido y unido={stopped}");
        }
        if !stopped {
            // Cierre FORZADO: el hilo del motor sigue atascado en una llamada del sistema. No se hace
            // join (bloquearía) y se continúa: al terminar el proceso, Windows destruye el hilo y
            // libera sus recursos. Esto queda registrado; no se afirma que el hilo terminara.
            eprintln!("[miel] el motor NO terminó en {ENGINE_STOP_TIMEOUT:?}; cierre forzado (el SO liberará el hilo)");
        }
    }
}

/// Error irrecuperable de arranque. En release no hay consola: se muestra un cuadro de diálogo
/// para que el fallo no sea silencioso. En desarrollo basta stderr.
pub fn fatal(message: &str) {
    eprintln!("[miel] ERROR: {message}");
    #[cfg(all(windows, not(debug_assertions)))]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        // SAFETY: llamada estándar de Win32 con cadenas válidas; sin ventana propietaria.
        unsafe {
            MessageBoxW(None, &HSTRING::from(message), &HSTRING::from("Miel"), MB_OK | MB_ICONERROR);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_show_needs_ready_and_loaded_and_happens_once() {
        let l = Lifecycle::default();
        assert!(!l.take_first_show(), "nada listo");
        l.mark_loaded();
        assert!(!l.take_first_show(), "página cargada pero sin bandeja/motor: no se muestra");
        l.mark_ready();
        assert!(l.take_first_show());
        assert!(!l.take_first_show(), "una sola vez: recargar la página no des-oculta a Miel");
    }

    #[test]
    fn first_show_works_in_either_order() {
        let l = Lifecycle::default();
        l.mark_ready();
        assert!(!l.take_first_show());
        l.mark_loaded();
        assert!(l.take_first_show());
    }

    #[test]
    fn no_first_show_while_closing() {
        let l = Lifecycle::default();
        l.mark_ready();
        l.mark_loaded();
        assert!(l.start_closing());
        assert!(!l.take_first_show());
    }

    #[test]
    fn closing_is_idempotent() {
        let l = Lifecycle::default();
        assert!(!l.is_closing());
        assert!(l.start_closing());
        assert!(!l.start_closing());
        assert!(!l.start_closing());
        assert!(l.is_closing());
    }
}
