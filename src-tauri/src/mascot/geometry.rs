//! Geometría pura en píxeles FÍSICOS. Sin dependencias de Tauri → totalmente testeable.
//!
//! Convenciones:
//! - `Rect` es el área de trabajo de un monitor (excluye barra de tareas); `x`/`y` pueden ser negativos.
//! - El "ancla" (`Vec2`) es el centro del SPRITE. Dentro de la ventana está en `(ax, ay)` desde su
//!   esquina superior izquierda (la burbuja de diálogo ocupa el espacio sobre la abeja, así que el
//!   sprite no está centrado verticalmente). Esquina de la ventana = `ancla - (ax, ay)`.
//! - Las restricciones usan la geometría COMPLETA de la ventana (no solo el sprite).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f64 {
        self.x as f64 + self.w as f64
    }
    pub fn bottom(&self) -> f64 {
        self.y as f64 + self.h as f64
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x as f64 && p.x < self.right() && p.y >= self.y as f64 && p.y < self.bottom()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn minus(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
    pub fn plus(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
    pub fn scale(self, k: f64) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn dist(self, o: Vec2) -> f64 {
        self.minus(o).len()
    }
}

/// Geometría física de la ventana: tamaño y posición del ancla (centro del sprite) dentro de ella.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub w: f64,
    pub h: f64,
    pub ax: f64,
    pub ay: f64,
}

impl Frame {
    /// Ancla en el centro de la ventana.
    #[cfg(test)]
    pub const fn centered(w: f64, h: f64) -> Self {
        Self { w, h, ax: w / 2.0, ay: h / 2.0 }
    }
    /// Ancla en una fracción (0..1) del ancho y alto de la ventana.
    pub fn with_anchor_frac(w: f64, h: f64, fx: f64, fy: f64) -> Self {
        Self { w, h, ax: w * fx, ay: h * fy }
    }
}

/// Rango válido del ancla para que la ventana quede completa dentro de `work`.
/// Si el área es más pequeña que la ventana, se colapsa al centro del área.
fn anchor_range(work: Rect, win: Frame) -> (f64, f64, f64, f64) {
    let (min_x, max_x) = axis(work.x as f64, work.w as f64, win.w, win.ax);
    let (min_y, max_y) = axis(work.y as f64, work.h as f64, win.h, win.ay);
    (min_x, min_y, max_x, max_y)
}

/// Rango del ancla en un eje: la ventana (`w`, con el ancla a `a` de su inicio) cabe en `[start, start+len]`.
fn axis(start: f64, len: f64, w: f64, a: f64) -> (f64, f64) {
    if len <= w {
        let c = start + len / 2.0 - w / 2.0 + a;
        (c, c)
    } else {
        (start + a, start + len - (w - a))
    }
}

fn dist_to_rect(work: Rect, p: Vec2) -> f64 {
    let dx = (work.x as f64 - p.x).max(0.0).max(p.x - work.right());
    let dy = (work.y as f64 - p.y).max(0.0).max(p.y - work.bottom());
    dx.hypot(dy)
}

/// Área más apropiada para un punto: la que lo contiene o, si no hay, la más cercana.
pub fn nearest_area(areas: &[Rect], p: Vec2) -> Option<Rect> {
    areas
        .iter()
        .copied()
        .min_by(|a, b| dist_to_rect(*a, p).total_cmp(&dist_to_rect(*b, p)))
}

/// Restringe el ancla al área más cercana de modo que la ventana completa quede dentro.
pub fn clamp_anchor(areas: &[Rect], p: Vec2, win: Frame) -> Vec2 {
    let Some(area) = nearest_area(areas, p) else {
        return p; // sin información de monitores: no se toca
    };
    let (min_x, min_y, max_x, max_y) = anchor_range(area, win);
    Vec2::new(p.x.clamp(min_x, max_x), p.y.clamp(min_y, max_y))
}

/// ¿La ventana completa cabe dentro de algún área?
pub fn fully_visible(areas: &[Rect], anchor: Vec2, win: Frame) -> bool {
    areas.iter().any(|a| {
        let (min_x, min_y, max_x, max_y) = anchor_range(*a, win);
        anchor.x >= min_x && anchor.x <= max_x && anchor.y >= min_y && anchor.y <= max_y
    })
}

/// Durante un desplazamiento se tolera cruzar el borde entre monitores: basta con que el
/// centro de la ventana esté en algún área; si no, se vuelve al área más cercana.
pub fn ensure_center_visible(areas: &[Rect], anchor: Vec2, win: Frame) -> Vec2 {
    if areas.iter().any(|a| a.contains(anchor)) {
        anchor
    } else {
        clamp_anchor(areas, anchor, win)
    }
}

/// Posición de descanso: esquina inferior derecha del área más cercana a `from`.
pub fn rest_spot(areas: &[Rect], from: Vec2, win: Frame) -> Vec2 {
    match nearest_area(areas, from) {
        Some(a) => {
            let (_, _, max_x, max_y) = anchor_range(a, win);
            Vec2::new(max_x, max_y)
        }
        None => from,
    }
}

/// Esquina superior izquierda de la ventana (píxeles enteros) para un ancla.
pub fn window_origin(anchor: Vec2, win: Frame) -> (i32, i32) {
    ((anchor.x - win.ax).round() as i32, (anchor.y - win.ay).round() as i32)
}

/// Ancla (centro del sprite) a partir de la esquina física de la ventana.
pub fn anchor_of(origin: (i32, i32), win: Frame) -> Vec2 {
    Vec2::new(origin.0 as f64 + win.ax, origin.1 as f64 + win.ay)
}

/// Destino de seguimiento. Mantiene a la abeja a `dist` del cursor, del lado desde el que
/// viene, sin taparlo. Si ya está en la banda cómoda no se mueve (detención temprana).
/// Cerca de un borde, si el punto ideal queda demasiado cerca del cursor tras restringirlo,
/// elige entre 8 direcciones la válida más cercana a la posición actual.
pub fn follow_target(cursor: Vec2, current: Vec2, dist: f64, areas: &[Rect], win: Frame) -> Vec2 {
    let d0 = current.dist(cursor);
    if d0 >= dist * 0.85 && d0 <= dist * 1.35 {
        return current;
    }
    let dir = if d0 > 1e-6 {
        current.minus(cursor).scale(1.0 / d0)
    } else {
        Vec2::new(-std::f64::consts::FRAC_1_SQRT_2, -std::f64::consts::FRAC_1_SQRT_2)
    };
    let ideal = clamp_anchor(areas, cursor.plus(dir.scale(dist)), win);
    let min_ok = dist * 0.7;
    if ideal.dist(cursor) >= min_ok {
        return ideal;
    }
    let mut best: Option<(f64, Vec2)> = None;
    let mut farthest: (f64, Vec2) = (ideal.dist(cursor), ideal);
    for k in 0..8 {
        let a = k as f64 * std::f64::consts::FRAC_PI_4;
        let cand = clamp_anchor(areas, cursor.plus(Vec2::new(a.cos(), a.sin()).scale(dist)), win);
        let dc = cand.dist(cursor);
        if dc > farthest.0 {
            farthest = (dc, cand);
        }
        if dc >= min_ok {
            let travel = cand.dist(current);
            if best.is_none_or(|(t, _)| travel < t) {
                best = Some((travel, cand));
            }
        }
    }
    best.map(|(_, c)| c).unwrap_or(farthest.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: Frame = Frame::centered(240.0, 220.0);
    fn hd() -> Rect {
        Rect::new(0, 0, 1920, 1032)
    }

    #[test]
    fn clamp_keeps_whole_window_inside() {
        let a = [hd()];
        let c = clamp_anchor(&a, Vec2::new(-500.0, 5000.0), WIN);
        assert_eq!(c, Vec2::new(120.0, 1032.0 - 110.0));
        let (ox, oy) = window_origin(c, WIN);
        assert_eq!((ox, oy), (0, 1032 - 220));
    }

    #[test]
    fn negative_coordinates_monitor() {
        let left = Rect::new(-1920, -100, 1920, 1040);
        let areas = [left, hd()];
        let c = clamp_anchor(&areas, Vec2::new(-5000.0, 0.0), WIN);
        assert_eq!(c.x, -1920.0 + 120.0);
        assert_eq!(c.y, -100.0 + 110.0);
        assert!(fully_visible(&areas, c, WIN));
    }

    #[test]
    fn picks_monitor_that_contains_point() {
        let left = Rect::new(-1920, 0, 1920, 1080);
        let areas = [left, hd()];
        let c = clamp_anchor(&areas, Vec2::new(-960.0, 500.0), WIN);
        assert_eq!(c, Vec2::new(-960.0, 500.0));
    }

    #[test]
    fn disconnected_monitor_snaps_back() {
        // la abeja estaba en el monitor derecho (x≈2500) que ya no existe
        let areas = [hd()];
        let a = Vec2::new(2500.0, 500.0);
        assert!(!fully_visible(&areas, a, WIN));
        let fixed = ensure_center_visible(&areas, a, WIN);
        assert!(fully_visible(&areas, fixed, WIN));
        assert_eq!(fixed.x, 1920.0 - 120.0);
    }

    #[test]
    fn straddling_between_monitors_is_tolerated_while_moving() {
        let areas = [Rect::new(-1920, 0, 1920, 1080), hd()];
        let a = Vec2::new(-30.0, 500.0); // centro en monitor izquierdo, ventana cruza el borde
        assert_eq!(ensure_center_visible(&areas, a, WIN), a);
    }

    #[test]
    fn window_smaller_area_collapses_to_center() {
        let tiny = [Rect::new(0, 0, 100, 100)];
        assert_eq!(clamp_anchor(&tiny, Vec2::new(900.0, 900.0), WIN), Vec2::new(50.0, 50.0));
    }

    #[test]
    fn rest_spot_is_bottom_right_corner_of_current_monitor() {
        let areas = [Rect::new(-1920, 0, 1920, 1080), hd()];
        let s = rest_spot(&areas, Vec2::new(-500.0, 300.0), WIN);
        assert_eq!(s, Vec2::new(0.0 - 120.0, 1080.0 - 110.0));
    }

    #[test]
    fn dpi_scaled_window_sizes() {
        // 100/125/150/200 %: ventana lógica 240×220 → física; el clamp respeta cada tamaño
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let win = Frame::centered(240.0 * scale, 220.0 * scale);
            let area = Rect::new(0, 0, 3840, 2100);
            let c = clamp_anchor(&[area], Vec2::new(1e6, 1e6), win);
            let (ox, oy) = window_origin(c, win);
            assert_eq!(ox as f64 + win.w, 3840.0, "x a {scale}");
            assert_eq!(oy as f64 + win.h, 2100.0, "y a {scale}");
        }
    }

    #[test]
    fn follow_stays_put_in_comfort_band() {
        let areas = [hd()];
        let cur = Vec2::new(800.0, 500.0);
        let cursor = Vec2::new(800.0 - 130.0, 500.0);
        assert_eq!(follow_target(cursor, cur, 130.0, &areas, WIN), cur);
    }

    #[test]
    fn follow_target_keeps_distance_and_never_on_cursor() {
        let areas = [hd()];
        let cursor = Vec2::new(900.0, 500.0);
        let t = follow_target(cursor, Vec2::new(300.0, 500.0), 130.0, &areas, WIN);
        assert!((t.dist(cursor) - 130.0).abs() < 1e-6);
        assert!(t.x < cursor.x, "debe quedarse del lado de origen");
        // cursor exactamente sobre la abeja → se aparta
        let t2 = follow_target(cursor, cursor, 130.0, &areas, WIN);
        assert!((t2.dist(cursor) - 130.0).abs() < 1e-6);
    }

    #[test]
    fn follow_target_near_edge_picks_valid_side() {
        let areas = [hd()];
        // cursor pegado a la esquina inferior derecha; la abeja viene desde la derecha-abajo
        let cursor = Vec2::new(1915.0, 1030.0);
        let cur = Vec2::new(1800.0, 900.0);
        let t = follow_target(cursor, cur, 130.0, &areas, WIN);
        assert!(t.dist(cursor) >= 130.0 * 0.7 - 1e-6, "no debe taparlo: {}", t.dist(cursor));
        assert!(fully_visible(&areas, t, WIN));
    }

    #[test]
    fn off_center_anchor_uses_full_window_geometry() {
        // ventana 260×200 con el sprite a (130, 128): hay 128 px sobre él (burbuja) y 72 debajo
        let win = Frame::with_anchor_frac(260.0, 200.0, 0.5, 0.64);
        let area = Rect::new(0, 0, 1920, 1032);
        let top_left = clamp_anchor(&[area], Vec2::new(-10.0, -10.0), win);
        assert_eq!(top_left, Vec2::new(130.0, 128.0));
        assert_eq!(window_origin(top_left, win), (0, 0));
        let bottom_right = clamp_anchor(&[area], Vec2::new(1e6, 1e6), win);
        let (ox, oy) = window_origin(bottom_right, win);
        assert_eq!((ox as f64 + win.w, oy as f64 + win.h), (1920.0, 1032.0));
        assert!(fully_visible(&[area], top_left, win) && fully_visible(&[area], bottom_right, win));
        assert_eq!(anchor_of((ox, oy), win), bottom_right);
    }

    #[test]
    fn off_center_anchor_respects_negative_origin_and_dpi() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let win = Frame::with_anchor_frac(260.0 * scale, 200.0 * scale, 0.5, 0.64);
            let area = Rect::new(-2560, -50, 2560, 1080);
            let c = clamp_anchor(&[area], Vec2::new(-1e6, 1e6), win);
            let (ox, oy) = window_origin(c, win);
            assert!((ox as f64 + 0.0 - (-2560.0)).abs() < 1.0, "x a {scale}");
            assert!((oy as f64 + win.h - 1030.0).abs() < 1.0, "y a {scale}");
        }
    }

    #[test]
    fn off_center_collapse_when_area_smaller_than_window() {
        let win = Frame::with_anchor_frac(260.0, 200.0, 0.5, 0.64);
        let tiny = Rect::new(0, 0, 100, 100);
        let c = clamp_anchor(&[tiny], Vec2::new(500.0, 500.0), win);
        // ventana centrada en el área: origen = (100-260)/2, (100-200)/2
        let (ox, oy) = window_origin(c, win);
        assert_eq!((ox, oy), (-80, -50));
    }
}
