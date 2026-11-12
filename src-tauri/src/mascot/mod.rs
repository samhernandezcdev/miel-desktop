//! Lógica de la mascota. `geometry`, `motion` y `state` son puros (sin Tauri); `engine`
//! los orquesta en un único hilo; `host` conecta con Tauri.

pub mod clock;
pub mod dnd;
pub mod engine;
pub mod geometry;
pub mod host;
pub mod messages;
pub mod motion;
pub mod probe;
pub mod sleep;
pub mod state;
