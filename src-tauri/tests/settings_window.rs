//! Pruebas de la ventana de ajustes (E3) sobre el runtime simulado de Tauri: una sola instancia,
//! destrucción al cerrar, reapertura y atributos de la ventana. No hay WebView2 ni sesión gráfica.
//!
//! LÍMITE del runtime simulado: no refleja el título, el marco ni el redimensionado de la ventana ni emite `Destroyed` al destruir una ventana
//! (la etiqueta no se libera), así que la destrucción al cerrar y la reapertura se verifican con la
//! aplicación real (ver docs/TEST_LOG.md, E3).
//!
//! Las pruebas comparten un candado de proceso (`open` serializa con un `static`) y cada una usa su
//! propia aplicación simulada, así que pueden ejecutarse en paralelo sin interferir.

use miel_lib::settings_window::{open, Opened, LABEL, PAGE};
use tauri::test::mock_app;
use tauri::Manager;

#[test]
fn first_open_creates_the_window_and_the_second_only_focuses_it() {
    let app = mock_app();
    assert!(app.get_webview_window(LABEL).is_none());
    assert_eq!(open(app.handle()).unwrap(), Opened::Created);
    assert_eq!(open(app.handle()).unwrap(), Opened::Focused, "un segundo clic la enfoca");
    assert_eq!(open(app.handle()).unwrap(), Opened::Focused);
    let n = app.webview_windows().keys().filter(|l| l.as_str() == LABEL).count();
    assert_eq!(n, 1, "una sola ventana");
}

#[test]
fn the_window_is_a_normal_one_not_always_on_top_and_with_the_expected_label() {
    let app = mock_app();
    open(app.handle()).unwrap();
    let w = app.get_webview_window(LABEL).expect("ventana");
    assert_eq!(w.label(), "settings");
    assert!(!w.is_always_on_top().unwrap(), "sin siempre-encima");
    assert_eq!(PAGE, "settings.html");
}

#[test]
fn concurrent_open_requests_create_a_single_window() {
    let app = mock_app();
    let handle = app.handle().clone();
    let results: Vec<Opened> = (0..6)
        .map(|_| {
            let h = handle.clone();
            std::thread::spawn(move || open(&h).unwrap())
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| **r == Opened::Created).count(), 1, "{results:?}");
    assert_eq!(results.iter().filter(|r| **r == Opened::Focused).count(), 5);
}

#[test]
fn the_settings_window_does_not_touch_the_main_window() {
    let app = mock_app();
    // `main` simulada: ni se crea ni se modifica al abrir ajustes
    assert!(app.get_webview_window("main").is_none());
    open(app.handle()).unwrap();
    assert!(app.get_webview_window("main").is_none());
}

#[test]
fn the_tray_path_opens_the_window_off_the_calling_thread_and_twice_still_gives_one() {
    // `open_in_background` es lo que ejecuta «Ajustes…» en la bandeja (el menú corre en el hilo principal).
    use miel_lib::settings_window::open_in_background;
    let app = mock_app();
    open_in_background(app.handle());
    open_in_background(app.handle());
    let appeared = (0..200).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(10));
        app.get_webview_window(LABEL).is_some()
    });
    assert!(appeared, "la ventana debe aparecer sin bloquear a quien la pide");
    std::thread::sleep(std::time::Duration::from_millis(150)); // deja terminar la segunda petición
    assert_eq!(app.webview_windows().keys().filter(|l| l.as_str() == LABEL).count(), 1);
}
