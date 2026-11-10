//! Pruebas del menú de la bandeja con los elementos REALES de Tauri/muda sobre el runtime
//! simulado (`tauri::test`): se leen texto, habilitado y casilla tras `MenuSync::sync`, sin mostrar
//! ningún menú nativo y sin sesión gráfica interactiva.

use miel_lib::mascot::state::{Behavior, Expression, View};
use miel_lib::tray::{
    id, make_menu, MenuSync, TrayItems, LABEL_FOLLOW, LABEL_HIDE, LABEL_SHOW,
};
use tauri::test::{mock_app, MockRuntime};

fn view(paused: bool, hidden: bool, follow: bool) -> View {
    View {
        version: 0,
        behavior: Behavior::Idle,
        expression: Expression::Idle,
        paused,
        hidden,
        follow_enabled: follow,
        message: None,
        suppressed: false,
        sleeping: false,
    }
}

/// (Mostrar habilitado, Ocultar habilitado, casilla de seguimiento, texto de Pausar/Reanudar)
fn read_real(sync: &MenuSync<TrayItems<MockRuntime>>) -> (bool, bool, bool, String) {
    let it = sync.sink();
    (
        it.show.is_enabled().unwrap(),
        it.hide.is_enabled().unwrap(),
        it.follow.is_checked().unwrap(),
        it.pause.text().unwrap(),
    )
}

#[test]
fn real_menu_items_have_the_expected_labels_and_initial_state() {
    let app = mock_app();
    let (menu, items) = make_menu(app.handle(), &view(false, false, true)).expect("menú");
    assert_eq!(items.show.text().unwrap(), LABEL_SHOW);
    assert_eq!(items.hide.text().unwrap(), LABEL_HIDE);
    assert_eq!(items.follow.text().unwrap(), LABEL_FOLLOW);
    assert_eq!(items.pause.text().unwrap(), "Pausar");
    assert_eq!((items.show.is_enabled().unwrap(), items.hide.is_enabled().unwrap()), (false, true));
    assert!(items.follow.is_checked().unwrap());
    // 5 acciones + 2 separadores, en el orden pedido
    let ids: Vec<String> = menu.items().unwrap().iter().map(|i| i.id().0.clone()).collect();
    assert_eq!(ids.len(), 7);
    let actions: Vec<&str> = ids.iter().map(String::as_str).filter(|i| i.starts_with("miel.")).collect();
    assert_eq!(actions, [id::SHOW, id::HIDE, id::FOLLOW, id::PAUSE, id::QUIT]);
}

#[test]
fn real_menu_items_follow_the_engine_view_after_sync() {
    let app = mock_app();
    let (_menu, items) = make_menu(app.handle(), &view(false, false, true)).unwrap();
    let sync = MenuSync::new(items);
    // (pausada, oculta, seguimiento) → (Mostrar, Ocultar, casilla, texto de pausa)
    let cases = [
        ((false, false, true), (false, true, true, "Pausar")),
        ((true, false, true), (false, true, true, "Reanudar")),
        ((true, true, true), (true, false, true, "Reanudar")), // oculta y pausada
        ((true, false, true), (false, true, true, "Reanudar")), // mostrar: sigue pausada
        ((true, false, false), (false, true, false, "Reanudar")), // seguimiento desactivado en pausa
        ((false, false, false), (false, true, false, "Pausar")), // reanudar conserva la preferencia
        ((false, true, false), (true, false, false, "Pausar")), // oculta sin pausa
        ((false, false, true), (false, true, true, "Pausar")),
    ];
    for ((p, h, f), (show, hide, chk, txt)) in cases {
        sync.sync(&view(p, h, f));
        assert_eq!(read_real(&sync), (show, hide, chk, txt.to_string()), "estado real ({p},{h},{f})");
    }
}

#[test]
fn real_menu_stays_consistent_for_any_sequence_of_states() {
    let app = mock_app();
    let (_menu, items) = make_menu(app.handle(), &view(false, false, true)).unwrap();
    let sync = MenuSync::new(items);
    let mut seed = 99u32;
    let mut next = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed >> 16
    };
    for _ in 0..300 {
        let v = view(next() % 2 == 0, next() % 2 == 0, next() % 2 == 0);
        sync.sync(&v);
        let (show, hide, chk, txt) = read_real(&sync);
        assert_eq!((show, hide, chk), (v.hidden, !v.hidden, v.follow_enabled));
        assert_eq!(txt, if v.paused { "Reanudar" } else { "Pausar" });
    }
}

#[test]
fn real_checkbox_is_corrected_after_a_native_auto_toggle() {
    // Al hacer clic el sistema invierte la casilla por su cuenta; el motor luego publica el estado
    // real y la sincronización debe dejar la casilla igual a ese estado.
    let app = mock_app();
    let (_menu, items) = make_menu(app.handle(), &view(false, false, true)).unwrap();
    let sync = MenuSync::new(items);
    sync.sync(&view(false, false, true));
    sync.sink().follow.set_checked(false).unwrap(); // "clic" nativo
    sync.sync(&view(false, false, false)); // el motor confirma: desactivado
    assert!(!sync.sink().follow.is_checked().unwrap());
    // y si el motor NO cambió (orden descartada), la sincronización forzada restaura la real
    sync.sink().follow.set_checked(true).unwrap();
    sync.invalidate();
    sync.sync(&view(false, false, false));
    assert!(!sync.sink().follow.is_checked().unwrap());
}

#[test]
fn sync_updates_items_in_place_without_rebuilding_the_menu() {
    let app = mock_app();
    let (menu, items) = make_menu(app.handle(), &view(false, false, true)).unwrap();
    let before: Vec<String> = menu.items().unwrap().iter().map(|i| i.id().0.clone()).collect();
    let sync = MenuSync::new(items);
    for k in 0..20 {
        sync.sync(&view(k % 2 == 0, k % 3 == 0, k % 5 != 0));
    }
    let after: Vec<String> = menu.items().unwrap().iter().map(|i| i.id().0.clone()).collect();
    assert_eq!(before, after, "mismos elementos: no se reconstruye el menú");
}
