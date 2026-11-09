//! Reloj inyectable del motor. Sustituye a `Instant` interno: en producción `SystemClock` (monótono),
//! en pruebas `FakeClock` (avanza solo cuando la prueba lo ordena).
//!
//! Solo mide tiempo del motor. Las esperas reales del hilo (`Condvar`) siguen siendo de reloj de pared.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use super::state::Ms;

/// Fuente de tiempo monótona en milisegundos desde un origen propio (no es hora del día).
pub trait Clock: Send + Sync + 'static {
    fn now_ms(&self) -> Ms;
}

/// Reloj real: milisegundos desde su creación, con `Instant` (monótono, inmune a cambios de hora).
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> Ms {
        self.start.elapsed().as_millis() as Ms
    }
}

/// Reloj de pruebas: solo avanza con `advance`/`set`. Seguro entre hilos.
#[derive(Debug, Default)]
pub struct FakeClock {
    ms: AtomicU64,
}

impl FakeClock {
    pub fn new(start_ms: Ms) -> Self {
        Self { ms: AtomicU64::new(start_ms) }
    }

    pub fn advance(&self, delta_ms: Ms) {
        self.ms.fetch_add(delta_ms, Ordering::SeqCst);
    }

    pub fn set(&self, ms: Ms) {
        self.ms.store(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> Ms {
        self.ms.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn fake_clock_only_moves_when_told() {
        let c = FakeClock::new(5);
        assert_eq!(c.now_ms(), 5);
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(c.now_ms(), 5, "no avanza solo");
        c.advance(100);
        assert_eq!(c.now_ms(), 105);
        c.set(7);
        assert_eq!(c.now_ms(), 7);
        assert_eq!(FakeClock::default().now_ms(), 0);
    }

    #[test]
    fn fake_clock_is_shared_across_threads_and_trait_objects() {
        let c = Arc::new(FakeClock::new(0));
        let dynamic: Arc<dyn Clock> = c.clone();
        let c2 = Arc::clone(&c);
        std::thread::spawn(move || c2.advance(40)).join().unwrap();
        assert_eq!(dynamic.now_ms(), 40);
    }

    #[test]
    fn system_clock_is_monotonic_and_starts_near_zero() {
        let c = SystemClock::new();
        let a = c.now_ms();
        std::thread::sleep(std::time::Duration::from_millis(15));
        let b = c.now_ms();
        assert!(a < 1000, "empieza cerca de 0: {a}");
        assert!(b >= a + 10, "avanza: {a} -> {b}");
    }
}
