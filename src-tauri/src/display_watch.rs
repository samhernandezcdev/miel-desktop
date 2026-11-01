//! Avisos NATIVOS de cambios de pantalla en Windows.
//!
//! Tauri no expone un evento público para cambios de resolución, disposición de monitores o área de
//! trabajo (`ScaleFactorChanged` solo cubre el DPI de la ventana). Por eso se subclasifica la
//! ventana principal (`SetWindowSubclass`, comctl32) y se escuchan tres mensajes de Windows:
//!
//! - `WM_DISPLAYCHANGE`: cambió la resolución o se conectó/desconectó un monitor.
//! - `WM_DPICHANGED`: cambió la escala DPI.
//! - `WM_SETTINGCHANGE` con `SPI_SETWORKAREA`: cambió el área de trabajo (barra de tareas movida,
//!   redimensionada o ocultada automáticamente).
//!
//! Los mensajes llegan a la ventana aunque esté oculta y no cuestan nada en reposo: no hay
//! polling. El callback solo avisa al motor (`Cmd::GeometryChanged`); el trabajo lo hace su hilo.
//! Sin datos personales: no se lee nada de otras ventanas.
//!
//! Ciclo de vida: `install` devuelve un `DisplayWatch`; `remove()` retira la subclase y libera el
//! callback (idempotente). Si la ventana se destruye antes, `WM_NCDESTROY` hace lo mismo. Solo uno
//! de los dos caminos libera el callback: el que logra quitar la subclase.

use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_SETWORKAREA, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_NCDESTROY, WM_SETTINGCHANGE,
};

type Callback = Box<dyn Fn() + Send + Sync>;

/// Identificador de la subclase (arbitrario pero fijo: "MIEL").
const SUBCLASS_ID: usize = 0x4D49_454C;

/// ¿Este mensaje de Windows indica un cambio de geometría de pantalla? (puro → testeable)
pub fn is_geometry_message(msg: u32, wparam: usize) -> bool {
    match msg {
        WM_DISPLAYCHANGE | WM_DPICHANGED => true,
        WM_SETTINGCHANGE => wparam as u32 == SPI_SETWORKAREA.0,
        _ => false,
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
    // SAFETY: `data` es el puntero a `Callback` creado en `install`; solo se libera tras quitar
    // la subclase con éxito, y mientras la subclase esté instalada sigue siendo válido.
    let cb = unsafe { &*(data as *const Callback) };
    if is_geometry_message(msg, wparam.0) {
        cb();
    }
    if msg == WM_NCDESTROY {
        // SAFETY: si `remove()` no se llamó antes, aquí se quita la subclase y se libera el
        // callback una única vez; si ya se quitó, RemoveWindowSubclass devuelve FALSE y no se toca.
        unsafe {
            let removed = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID).as_bool();
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            if removed {
                drop(Box::from_raw(data as *mut Callback));
            }
            return r;
        }
    }
    // SAFETY: llamada estándar al procedimiento siguiente de la cadena de subclases.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Asa de la subclase instalada. `remove` debe llamarse desde el hilo de la ventana (el principal).
pub struct DisplayWatch {
    hwnd: HWND,
    data: Mutex<Option<usize>>,
}

// SAFETY: el asa solo guarda un HWND y un puntero opaco; las operaciones sobre la ventana
// (`remove`) se invocan únicamente desde el hilo principal de la aplicación.
unsafe impl Send for DisplayWatch {}
unsafe impl Sync for DisplayWatch {}

impl DisplayWatch {
    /// Retira la subclase y libera el callback. Devuelve `true` si esta llamada lo hizo,
    /// `false` si ya estaba retirada (segunda llamada, o la ventana ya se destruyó).
    pub fn remove(&self) -> bool {
        let Some(ptr) = self.data.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return false;
        };
        // SAFETY: si la subclase sigue instalada, `ptr` sigue siendo el `Box<Callback>` válido.
        unsafe {
            if RemoveWindowSubclass(self.hwnd, Some(subclass_proc), SUBCLASS_ID).as_bool() {
                drop(Box::from_raw(ptr as *mut Callback));
                true
            } else {
                false // WM_NCDESTROY ya la quitó y liberó el callback
            }
        }
    }
}

/// Instala el aviso. Debe llamarse desde el hilo que creó la ventana (el de `setup`).
pub fn install(
    hwnd: HWND,
    on_change: impl Fn() + Send + Sync + 'static,
) -> Result<DisplayWatch, String> {
    let cb: Box<Callback> = Box::new(Box::new(on_change));
    let raw = Box::into_raw(cb) as usize;
    // SAFETY: `hwnd` es una ventana válida del hilo actual; `raw` se libera al retirar la subclase.
    let ok = unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, raw) };
    if ok.as_bool() {
        Ok(DisplayWatch { hwnd, data: Mutex::new(Some(raw)) })
    } else {
        // SAFETY: la subclase no se instaló; se recupera el Box para no filtrar memoria.
        unsafe { drop(Box::from_raw(raw as *mut Callback)) };
        Err("SetWindowSubclass falló".into())
    }
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

    /// Ventana de solo mensajes (sin interfaz gráfica): permite probar la subclase de verdad.
    fn message_window() -> HWND {
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("miel-test"),
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
        unsafe {
            SendMessageW(hwnd, msg, Some(WPARAM(w)), Some(LPARAM(0)));
        }
    }

    #[test]
    fn recognizes_geometry_messages_only() {
        assert!(is_geometry_message(WM_DISPLAYCHANGE, 0));
        assert!(is_geometry_message(WM_DPICHANGED, 0));
        assert!(is_geometry_message(WM_SETTINGCHANGE, SPI_SETWORKAREA.0 as usize));
        assert!(!is_geometry_message(WM_SETTINGCHANGE, 0x2A)); // otro ajuste del sistema
        assert!(!is_geometry_message(0x0001, 0)); // WM_CREATE
    }

    #[test]
    fn hook_fires_only_for_geometry_messages_and_is_removed_cleanly() {
        let hwnd = message_window();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = Arc::clone(&hits);
        let watch = install(hwnd, move || {
            h.fetch_add(1, Ordering::SeqCst);
        })
        .expect("instala");

        send(hwnd, WM_DISPLAYCHANGE, 32);
        send(hwnd, WM_DPICHANGED, 0);
        send(hwnd, WM_SETTINGCHANGE, SPI_SETWORKAREA.0 as usize);
        send(hwnd, WM_SETTINGCHANGE, 0x2A); // ajuste ajeno: ignorado
        send(hwnd, 0x0001, 0);
        assert_eq!(hits.load(Ordering::SeqCst), 3);

        // retirada explícita: el procedimiento deja de estar registrado
        assert!(watch.remove(), "la primera retirada quita la subclase");
        send(hwnd, WM_DISPLAYCHANGE, 32);
        assert_eq!(hits.load(Ordering::SeqCst), 3, "tras remove() no se llama al callback");
        assert!(!watch.remove(), "idempotente: la segunda retirada no hace nada");

        unsafe {
            DestroyWindow(hwnd).expect("destruye");
        }
        assert!(!watch.remove(), "tras destruir la ventana tampoco hay doble liberación");
    }

    #[test]
    fn window_destruction_frees_the_hook_without_remove() {
        let hwnd = message_window();
        let hits = Arc::new(AtomicUsize::new(0));
        let h = Arc::clone(&hits);
        let watch = install(hwnd, move || {
            h.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        send(hwnd, WM_DISPLAYCHANGE, 0);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        // WM_NCDESTROY quita la subclase y libera el callback; remove() posterior es inofensivo
        unsafe {
            DestroyWindow(hwnd).unwrap();
        }
        assert!(!watch.remove());
        // el callback (y su Arc) se liberó exactamente una vez: solo queda nuestra referencia
        assert_eq!(Arc::strong_count(&hits), 1);
    }

    #[test]
    fn remove_frees_the_callback_exactly_once() {
        let hwnd = message_window();
        let marker = Arc::new(());
        let m = Arc::clone(&marker);
        let watch = install(hwnd, move || {
            let _ = &m;
        })
        .unwrap();
        assert_eq!(Arc::strong_count(&marker), 2);
        assert!(watch.remove());
        assert_eq!(Arc::strong_count(&marker), 1, "el callback se liberó al retirar la subclase");
        unsafe {
            DestroyWindow(hwnd).unwrap();
        }
        assert_eq!(Arc::strong_count(&marker), 1);
    }
}
