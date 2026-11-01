#[cfg(any(debug_assertions, feature = "diag"))]
mod diag;
#[cfg(windows)]
mod display_watch;
mod errlog;
mod lifecycle;
pub mod mascot;
mod settings;
pub mod tray;
mod window;

use std::sync::Arc;

use serde::Serialize;
use tauri::{Manager, RunEvent, WebviewWindow, WindowEvent};

use lifecycle::Lifecycle;
use mascot::engine::{Cmd, Engine, EngineConfig};
use mascot::host::TauriHost;
use mascot::messages::TalkerConfig;
use mascot::state::View;
use settings::{LoadOutcome, Settings, SettingsStore};

/// Herramientas de diagnóstico (variables de entorno `MIEL_*`, registros con marca de tiempo):
/// activas en compilaciones de depuración o con `--features diag`; inexistentes en el instalador.
pub const DIAG: bool = cfg!(any(debug_assertions, feature = "diag"));

/// Consulta de solo lectura del estado actual: sincronización inicial del frontend
/// (el evento `mascot://view` solo se emite en cambios, por lo que React pide este snapshot
/// DESPUÉS de suscribirse y descarta cualquier versión anterior).
#[tauri::command]
fn get_view(engine: tauri::State<'_, Engine>) -> View {
    let view = engine.view();
    if DIAG {
        eprintln!("[miel] get_view -> v{}", view.version);
    }
    view
}

/// Preferencias de presentación (fijas durante la ejecución): tamaño del sprite y movimiento reducido.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DisplayPrefs {
    pet_size: u32,
    reduced_motion: bool,
}

#[tauri::command]
fn get_display_prefs(store: tauri::State<'_, Arc<SettingsStore>>) -> DisplayPrefs {
    let s = store.get();
    DisplayPrefs { pet_size: s.pet_size, reduced_motion: s.reduced_motion }
}

/// Informe de un error del frontend (React/WebView2). Se sanea, se limita en cantidad y tamaño y
/// se guarda solo en un archivo local del directorio de registros de la aplicación.
#[tauri::command]
fn report_frontend_error(app: tauri::AppHandle, kind: String, message: String) {
    if kind.len() > 64 || message.len() > 4096 {
        return; // entrada desproporcionada: se descarta
    }
    match app.path().app_log_dir() {
        Ok(dir) => {
            errlog::report(&dir.join("frontend-errors.log"), &kind, &message);
        }
        Err(e) => eprintln!("[miel] sin directorio de registros: {e}"),
    }
}

/// Secuencia: la ventana nace oculta (tauri.conf.json) → se dimensiona, configura y coloca en `setup`
/// → arranca el motor → se muestra solo cuando el WebView termina de cargar la página.
fn prepare(win: &WebviewWindow, pet_size: u32) {
    if let Err(e) = window::configure(win, pet_size) {
        eprintln!("[miel] no se pudo configurar la ventana: {e}");
    }
    if let Err(e) = window::place_initial(win) {
        eprintln!("[miel] no se pudo posicionar la ventana: {e}");
    }
}

/// Avisos nativos de resolución / monitores / área de trabajo (Windows). Si no se pueden
/// instalar, Miel sigue funcionando: la revisión al despertar del motor actúa como red de seguridad.
/// El asa se guarda para retirar la subclase explícitamente al cerrar.
fn watch_displays(app: &tauri::AppHandle, win: &WebviewWindow) {
    #[cfg(windows)]
    {
        let handle = app.clone();
        match win.hwnd() {
            Ok(hwnd) => {
                let r = display_watch::install(hwnd, move || {
                    if let Some(engine) = handle.try_state::<Engine>() {
                        engine.send(Cmd::GeometryChanged);
                    }
                });
                match r {
                    Ok(watch) => {
                        if let Some(slot) = app.try_state::<lifecycle::WatchSlot>() {
                            *slot.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(watch);
                        }
                        if let Some(engine) = app.try_state::<Engine>() {
                            engine.send(Cmd::NativeGeometryEvents);
                        }
                    }
                    Err(e) => eprintln!("[miel] sin aviso nativo de pantalla: {e}"),
                }
            }
            Err(e) => eprintln!("[miel] sin HWND para el aviso de pantalla: {e}"),
        }
    }
    #[cfg(not(windows))]
    let _ = (app, win);
}

/// Configuración del motor a partir de las preferencias. Con `DIAG`, `MIEL_FAST=1` acelera los
/// tiempos para observar todos los comportamientos en segundos (no existe en el instalador).
fn engine_config(s: &Settings) -> EngineConfig {
    let mut cfg = EngineConfig {
        speed: s.speed,
        anchor_frac: window::anchor_frac(s.pet_size),
        initial_follow: s.follow_enabled,
        initial_paused: s.paused,
        talker: TalkerConfig {
            enabled: s.messages_enabled,
            cooldown: u64::from(s.message_interval_minutes) * 60_000,
            ..TalkerConfig::default()
        }
        .validated(),
        ..EngineConfig::default()
    };
    if DIAG && std::env::var_os("MIEL_FAST").is_some() {
        use mascot::state::Timings;
        cfg.timings = Timings {
            idle: (3_000, 5_000),
            follow: (9_000, 9_000),
            rest: (8_000, 8_000),
            reaction: (5_000, 5_000),
            rest_settle: 2_000,
            follow_cooldown: 4_000,
            reaction_cooldown: 4_000,
            weights: (30, 20, 50),
        };
        cfg.talker = TalkerConfig { cooldown: 12_000, initial_delay: (3_000, 3_000), ..cfg.talker };
    }
    cfg
}

fn describe_outcome(o: &LoadOutcome) -> Option<String> {
    match o {
        LoadOutcome::Missing => None,
        LoadOutcome::Loaded { invalid, clamped } if invalid.is_empty() && clamped.is_empty() => None,
        LoadOutcome::Loaded { invalid, clamped } => {
            Some(format!("campos inválidos restablecidos {invalid:?}, campos ajustados {clamped:?}"))
        }
        LoadOutcome::Corrupt { backup } => {
            Some(format!("archivo corrupto; se usan valores por defecto (copia: {backup:?})"))
        }
        LoadOutcome::Future { version, backup } => {
            Some(format!("esquema futuro v{version}; se usan los campos conocidos (copia: {backup:?})"))
        }
        LoadOutcome::Unreadable(e) => Some(format!("no se pudo leer: {e}")),
    }
}

/// Acciones que pide una segunda ejecución sobre la instancia existente: solo mostrarla.
/// (No cambia la pausa ni el seguimiento y no activa la ventana: es no activable.)
pub fn second_instance_commands() -> [Cmd; 1] {
    [Cmd::Hidden(false)]
}

pub fn run() {
    let built = tauri::Builder::default()
        // La instancia única debe registrarse PRIMERO (recomendación del plugin): una segunda
        // ejecución envía su aviso a la primera y termina antes de crear ventana, motor ni bandeja.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // No se registran los argumentos ni el directorio de trabajo de la segunda ejecución.
            if let Some(engine) = app.try_state::<Engine>() {
                for cmd in second_instance_commands() {
                    engine.send(cmd);
                }
            }
        }))
        .manage(Lifecycle::default())
        .manage(lifecycle::WatchSlot::default())
        .invoke_handler(tauri::generate_handler![get_view, get_display_prefs, report_frontend_error])
        .setup(|app| {
            let win = app
                .get_webview_window("main")
                .ok_or("ventana 'main' no encontrada")?;

            // Preferencias: carga tolerante (nunca impide el arranque).
            let config_dir = app.path().app_config_dir()?;
            let store = Arc::new(SettingsStore::load(config_dir.join(settings::FILE_NAME)));
            if let Some(msg) = describe_outcome(store.outcome()) {
                eprintln!("[miel] preferencias: {msg}");
            }
            let prefs = store.get();
            app.manage(Arc::clone(&store));

            prepare(&win, prefs.pet_size);
            let host = Arc::new(TauriHost::new(app.handle().clone(), win.clone(), Arc::clone(&store)));
            let engine = Engine::spawn(host, engine_config(&prefs));
            let initial = engine.view();
            if DIAG {
                eprintln!(
                    "[miel] inicio: paused={} hidden={} follow={} pet_size={} preferencias={:?}",
                    initial.paused,
                    initial.hidden,
                    initial.follow_enabled,
                    prefs.pet_size,
                    store.outcome()
                );
            }
            app.manage(engine);
            // Sin bandeja no hay forma de cerrar una mascota click-through: si falla, se aborta el
            // arranque (la ventana sigue oculta) y se avisa; nunca queda un proceso inaccesible.
            if let Err(e) = tray::build(app.handle(), &initial) {
                lifecycle::begin_shutdown(app.handle());
                lifecycle::fatal(&format!("No se pudo crear el icono de la bandeja del sistema: {e}"));
                return Err(e);
            }
            watch_displays(app.handle(), &win);
            #[cfg(any(debug_assertions, feature = "diag"))]
            diag::spawn_script(app.handle());
            if let Some(lc) = app.try_state::<Lifecycle>() {
                lc.mark_ready();
            }
            lifecycle::try_first_show(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Cambio de escala DPI (evento de Tauri; el resto de cambios de pantalla llegan por
            // `display_watch`). Ambos solo avisan al motor, que agrupa y actúa en su hilo.
            if let WindowEvent::ScaleFactorChanged { .. } = event {
                if let Some(engine) = window.app_handle().try_state::<Engine>() {
                    engine.send(Cmd::GeometryChanged);
                }
            }
        })
        .on_page_load(|webview, payload| {
            if let tauri::webview::PageLoadEvent::Finished = payload.event() {
                let app = webview.app_handle();
                if let Some(lc) = app.try_state::<Lifecycle>() {
                    lc.mark_loaded();
                }
                lifecycle::try_first_show(app);
            }
        })
        .build(tauri::generate_context!());

    let app = match built {
        Ok(app) => app,
        Err(e) => {
            lifecycle::fatal(&format!("Miel no pudo iniciarse: {e}"));
            std::process::exit(1);
        }
    };

    app.run(|handle, event| match event {
        // Cualquier ruta de salida (bandeja, cierre de la última ventana, SO) para el motor y
        // retira la subclase sin bloquear: el bucle de eventos sigue vivo.
        RunEvent::ExitRequested { .. } => {
            lifecycle::begin_shutdown(handle);
        }
        // El bucle ya terminó: ahora sí se espera (con plazo) a que el hilo del motor acabe.
        RunEvent::Exit => lifecycle::on_exit(handle),
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_instance_only_asks_to_show_and_never_touches_pause_or_follow() {
        let cmds = second_instance_commands();
        assert_eq!(cmds, [Cmd::Hidden(false)]);
        assert!(cmds
            .iter()
            .all(|c| !matches!(c, Cmd::Pause(_) | Cmd::TogglePause | Cmd::FollowEnabled(_) | Cmd::ToggleFollow)));
    }

    #[test]
    fn engine_config_reflects_every_preference() {
        let s = Settings {
            follow_enabled: false,
            messages_enabled: false,
            message_interval_minutes: 30,
            pet_size: 128,
            speed: 400.0,
            paused: true,
            ..Settings::default()
        };
        let c = engine_config(&s);
        assert_eq!(c.speed, 400.0);
        assert!(!c.initial_follow && c.initial_paused);
        assert!(!c.talker.enabled);
        assert_eq!(c.talker.cooldown, 30 * 60_000);
        assert_eq!(c.anchor_frac, window::anchor_frac(128));
    }

    #[test]
    fn default_preferences_give_the_documented_defaults() {
        let c = engine_config(&Settings::default());
        assert_eq!(c.talker.cooldown, 12 * 60_000, "12 minutos");
        assert_eq!(c.anchor_frac, (0.5, 0.64));
        assert!(c.initial_follow && !c.initial_paused && c.talker.enabled);
    }

    #[test]
    fn hidden_is_never_restored_from_preferences() {
        // Settings no tiene campo `hidden`: Miel siempre puede aparecer al iniciar.
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert!(v.get("hidden").is_none());
    }

    #[test]
    fn corrupt_preferences_are_described_without_blocking_startup() {
        assert!(describe_outcome(&LoadOutcome::Corrupt { backup: None }).is_some());
        assert!(describe_outcome(&LoadOutcome::Missing).is_none());
        assert!(describe_outcome(&LoadOutcome::Loaded { invalid: vec![], clamped: vec![] }).is_none());
    }
}
