//! Bandeja del sistema: icono único, menú y su sincronización con el estado real del motor.
//!
//! - Rust es la única fuente de verdad: las acciones del menú se traducen en órdenes al motor y
//!   el menú se actualiza a partir de la VISTA del motor (`View`), nunca de copias locales.
//! - Nada de esto depende de React: la bandeja funciona aunque el renderer falle.
//! - El hilo del motor nunca toca el menú: pide la actualización con `run_on_main_thread`
//!   (no bloqueante), de modo que no puede haber interbloqueo con el hilo principal.
//! - La lógica (modelo del menú, despacho de acciones, deduplicación de actualizaciones) es pura y
//!   se prueba con mocks (`MenuSink`, `Control`); la parte de Tauri es una capa fina.

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Runtime, Wry};

use crate::lifecycle;
use crate::mascot::engine::{Cmd, Engine};
use crate::mascot::state::View;

/// Identificador estable del único icono de bandeja.
pub const TRAY_ID: &str = "miel-tray";

pub mod id {
    pub const SHOW: &str = "miel.show";
    pub const HIDE: &str = "miel.hide";
    pub const FOLLOW: &str = "miel.follow";
    pub const PAUSE: &str = "miel.pause";
    pub const SETTINGS: &str = "miel.settings";
    pub const SHOW_ANYWAY: &str = "miel.show_anyway";
    pub const QUIT: &str = "miel.quit";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Hide,
    ToggleFollow,
    TogglePause,
    /// «Ajustes…» (E3): abre la ventana de ajustes o la enfoca si ya existe.
    OpenSettings,
    /// «Mostrar de todos modos» (E4b): anula la supresión de No molestar del episodio actual.
    ShowAnyway,
    Quit,
}

impl TrayAction {
    pub fn from_id(menu_id: &str) -> Option<Self> {
        Some(match menu_id {
            id::SHOW => Self::Show,
            id::HIDE => Self::Hide,
            id::FOLLOW => Self::ToggleFollow,
            id::PAUSE => Self::TogglePause,
            id::SETTINGS => Self::OpenSettings,
            id::SHOW_ANYWAY => Self::ShowAnyway,
            id::QUIT => Self::Quit,
            _ => return None,
        })
    }
}

/// Lo que el menú debe mostrar para un estado dado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuModel {
    pub show_enabled: bool,
    pub hide_enabled: bool,
    pub follow_checked: bool,
    pub pause_text: &'static str,
    /// «Ajustes…» (E3). Hoy siempre disponible: abrir la ventana no depende del estado de Miel (ni
    /// oculta ni pausada ni suprimida), pero vive en el modelo para que el menú siga siendo una función
    /// pura de la vista.
    pub settings_enabled: bool,
    /// «Mostrar de todos modos» (E4b): el elemento SOLO existe en el menú mientras hay supresión en vigor y la
    /// persona no ocultó a Miel (si la ocultó, «Mostrar» ya hace lo mismo: quita `hidden` y anula el episodio).
    /// Mostrar/Ocultar siguen reflejando únicamente `hidden`.
    pub show_anyway: bool,
    /// Ayuda del icono (tooltip): «Miel está oculta por No molestar» mientras la supresión esté en vigor.
    pub tooltip: &'static str,
}

pub fn menu_model(v: &View) -> MenuModel {
    MenuModel {
        show_enabled: v.hidden,
        hide_enabled: !v.hidden,
        follow_checked: v.follow_enabled,
        pause_text: if v.paused { "Reanudar" } else { "Pausar" },
        settings_enabled: true,
        show_anyway: v.suppressed && !v.hidden,
        tooltip: if v.suppressed { TOOLTIP_SUPPRESSED } else { TOOLTIP_DEFAULT },
    }
}

/// Destino de las actualizaciones del menú (implementado por los elementos reales y por mocks).
pub trait MenuSink {
    fn set_show_enabled(&self, enabled: bool);
    fn set_hide_enabled(&self, enabled: bool);
    fn set_follow_checked(&self, checked: bool);
    fn set_pause_text(&self, text: &str);
    fn set_settings_enabled(&self, enabled: bool);
    fn set_show_anyway_visible(&self, visible: bool);
    fn set_tooltip(&self, text: &str);
}

/// `sync_tray_menu`: aplica el modelo al menú SOLO en lo que cambió desde la última vez
/// (sin reconstruir el menú ni el icono, y sin llamadas redundantes al sistema).
pub struct MenuSync<S: MenuSink> {
    sink: S,
    last: Mutex<Option<MenuModel>>,
}

impl<S: MenuSink> MenuSync<S> {
    pub fn new(sink: S) -> Self {
        Self { sink, last: Mutex::new(None) }
    }

    /// Acceso de solo lectura al destino (las pruebas leen los elementos reales del menú).
    #[allow(dead_code)]
    pub fn sink(&self) -> &S {
        &self.sink
    }

    /// Devuelve `true` si hubo algo que actualizar.
    pub fn sync(&self, view: &View) -> bool {
        let new = menu_model(view);
        // el candado se suelta antes de llamar al sink (las llamadas a menús nativos pueden tardar)
        let old = {
            let mut g = self.last.lock().unwrap_or_else(|e| e.into_inner());
            let old = *g;
            if old == Some(new) {
                return false;
            }
            *g = Some(new);
            old
        };
        if old.is_none_or(|o| o.show_enabled != new.show_enabled) {
            self.sink.set_show_enabled(new.show_enabled);
        }
        if old.is_none_or(|o| o.hide_enabled != new.hide_enabled) {
            self.sink.set_hide_enabled(new.hide_enabled);
        }
        if old.is_none_or(|o| o.follow_checked != new.follow_checked) {
            self.sink.set_follow_checked(new.follow_checked);
        }
        if old.is_none_or(|o| o.pause_text != new.pause_text) {
            self.sink.set_pause_text(new.pause_text);
        }
        if old.is_none_or(|o| o.settings_enabled != new.settings_enabled) {
            self.sink.set_settings_enabled(new.settings_enabled);
        }
        // E4b: el menú y el icono nacen con el modelo inicial (`make_menu`/`build`); en la primera sincronización
        // solo hay algo que hacer si ese modelo NO es el de reposo (elemento ausente y tooltip «Miel»).
        if old.map_or(new.show_anyway, |o| o.show_anyway != new.show_anyway) {
            self.sink.set_show_anyway_visible(new.show_anyway);
        }
        if old.map_or(new.tooltip != TOOLTIP_DEFAULT, |o| o.tooltip != new.tooltip) {
            self.sink.set_tooltip(new.tooltip);
        }
        true
    }

    /// Fuerza que la próxima `sync` reaplique todo (p. ej. si el sistema alteró el menú).
    #[allow(dead_code)]
    pub fn invalidate(&self) {
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Acciones que el controlador puede pedir al resto de la aplicación.
pub trait Control {
    fn send(&self, cmd: Cmd);
    fn open_settings(&self);
    fn quit(&self);
}

/// Traduce una acción del menú en órdenes al motor. Todas son idempotentes o alternan sobre el
/// estado REAL del motor; ninguna espera respuesta (no bloquea el hilo principal).
pub fn dispatch(action: TrayAction, ctl: &dyn Control) {
    match action {
        TrayAction::Show => ctl.send(Cmd::Hidden(false)),
        TrayAction::Hide => ctl.send(Cmd::Hidden(true)),
        TrayAction::ToggleFollow => ctl.send(Cmd::ToggleFollow),
        TrayAction::TogglePause => ctl.send(Cmd::TogglePause),
        TrayAction::OpenSettings => ctl.open_settings(),
        TrayAction::ShowAnyway => ctl.send(Cmd::DndOverride),
        TrayAction::Quit => ctl.quit(),
    }
}

// ---- capa Tauri -----------------------------------------------------------------------------

/// Elementos reales del menú (clones baratos de asas) y el icono.
pub struct TrayItems<R: Runtime> {
    pub show: MenuItem<R>,
    pub hide: MenuItem<R>,
    pub follow: CheckMenuItem<R>,
    pub pause: MenuItem<R>,
    pub settings: MenuItem<R>,
    /// «Mostrar de todos modos» (E4b): se inserta en `menu` solo mientras hay supresión y se retira después.
    pub show_anyway: MenuItem<R>,
    menu: Menu<R>,
    show_anyway_in_menu: Mutex<bool>,
    /// El icono se conserva mientras viva el menú (no se recrea al cambiar de estado).
    tray: Mutex<Option<TrayIcon<R>>>,
}

fn log_err<T, E: std::fmt::Display>(what: &str, r: Result<T, E>) {
    if let Err(e) = r {
        eprintln!("[miel] bandeja: {what} falló: {e}");
    }
}

impl<R: Runtime> MenuSink for TrayItems<R> {
    fn set_show_enabled(&self, enabled: bool) {
        log_err("Mostrar", self.show.set_enabled(enabled));
    }
    fn set_hide_enabled(&self, enabled: bool) {
        log_err("Ocultar", self.hide.set_enabled(enabled));
    }
    fn set_follow_checked(&self, checked: bool) {
        log_err("Seguir cursor", self.follow.set_checked(checked));
    }
    fn set_pause_text(&self, text: &str) {
        log_err("Pausar/Reanudar", self.pause.set_text(text));
    }
    fn set_settings_enabled(&self, enabled: bool) {
        log_err("Ajustes", self.settings.set_enabled(enabled));
    }
    fn set_show_anyway_visible(&self, visible: bool) {
        let mut inside = self.show_anyway_in_menu.lock().unwrap_or_else(|e| e.into_inner());
        if visible == *inside {
            return;
        }
        // justo después de «Ocultar Miel» (posición 2: tras Mostrar y Ocultar)
        let r = if visible { self.menu.insert(&self.show_anyway, 2) } else { self.menu.remove(&self.show_anyway) };
        match r {
            Ok(()) => *inside = visible,
            Err(e) => eprintln!("[miel] bandeja: Mostrar de todos modos falló: {e}"),
        }
    }
    fn set_tooltip(&self, text: &str) {
        if let Some(tray) = self.tray.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            log_err("tooltip", tray.set_tooltip(Some(text)));
        }
    }
}

/// Estado gestionado por Tauri: el sincronizador del menú real.
pub type TraySync = MenuSync<TrayItems<Wry>>;

/// `Control` real: órdenes al motor gestionado por Tauri y cierre controlado.
pub struct AppControl(pub AppHandle);

impl Control for AppControl {
    fn send(&self, cmd: Cmd) {
        match self.0.try_state::<Engine>() {
            Some(engine) => engine.send(cmd),
            None => eprintln!("[miel] bandeja: motor no disponible para {cmd:?}"),
        }
    }
    fn open_settings(&self) {
        crate::settings_window::open_in_background(&self.0);
    }
    fn quit(&self) {
        lifecycle::quit(&self.0);
    }
}

/// Crea el icono y el menú (una sola vez) y los deja sincronizados con `initial`.
/// Cualquier error se devuelve: sin bandeja no hay forma de salir, y el llamador debe abortar el
/// arranque en lugar de dejar una mascota click-through inaccesible.
pub fn build(app: &AppHandle, initial: &View) -> Result<(), Box<dyn std::error::Error>> {
    if crate::DIAG && std::env::var_os("MIEL_SIMULATE_TRAY_FAILURE").is_some() {
        return Err("fallo simulado de la bandeja (MIEL_SIMULATE_TRAY_FAILURE)".into());
    }
    let default = app
        .default_window_icon()
        .ok_or("la aplicación no tiene icono por defecto")?;
    let icon = Image::new_owned(default.rgba().to_vec(), default.width(), default.height());

    let (menu, items) = make_menu(app, initial)?;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip(TOOLTIP_DEFAULT)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match TrayAction::from_id(event.id().as_ref()) {
            Some(action) => dispatch(action, &AppControl(app.clone())),
            None => eprintln!("[miel] bandeja: acción desconocida {:?}", event.id()),
        })
        .build(app)?;

    *items.tray.lock().unwrap_or_else(|e| e.into_inner()) = Some(tray);
    let sync = MenuSync::new(items);
    sync.sync(initial); // ya creado con el modelo inicial: aplica todo una vez
    app.manage(sync);
    Ok(())
}

/// Textos del menú (fuente única; los tests los comprueban contra los elementos reales).
pub const LABEL_SHOW: &str = "Mostrar Miel";
pub const LABEL_HIDE: &str = "Ocultar Miel";
pub const LABEL_FOLLOW: &str = "Seguir cursor";
pub const LABEL_SETTINGS: &str = "Ajustes…";
pub const LABEL_QUIT: &str = "Cerrar Miel";
pub const LABEL_SHOW_ANYWAY: &str = "Mostrar de todos modos";
/// Ayuda del icono de la bandeja (tooltip).
pub const TOOLTIP_DEFAULT: &str = "Miel";
pub const TOOLTIP_SUPPRESSED: &str = "Miel está oculta por No molestar";

/// Crea los elementos del menú con el modelo inicial. Genérica sobre el runtime: en producción
/// es `Wry`; en pruebas, `MockRuntime` (sin menú nativo ni sesión gráfica).
pub fn make_menu<R: Runtime>(
    app: &AppHandle<R>,
    initial: &View,
) -> Result<(Menu<R>, TrayItems<R>), Box<dyn std::error::Error>> {
    let m = menu_model(initial);
    let show = MenuItem::with_id(app, id::SHOW, LABEL_SHOW, m.show_enabled, None::<&str>)?;
    let hide = MenuItem::with_id(app, id::HIDE, LABEL_HIDE, m.hide_enabled, None::<&str>)?;
    let follow =
        CheckMenuItem::with_id(app, id::FOLLOW, LABEL_FOLLOW, true, m.follow_checked, None::<&str>)?;
    let pause = MenuItem::with_id(app, id::PAUSE, m.pause_text, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, id::SETTINGS, LABEL_SETTINGS, m.settings_enabled, None::<&str>)?;
    let quit = MenuItem::with_id(app, id::QUIT, LABEL_QUIT, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let show_anyway =
        MenuItem::with_id(app, id::SHOW_ANYWAY, LABEL_SHOW_ANYWAY, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &sep1, &follow, &pause, &sep2, &settings, &sep3, &quit])?;
    let items = TrayItems {
        show,
        hide,
        follow,
        pause,
        settings,
        show_anyway,
        menu: menu.clone(),
        show_anyway_in_menu: Mutex::new(false),
        tray: Mutex::new(None),
    };
    if m.show_anyway {
        items.set_show_anyway_visible(true); // arranque ya suprimido (no ocurre hoy, pero el modelo manda)
    }
    Ok((menu, items))
}

/// Actualiza el menú desde el hilo principal. Se invoca vía `run_on_main_thread`.
pub fn sync_on_main(app: &AppHandle, view: &View) {
    if let Some(s) = app.try_state::<TraySync>() {
        s.sync(view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mascot::state::{Behavior, Expression};
    use std::cell::RefCell;

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

    #[derive(Default)]
    struct FakeSink {
        calls: RefCell<Vec<String>>,
        show: RefCell<bool>,
        hide: RefCell<bool>,
        follow: RefCell<bool>,
        pause: RefCell<String>,
        settings: RefCell<bool>,
        show_anyway: RefCell<bool>,
        tooltip: RefCell<String>,
    }
    impl MenuSink for &FakeSink {
        fn set_show_enabled(&self, e: bool) {
            *self.show.borrow_mut() = e;
            self.calls.borrow_mut().push(format!("show={e}"));
        }
        fn set_hide_enabled(&self, e: bool) {
            *self.hide.borrow_mut() = e;
            self.calls.borrow_mut().push(format!("hide={e}"));
        }
        fn set_follow_checked(&self, c: bool) {
            *self.follow.borrow_mut() = c;
            self.calls.borrow_mut().push(format!("follow={c}"));
        }
        fn set_pause_text(&self, t: &str) {
            *self.pause.borrow_mut() = t.to_string();
            self.calls.borrow_mut().push(format!("pause={t}"));
        }
        fn set_settings_enabled(&self, e: bool) {
            *self.settings.borrow_mut() = e;
            self.calls.borrow_mut().push(format!("settings={e}"));
        }
        fn set_show_anyway_visible(&self, v: bool) {
            *self.show_anyway.borrow_mut() = v;
            self.calls.borrow_mut().push(format!("show_anyway={v}"));
        }
        fn set_tooltip(&self, t: &str) {
            *self.tooltip.borrow_mut() = t.to_string();
            self.calls.borrow_mut().push(format!("tooltip={t}"));
        }
    }

    #[derive(Default)]
    struct FakeControl {
        sent: RefCell<Vec<Cmd>>,
        quits: RefCell<u32>,
        settings_opens: RefCell<u32>,
    }
    impl Control for FakeControl {
        fn send(&self, cmd: Cmd) {
            self.sent.borrow_mut().push(cmd);
        }
        fn open_settings(&self) {
            *self.settings_opens.borrow_mut() += 1;
        }
        fn quit(&self) {
            *self.quits.borrow_mut() += 1;
        }
    }

    #[test]
    fn action_ids_map_to_actions_and_unknown_is_rejected() {
        assert_eq!(TrayAction::from_id(id::SHOW), Some(TrayAction::Show));
        assert_eq!(TrayAction::from_id(id::HIDE), Some(TrayAction::Hide));
        assert_eq!(TrayAction::from_id(id::FOLLOW), Some(TrayAction::ToggleFollow));
        assert_eq!(TrayAction::from_id(id::PAUSE), Some(TrayAction::TogglePause));
        assert_eq!(TrayAction::from_id(id::QUIT), Some(TrayAction::Quit));
        assert_eq!(TrayAction::from_id(id::SETTINGS), Some(TrayAction::OpenSettings));
        assert_eq!(TrayAction::from_id("otro"), None);
        let ids = [id::SHOW, id::HIDE, id::FOLLOW, id::PAUSE, id::SETTINGS, id::QUIT];
        assert_eq!(ids.iter().collect::<std::collections::HashSet<_>>().len(), 6);
    }

    #[test]
    fn model_reflects_visibility_follow_and_pause() {
        let m = menu_model(&view(false, false, true));
        assert_eq!(m, MenuModel { show_enabled: false, hide_enabled: true, follow_checked: true, pause_text: "Pausar", settings_enabled: true, show_anyway: false, tooltip: TOOLTIP_DEFAULT });
        let m = menu_model(&view(true, true, false));
        assert_eq!(m, MenuModel { show_enabled: true, hide_enabled: false, follow_checked: false, pause_text: "Reanudar", settings_enabled: true, show_anyway: false, tooltip: TOOLTIP_DEFAULT });
        // pausar no es ocultar y ocultar no quita la pausa: dimensiones independientes
        let m = menu_model(&view(true, false, true));
        assert!(m.hide_enabled && !m.show_enabled && m.pause_text == "Reanudar");
        let m = menu_model(&view(false, true, true));
        assert!(m.show_enabled && !m.hide_enabled && m.pause_text == "Pausar");
    }

    #[test]
    fn first_sync_applies_everything_then_only_changes() {
        let sink = FakeSink::default();
        let sync = MenuSync::new(&sink);
        assert!(sync.sync(&view(false, false, true)));
        assert_eq!(sink.calls.borrow().len(), 5);
        assert!(!sync.sync(&view(false, false, true)), "igual: nada que aplicar");
        assert_eq!(sink.calls.borrow().len(), 5);
        // solo cambia la pausa → una única llamada
        assert!(sync.sync(&view(true, false, true)));
        assert_eq!(sink.calls.borrow().last().unwrap(), "pause=Reanudar");
        assert_eq!(sink.calls.borrow().len(), 6);
        // ocultar → Mostrar/Ocultar cambian, la pausa y el seguimiento no
        assert!(sync.sync(&view(true, true, true)));
        assert_eq!(&sink.calls.borrow()[6..], ["show=true", "hide=false"]);
    }

    #[test]
    fn menu_always_ends_up_matching_the_real_state_for_any_sequence() {
        let sink = FakeSink::default();
        let sync = MenuSync::new(&sink);
        let mut seed = 12345u32;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed >> 16
        };
        for _ in 0..500 {
            let v = view(next() % 2 == 0, next() % 2 == 0, next() % 2 == 0);
            sync.sync(&v);
            assert_eq!(*sink.show.borrow(), v.hidden);
            assert_eq!(*sink.hide.borrow(), !v.hidden);
            assert_eq!(*sink.follow.borrow(), v.follow_enabled);
            assert_eq!(*sink.pause.borrow(), if v.paused { "Reanudar" } else { "Pausar" });
        }
    }

    #[test]
    fn invalidate_forces_a_full_reapply() {
        let sink = FakeSink::default();
        let sync = MenuSync::new(&sink);
        sync.sync(&view(false, false, true));
        sync.invalidate();
        assert!(sync.sync(&view(false, false, true)));
        assert_eq!(sink.calls.borrow().len(), 10);
    }

    #[test]
    fn dispatch_maps_actions_to_idempotent_or_toggle_commands() {
        let ctl = FakeControl::default();
        for a in [TrayAction::Show, TrayAction::Show, TrayAction::Hide, TrayAction::Hide, TrayAction::ToggleFollow, TrayAction::TogglePause] {
            dispatch(a, &ctl);
        }
        assert_eq!(
            *ctl.sent.borrow(),
            [Cmd::Hidden(false), Cmd::Hidden(false), Cmd::Hidden(true), Cmd::Hidden(true), Cmd::ToggleFollow, Cmd::TogglePause]
        );
        assert_eq!(*ctl.quits.borrow(), 0, "solo Cerrar cierra");
        dispatch(TrayAction::Quit, &ctl);
        assert_eq!(*ctl.quits.borrow(), 1);
        assert_eq!(ctl.sent.borrow().len(), 6, "Cerrar no envía órdenes de comportamiento");
    }

    #[test]
    fn settings_item_is_always_available_and_opens_the_settings_window() {
        for (p, h, f) in [(false, false, true), (true, false, true), (false, true, false), (true, true, false)] {
            assert!(menu_model(&view(p, h, f)).settings_enabled, "({p},{h},{f})");
        }
        let mut v = view(false, true, true);
        v.suppressed = true;
        assert!(menu_model(&v).settings_enabled, "tampoco depende de la supresión");
        let sink = FakeSink::default();
        let sync = MenuSync::new(&sink);
        sync.sync(&view(false, false, true));
        assert!(*sink.settings.borrow());
        let ctl = FakeControl::default();
        dispatch(TrayAction::OpenSettings, &ctl);
        dispatch(TrayAction::OpenSettings, &ctl);
        assert_eq!(*ctl.settings_opens.borrow(), 2, "cada clic pide abrir; la unicidad la garantiza la ventana");
        assert!(ctl.sent.borrow().is_empty(), "abrir ajustes no envía órdenes al motor");
        assert_eq!(*ctl.quits.borrow(), 0);
    }

    fn suppressed_view(hidden: bool) -> View {
        View { suppressed: true, ..view(false, hidden, true) }
    }

    #[test]
    fn show_anyway_exists_only_while_suppressed_and_not_hidden_by_the_user() {
        assert!(!menu_model(&view(false, false, true)).show_anyway, "sin supresión: nunca");
        assert!(!menu_model(&view(false, true, true)).show_anyway, "oculta sin supresión: nunca");
        let m = menu_model(&suppressed_view(false));
        assert!(m.show_anyway, "suprimida y no oculta");
        // Mostrar/Ocultar siguen reflejando solo `hidden`
        assert_eq!((m.show_enabled, m.hide_enabled), (false, true));
        // suprimida y oculta por la persona: «Mostrar» ya hace lo mismo (quita hidden y anula el episodio)
        let m = menu_model(&suppressed_view(true));
        assert!(!m.show_anyway);
        assert_eq!((m.show_enabled, m.hide_enabled), (true, false));
        // la pausa no influye
        assert!(menu_model(&View { paused: true, ..suppressed_view(false) }).show_anyway);
    }

    #[test]
    fn the_tooltip_explains_the_suppression_while_it_lasts() {
        assert_eq!(menu_model(&view(false, false, true)).tooltip, "Miel");
        assert_eq!(menu_model(&suppressed_view(false)).tooltip, "Miel está oculta por No molestar");
        assert_eq!(menu_model(&suppressed_view(true)).tooltip, TOOLTIP_SUPPRESSED, "también si además la ocultó la persona");
    }

    #[test]
    fn sync_adds_and_removes_the_item_and_the_tooltip_only_on_changes() {
        let sink = FakeSink::default();
        let sync = MenuSync::new(&sink);
        sync.sync(&view(false, false, true));
        assert!(
            !sink.calls.borrow().iter().any(|c| c.starts_with("show_anyway") || c.starts_with("tooltip")),
            "reposo: nada que aplicar"
        );
        assert!(sync.sync(&suppressed_view(false)));
        assert_eq!(&sink.calls.borrow()[5..], ["show_anyway=true", &format!("tooltip={TOOLTIP_SUPPRESSED}")]);
        assert!(!sync.sync(&suppressed_view(false)), "igual: nada");
        let n = sink.calls.borrow().len();
        // la persona oculta durante el episodio: el elemento sale, el tooltip se queda
        assert!(sync.sync(&suppressed_view(true)));
        assert_eq!(&sink.calls.borrow()[n..], ["show=true", "hide=false", "show_anyway=false"]);
        // termina el episodio
        assert!(sync.sync(&view(false, false, true)));
        assert_eq!(sink.calls.borrow().last().unwrap(), &format!("tooltip={TOOLTIP_DEFAULT}"));
        assert!(!*sink.show_anyway.borrow());
    }

    #[test]
    fn a_first_sync_that_starts_suppressed_applies_the_item_and_the_tooltip() {
        let sink = FakeSink::default();
        let sync = MenuSync::new(&sink);
        sync.sync(&suppressed_view(false));
        assert!(*sink.show_anyway.borrow() && *sink.tooltip.borrow() == TOOLTIP_SUPPRESSED);
    }

    #[test]
    fn show_anyway_maps_to_the_override_command_and_nothing_else() {
        assert_eq!(TrayAction::from_id(id::SHOW_ANYWAY), Some(TrayAction::ShowAnyway));
        let ids = [id::SHOW, id::HIDE, id::FOLLOW, id::PAUSE, id::SETTINGS, id::SHOW_ANYWAY, id::QUIT];
        assert_eq!(ids.iter().collect::<std::collections::HashSet<_>>().len(), 7, "ids únicos");
        let ctl = FakeControl::default();
        dispatch(TrayAction::ShowAnyway, &ctl);
        assert_eq!(*ctl.sent.borrow(), [Cmd::DndOverride]);
        assert_eq!((*ctl.quits.borrow(), *ctl.settings_opens.borrow()), (0, 0));
    }
}
