//! Lógica de la mascota. `geometry`, `motion` y `state` son puros (sin Tauri); `engine`
//! los orquesta en un único hilo; `host` conecta con Tauri.

pub mod engine;
pub mod geometry;
pub mod host;
pub mod messages;
pub mod motion;
pub mod state;
