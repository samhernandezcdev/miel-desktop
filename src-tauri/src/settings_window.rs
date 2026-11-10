//! Ventana de ajustes (E3): se crea desde Rust al pedirla (bandeja → «Ajustes…»), es de instancia
//! única y se DESTRUYE al cerrarla (no se oculta), de modo que el WebView2 de esa ventana se libera.
//!
//! - Etiqueta `settings`: la única con la capability `settings` (`capabilities/settings.json`).
//!   `main` no la usa ni gana permisos; sigue sin foco, click-through y siempre encima.
//! - Es una ventana normal: con marco, enfocable, en la barra de tareas y SIN «siempre encima».
//! - La creación se hace fuera del hilo principal (crear ventanas desde un manejador síncrono puede
//!   bloquear en Windows) y bajo un candado: dos clics seguidos crean una sola ventana y el segundo
//!   solo la enfoca.
//! - Al cerrar y reabrir enseguida, Tauri puede tardar en liberar la etiqueta: la creación se
//!   reintenta unos instantes antes de rendirse.

use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

/// Etiqueta de la ventana (y de la capability).
pub const LABEL: &str = "settings";
/// Página de la segunda entrada de build (`settings.html`).
pub const PAGE: &str = "settings.html";
/// Título nativo (la ventana no conoce el idioma de la persona hasta que carga la página).
pub const TITLE: &str = "Miel · Ajustes / Settings";

const WIDTH: f64 = 520.0;
const HEIGHT: f64 = 680.0;
const MIN_WIDTH: f64 = 360.0;
const MIN_HEIGHT: f64 = 420.0;
const ATTEMPTS: u32 = 10;
const RETRY_PAUSE: Duration = Duration::from_millis(100);

/// Serializa las aperturas (una ventana, aunque se pida varias veces seguidas).
static OPEN_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// No existía: se creó.
    Created,
    /// Ya existía: se trajo al frente y se enfocó.
    Focused,
}

fn focus_existing<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::Result<Opened>> {
    let win = app.get_webview_window(LABEL)?;
    let r = (|| {
        if win.is_minimized().unwrap_or(false) {
            win.unminimize()?;
        }
        win.show()?;
        win.set_focus()?;
        Ok(Opened::Focused)
    })();
    Some(r)
}

/// Abre la ventana de ajustes o, si ya existe, la enfoca. Bloquea hasta terminar: llamarla desde
/// un hilo propio (ver `open_in_background`), nunca desde el hilo principal de la aplicación.
pub fn open<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Opened> {
    open_with(app, ATTEMPTS, RETRY_PAUSE)
}

fn open_with<R: Runtime>(app: &AppHandle<R>, attempts: u32, pause: Duration) -> tauri::Result<Opened> {
    let _guard = OPEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut last = None;
    for attempt in 0..attempts.max(1) {
        if let Some(r) = focus_existing(app) {
            return r;
        }
        match WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(PAGE.into()))
            .title(TITLE)
            .inner_size(WIDTH, HEIGHT)
            .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
            .resizable(true)
            .maximizable(false)
            .decorations(true)
            .always_on_top(false)
            .skip_taskbar(false)
            .focused(true)
            .center()
            .build()
        {
            Ok(_) => return Ok(Opened::Created),
            Err(e) => {
                last = Some(e);
                if attempt + 1 < attempts {
                    sleep(pause);
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| tauri::Error::WindowNotFound))
}

/// Pide la apertura sin bloquear a quien llama (el menú de la bandeja corre en el hilo principal).
pub fn open_in_background<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("miel-settings-open".into()).spawn(move || {
        if let Err(e) = open(&app) {
            eprintln!("[miel] no se pudo abrir la ventana de ajustes: {e}");
        }
    });
    if let Err(e) = spawned {
        eprintln!("[miel] no se pudo lanzar la apertura de la ventana de ajustes: {e}");
    }
}

/// Cierra (destruye) la ventana si existe. Equivale a pulsar la X: pasa por `CloseRequested`.
#[cfg(any(debug_assertions, feature = "diag"))]
pub fn close<R: Runtime>(app: &AppHandle<R>) -> bool {
    match app.get_webview_window(LABEL) {
        Some(w) => w.close().is_ok(),
        None => false,
    }
}
