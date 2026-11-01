//! Configuración nativa y posicionamiento inicial de la ventana de Miel.
//! Todo ocurre en Rust: el frontend no necesita permisos de ventana.

use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::mascot::geometry::Rect;

/// Margen respecto al borde del área de trabajo, en píxeles lógicos.
const MARGIN_LOGICAL: f64 = 24.0;

/// Ancho de la ventana (px lógicos): cabe la burbuja (220 px) con margen, para cualquier tamaño de sprite.
pub const WIDTH: f64 = 260.0;
/// Espacio sobre el sprite reservado para la burbuja (px lógicos).
pub const BUBBLE_SPACE: f64 = 80.0;
/// Espacio bajo el sprite (patas, flotación y holgura) (px lógicos).
pub const BOTTOM_SPACE: f64 = 24.0;

/// Geometría lógica de la ventana para un tamaño de sprite. Debe coincidir con
/// `src/components/MascotStage/layout.ts` (hay una prueba en cada lado con los mismos números).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub width: f64,
    pub height: f64,
    /// Distancia del centro del sprite al borde superior de la ventana.
    pub anchor_y: f64,
}

pub fn metrics(size: u32) -> Metrics {
    let s = size as f64;
    Metrics { width: WIDTH, height: BUBBLE_SPACE + s + BOTTOM_SPACE, anchor_y: BUBBLE_SPACE + s / 2.0 }
}

/// Posición del ancla como fracción de la ventana (x, y), para la geometría del motor.
pub fn anchor_frac(size: u32) -> (f64, f64) {
    let m = metrics(size);
    (0.5, m.anchor_y / m.height)
}

/// Esquina inferior derecha del área de trabajo, con margen, garantizando que la
/// ventana quede siempre dentro del área aunque ésta sea más pequeña que ella.
pub fn initial_position(work: Rect, win: PhysicalSize<u32>, margin_px: i32) -> (i32, i32) {
    let max_x = work.x + work.w as i32 - win.width as i32;
    let max_y = work.y + work.h as i32 - win.height as i32;
    let x = (max_x - margin_px).max(work.x).min(max_x.max(work.x));
    let y = (max_y - margin_px).max(work.y).min(max_y.max(work.y));
    (x, y)
}

/// Propiedades nativas que no se pueden (o no conviene) expresar en tauri.conf.json.
/// Cada operación devuelve error explícito; el llamador decide cómo registrarlo.
pub fn configure(win: &WebviewWindow, pet_size: u32) -> tauri::Result<()> {
    // Tamaño de la ventana según el tamaño del sprite (la ventana nace oculta: sin parpadeo).
    let m = metrics(pet_size);
    win.set_size(LogicalSize::new(m.width, m.height))?;
    // Click-through: los eventos del ratón atraviesan la ventana.
    win.set_ignore_cursor_events(true)?;
    // Refuerzo de lo declarado en la configuración (no robar foco).
    win.set_focusable(false)?;
    Ok(())
}

/// Coloca la ventana en el área de trabajo del monitor principal (excluye la barra de tareas).
pub fn place_initial(win: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match win.primary_monitor()? {
        Some(m) => m,
        None => match win.current_monitor()? {
            Some(m) => m,
            None => return Ok(()), // sin monitor: se deja la posición del sistema
        },
    };
    let wa = monitor.work_area();
    let work = Rect::new(wa.position.x, wa.position.y, wa.size.width, wa.size.height);
    let size = win.outer_size()?;
    let margin = (MARGIN_LOGICAL * monitor.scale_factor()).round() as i32;
    let (x, y) = initial_position(work, size, margin);
    win.set_position(PhysicalPosition::new(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: PhysicalSize<u32> = PhysicalSize { width: 240, height: 220 };

    #[test]
    fn metrics_for_the_reference_size_match_the_original_window() {
        assert_eq!(metrics(96), Metrics { width: 260.0, height: 200.0, anchor_y: 128.0 });
        assert_eq!(anchor_frac(96), (0.5, 0.64));
    }

    #[test]
    fn metrics_leave_room_for_bubble_above_and_legs_below_for_every_size() {
        for size in [64u32, 96, 128] {
            let m = metrics(size);
            let half = size as f64 / 2.0;
            assert_eq!(m.anchor_y - half, BUBBLE_SPACE, "espacio para la burbuja sobre el sprite ({size})");
            assert_eq!(m.height - (m.anchor_y + half), BOTTOM_SPACE, "holgura inferior ({size})");
            assert!(m.width >= size as f64 + 2.0 * 40.0, "alas y antenas caben en ancho ({size})");
            assert!(m.width >= 220.0 + 2.0 * 20.0, "la burbuja (220 px) cabe con margen ({size})");
            let (fx, fy) = anchor_frac(size);
            assert_eq!(fx, 0.5);
            assert!((fy * m.height - m.anchor_y).abs() < 1e-9);
        }
    }

    #[test]
    fn bigger_sprite_gets_a_taller_window_with_the_same_bubble_space() {
        assert!(metrics(128).height > metrics(96).height && metrics(96).height > metrics(64).height);
        assert_eq!(metrics(64).height, 168.0);
        assert_eq!(metrics(128).height, 232.0);
    }

    #[test]
    fn bottom_right_with_margin() {
        let work = Rect::new(0, 0, 1920, 1040);
        assert_eq!(initial_position(work, WIN, 24), (1920 - 240 - 24, 1040 - 220 - 24));
    }

    #[test]
    fn negative_origin_monitor() {
        let work = Rect::new(-1920, -200, 1920, 1040);
        assert_eq!(initial_position(work, WIN, 24), (-264, 596));
    }

    #[test]
    fn tiny_work_area_stays_anchored_top_left() {
        let work = Rect::new(100, 50, 100, 100);
        assert_eq!(initial_position(work, WIN, 24), (100, 50));
    }

    #[test]
    fn scaled_dpi_window_fits() {
        // 150 %: ventana 360×330 físicos, margen 36
        let work = Rect::new(0, 0, 2560, 1400);
        let win = PhysicalSize { width: 360, height: 330 };
        assert_eq!(initial_position(work, win, 36), (2560 - 360 - 36, 1400 - 330 - 36));
    }
}
