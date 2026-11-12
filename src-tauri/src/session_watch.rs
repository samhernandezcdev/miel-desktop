//! Avisos NATIVOS de sesión y energía (E5): bloqueo/desbloqueo de la sesión y suspensión/reanudación del equipo.
//!
//! Opción más pequeña (la misma técnica que `display_watch`): una SEGUNDA subclase de la ventana principal
//! (`SetWindowSubclass`, id propio) que escucha dos mensajes que Windows ya entrega a las ventanas de nivel superior:
//!
//! - `WM_POWERBROADCAST` con `PBT_APMSUSPEND` (se suspende) o `PBT_APMRESUMEAUTOMATIC`/`PBT_APMRESUMESUSPEND` (se reanuda).
//! - `WM_WTSSESSION_CHANGE` con `WTS_SESSION_LOCK`/`WTS_SESSION_UNLOCK`, que solo llega a las ventanas registradas con
//!   `WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)` (se anula al retirar la subclase).
//!
//! No hay ventana nueva, hilo nuevo, *polling* ni gancho global; el callback solo avisa al motor
//! (`Cmd::Session`) y el trabajo lo hace su hilo. Windows no entrega ningún dato más que «se bloqueó»,
//! «se desbloqueó», «se suspende» o «se reanuda»: nada de teclas, ventanas ni contenido. Ver `docs/PRIVACY.md`.
//!
//! Ciclo de vida igual que `DisplayWatch`: `install` devuelve un `SessionWatch`; `remove()` retira la subclase y
//! anula el registro (idempotente); `WM_NCDESTROY` hace lo mismo si la ventana se destruye antes.

use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, WM_NCDESTROY, WM_POWERBROADCAST,
    WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};

use crate::mascot::engine::SessionEvent;

type Callback = Box<dyn Fn(SessionEvent) + Send + Sync>;

/// Identificador de la subclase (distinto del de `display_watch`): "SESS".
const SUBCLASS_ID: usize = 0x5345_5353;

/// Traduce un mensaje de Windows a un evento de sesión/energía, o `None` si no interesa. Pura y probada.
pub fn classify(msg: u32, wparam: usize) -> Option<SessionEvent> {
    match msg {
        WM_WTSSESSION_CHANGE => match wparam as u32 {
            WTS_SESSION_LOCK => Some(SessionEvent::Locked),
            WTS_SESSION_UNLOCK => Some(SessionEvent::Unlocked),
            _ => None,
        },
        WM_POWERBROADCAST => match wparam as u32 {
            PBT_APMSUSPEND => Some(SessionEvent::Suspended),
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => Some(SessionEvent::Resumed),
            _ => None,
        },
        _ => None,
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: `data` es el puntero a `Callback` creado en `install`; solo se libera tras quitar la subclase con
    // éxito, y mientras esté instalada sigue siendo válido.
    let cb = unsafe { &*(data as *const Callback) };
    if let Some(ev) = classify(msg, wparam.0) {
        cb(ev);
    }
    if msg == WM_NCDESTROY {
        // SAFETY: igual que en `display_watch`: solo quien logra quitar la subclase libera el callback.
        unsafe {
            let removed = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID).as_bool();
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            if removed {
                let _ = WTSUnRegisterSessionNotification(hwnd);
                drop(Box::from_raw(data as *mut Callback));
            }
            return r;
        }
    }
    // SAFETY: llamada estándar al procedimiento siguiente de la cadena de subclases.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Asa de la subclase instalada. `remove` debe llamarse desde el hilo de la ventana (el principal).
pub struct SessionWatch {
    hwnd: HWND,
    data: Mutex<Option<usize>>,
}

// SAFETY: el asa solo guarda un HWND y un puntero opaco; `remove` se invoca únicamente desde el hilo principal.
unsafe impl Send for SessionWatch {}
unsafe impl Sync for SessionWatch {}

impl SessionWatch {
    /// Retira la subclase, anula el registro de avisos de sesión y libera el callback. `true` si esta llamada lo hizo.
    pub fn remove(&self) -> bool {
        let Some(ptr) = self.data.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return false;
        };
        // SAFETY: si la subclase sigue instalada, `ptr` sigue siendo el `Box<Callback>` válido.
        unsafe {
            if RemoveWindowSubclass(self.hwnd, Some(subclass_proc), SUBCLASS_ID).as_bool() {
                let _ = WTSUnRegisterSessionNotification(self.hwnd);
                drop(Box::from_raw(ptr as *mut Callback));
                true
            } else {
                false // WM_NCDESTROY ya la quitó y liberó el callback
            }
        }
    }
}

/// Instala el aviso. Debe llamarse desde el hilo que creó la ventana (el de `setup`).
pub fn install(hwnd: HWND, on_event: impl Fn(SessionEvent) + Send + Sync + 'static) -> Result<SessionWatch, String> {
    let cb: Box<Callback> = Box::new(Box::new(on_event));
    let raw = Box::into_raw(cb) as usize;
    // SAFETY: `hwnd` es una ventana válida del hilo actual; `raw` se libera al retirar la subclase.
    let ok = unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, raw) };
    if !ok.as_bool() {
        // SAFETY: la subclase no se instaló; se recupera el Box para no filtrar memoria.
        unsafe { drop(Box::from_raw(raw as *mut Callback)) };
        return Err("SetWindowSubclass falló".into());
    }
    // SAFETY: `hwnd` válida; solo pide a Windows los avisos de ESTA sesión.
    if let Err(e) = unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) } {
        // sin aviso de bloqueo, pero la suspensión/reanudación (WM_POWERBROADCAST) sigue funcionando
        eprintln!("[miel] sin aviso de bloqueo de sesión: {e}");
    }
    Ok(SessionWatch { hwnd, data: Mutex::new(Some(raw)) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SendMessageW, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
    };

    fn message_window() -> HWND {
        // SAFETY: ventana de solo mensajes (sin interfaz); se destruye en cada prueba.
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("miel-session-test"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
            .expect("ventana de mensajes")
        }
    }

    fn send(hwnd: HWND, msg: u32, w: usize) {
        // SAFETY: mensaje síncrono a una ventana propia del hilo actual.
        unsafe {
            SendMessageW(hwnd, msg, Some(WPARAM(w)), Some(LPARAM(0)));
        }
    }

    #[test]
    fn classifies_only_the_four_session_and_power_events() {
        assert_eq!(classify(WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize), Some(SessionEvent::Locked));
        assert_eq!(classify(WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK as usize), Some(SessionEvent::Unlocked));
        assert_eq!(classify(WM_POWERBROADCAST, PBT_APMSUSPEND as usize), Some(SessionEvent::Suspended));
        assert_eq!(classify(WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize), Some(SessionEvent::Resumed));
        assert_eq!(classify(WM_POWERBROADCAST, PBT_APMRESUMESUSPEND as usize), Some(SessionEvent::Resumed));
        // el resto de avisos de sesión (inicio/fin de sesión remota, conexión...) y de energía (batería, etc.) se ignoran
        for w in [1usize, 2, 3, 4, 5, 6, 9, 10] {
            assert_eq!(classify(WM_WTSSESSION_CHANGE, w), None, "WTS {w}");
        }
        for w in [0usize, 9, 10, 11, 32, 0x8000] {
            assert_eq!(classify(WM_POWERBROADCAST, w), None, "PBT {w}");
        }
        assert_eq!(classify(0x0001, WTS_SESSION_LOCK as usize), None);
    }

    #[test]
    fn the_subclass_forwards_events_and_is_removed_cleanly() {
        let hwnd = message_window();
        let hits = Arc::new(AtomicUsize::new(0));
        let last = Arc::new(Mutex::new(Vec::new()));
        let (h, l) = (Arc::clone(&hits), Arc::clone(&last));
        let watch = install(hwnd, move |ev| {
            h.fetch_add(1, Ordering::SeqCst);
            l.lock().unwrap().push(ev);
        })
        .expect("instala");
        send(hwnd, WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize);
        send(hwnd, WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK as usize);
        send(hwnd, WM_POWERBROADCAST, PBT_APMSUSPEND as usize);
        send(hwnd, WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize);
        send(hwnd, WM_POWERBROADCAST, 10); // PBT_APMPOWERSTATUSCHANGE: ignorado
        send(hwnd, 0x0001, 0);
        assert_eq!(
            *last.lock().unwrap(),
            [SessionEvent::Locked, SessionEvent::Unlocked, SessionEvent::Suspended, SessionEvent::Resumed]
        );
        assert!(watch.remove(), "la primera retirada quita la subclase");
        send(hwnd, WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize);
        assert_eq!(hits.load(Ordering::SeqCst), 4, "tras remove() no se llama al callback");
        assert!(!watch.remove(), "idempotente");
        // SAFETY: la ventana es nuestra.
        unsafe { DestroyWindow(hwnd).expect("destruye") };
        assert_eq!(Arc::strong_count(&hits), 1, "el callback se liberó exactamente una vez");
    }

    #[test]
    fn window_destruction_frees_the_hook_without_remove() {
        let hwnd = message_window();
        let marker = Arc::new(());
        let m = Arc::clone(&marker);
        let watch = install(hwnd, move |_| {
            let _ = &m;
        })
        .unwrap();
        assert_eq!(Arc::strong_count(&marker), 2);
        // SAFETY: la ventana es nuestra; WM_NCDESTROY quita la subclase y libera el callback.
        unsafe { DestroyWindow(hwnd).unwrap() };
        assert!(!watch.remove());
        assert_eq!(Arc::strong_count(&marker), 1);
    }
}
