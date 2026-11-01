//! Cinemática pura: integración con delta time, velocidad máxima y aceleración limitada.
//! Unidades: píxeles físicos y segundos.

use super::geometry::Vec2;

#[derive(Debug, Clone, Copy)]
pub struct MotionParams {
    /// Velocidad máxima (px/s).
    pub max_speed: f64,
    /// Aceleración máxima (px/s²): suaviza arranque y frenado.
    pub max_accel: f64,
    /// Por debajo de esta distancia al destino (y a baja velocidad) se considera llegada.
    pub tolerance: f64,
}

/// Ganancia de llegada (1/s): la velocidad deseada es `min(max_speed, dist * ARRIVE_GAIN)`,
/// lo que produce una desaceleración progresiva al acercarse.
const ARRIVE_GAIN: f64 = 3.5;
/// Un paso de simulación nunca supera este dt (s): evita saltos tras suspensión o stalls.
pub const MAX_DT: f64 = 0.1;

#[derive(Debug, Clone, Copy, Default)]
pub struct Motion {
    pub pos: Vec2,
    pub vel: Vec2,
}

impl Motion {
    pub fn at(pos: Vec2) -> Self {
        Self { pos, vel: Vec2::default() }
    }

    pub fn is_still(&self) -> bool {
        self.vel.len() < 0.5
    }

    pub fn stop(&mut self) {
        self.vel = Vec2::default();
    }

    /// Avanza `dt` segundos hacia `target` (o frena si es `None`).
    /// Devuelve `true` mientras la abeja sigue en movimiento.
    pub fn step(&mut self, target: Option<Vec2>, dt: f64, p: &MotionParams) -> bool {
        let dt = dt.clamp(0.0, MAX_DT);
        let desired = match target {
            Some(t) => {
                let to = t.minus(self.pos);
                let d = to.len();
                if d <= p.tolerance && self.vel.len() <= p.max_accel * dt.max(1e-3) {
                    // llegada: encaja exactamente y se detiene
                    self.pos = t;
                    self.vel = Vec2::default();
                    return false;
                }
                if d < 1e-9 {
                    Vec2::default()
                } else {
                    to.scale((p.max_speed.min(d * ARRIVE_GAIN)) / d)
                }
            }
            None => Vec2::default(),
        };
        // limitar el cambio de velocidad por la aceleración máxima
        let dv = desired.minus(self.vel);
        let max_dv = p.max_accel * dt;
        let dvl = dv.len();
        let dv = if dvl > max_dv && dvl > 0.0 { dv.scale(max_dv / dvl) } else { dv };
        self.vel = self.vel.plus(dv);
        if self.vel.len() > p.max_speed {
            self.vel = self.vel.scale(p.max_speed / self.vel.len());
        }
        self.pos = self.pos.plus(self.vel.scale(dt));
        match target {
            Some(_) => true,
            None => !self.is_still(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: MotionParams = MotionParams { max_speed: 300.0, max_accel: 900.0, tolerance: 1.5 };

    fn run(m: &mut Motion, target: Option<Vec2>, secs: f64, hz: f64) -> f64 {
        let dt = 1.0 / hz;
        let mut peak: f64 = 0.0;
        for _ in 0..(secs * hz) as usize {
            m.step(target, dt, &P);
            peak = peak.max(m.vel.len());
        }
        peak
    }

    #[test]
    fn never_exceeds_max_speed_even_for_far_targets() {
        let mut m = Motion::at(Vec2::new(0.0, 0.0));
        let peak = run(&mut m, Some(Vec2::new(10_000.0, 0.0)), 5.0, 30.0);
        assert!(peak <= P.max_speed + 1e-9, "pico {peak}");
        assert!(peak > P.max_speed * 0.95, "debe alcanzar la velocidad máxima");
    }

    #[test]
    fn converges_and_stops_within_tolerance() {
        let mut m = Motion::at(Vec2::new(0.0, 0.0));
        let t = Vec2::new(400.0, 250.0);
        run(&mut m, Some(t), 6.0, 30.0);
        assert!(m.pos.dist(t) <= P.tolerance);
        assert!(m.is_still());
        assert!(!m.step(Some(t), 1.0 / 30.0, &P), "ya llegó: no se mueve");
    }

    #[test]
    fn accelerates_gradually_not_instantly() {
        let mut m = Motion::at(Vec2::default());
        m.step(Some(Vec2::new(1000.0, 0.0)), 1.0 / 30.0, &P);
        assert!(m.vel.len() <= P.max_accel / 30.0 + 1e-9);
        assert!(m.vel.len() < P.max_speed);
    }

    #[test]
    fn decelerates_near_target_without_big_overshoot() {
        let mut m = Motion::at(Vec2::default());
        let t = Vec2::new(300.0, 0.0);
        let mut max_x: f64 = 0.0;
        for _ in 0..300 {
            m.step(Some(t), 1.0 / 30.0, &P);
            max_x = max_x.max(m.pos.x);
        }
        assert!(max_x < t.x + 15.0, "sobrepaso {}", max_x - t.x);
    }

    #[test]
    fn small_offsets_below_tolerance_do_not_move() {
        let mut m = Motion::at(Vec2::new(100.0, 100.0));
        let moving = m.step(Some(Vec2::new(100.5, 100.5)), 1.0 / 30.0, &P);
        assert!(!moving);
    }

    #[test]
    fn brakes_smoothly_without_target() {
        let mut m = Motion::at(Vec2::default());
        m.vel = Vec2::new(300.0, 0.0);
        let x0 = m.pos.x;
        let mut steps = 0;
        while m.step(None, 1.0 / 30.0, &P) {
            steps += 1;
            assert!(steps < 100);
        }
        assert!(m.pos.x > x0, "avanza un poco al frenar, sin parada en seco");
        assert!(m.is_still());
    }

    #[test]
    fn huge_dt_is_capped() {
        let mut m = Motion::at(Vec2::default());
        m.step(Some(Vec2::new(10_000.0, 0.0)), 60.0, &P); // pausa larga → un único paso de 0.1 s
        assert!(m.pos.x <= P.max_accel * MAX_DT * MAX_DT + 1e-9);
    }

    #[test]
    fn fast_cursor_changes_do_not_teleport() {
        let mut m = Motion::at(Vec2::default());
        for i in 0..30 {
            let t = if i % 2 == 0 { Vec2::new(5000.0, 0.0) } else { Vec2::new(-5000.0, 0.0) };
            let before = m.pos;
            m.step(Some(t), 1.0 / 30.0, &P);
            assert!(m.pos.dist(before) <= P.max_speed / 30.0 + 1e-9);
        }
    }
}
