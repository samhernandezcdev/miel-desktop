//! Registro mínimo de errores del frontend (React/WebView2) en un archivo local.
//!
//! - Sin red: solo un archivo de texto en el directorio de registros de la aplicación.
//! - Privacidad: los mensajes se sanean (rutas de disco, saltos de línea y longitud) antes de guardarse.
//! - Acotado: el archivo nunca supera `MAX_BYTES` (se conserva el final) y hay un tope de informes
//!   por ejecución, para que un bucle de errores no llene el disco ni la CPU.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const MAX_BYTES: u64 = 64 * 1024;
pub const MAX_REPORTS_PER_RUN: usize = 25;
const MAX_FIELD: usize = 400;

static REPORTS: AtomicUsize = AtomicUsize::new(0);

/// Quita saltos de línea y rutas de Windows (pueden contener el nombre de usuario) y acota la longitud.
pub fn sanitize(input: &str) -> String {
    let flat: String = input.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let mut out = String::with_capacity(flat.len());
    let b: Vec<char> = flat.chars().collect();
    let mut i = 0;
    while i < b.len() {
        // "X:\..." o "X:/..." → <ruta>
        if i + 2 < b.len() && b[i].is_ascii_alphabetic() && b[i + 1] == ':' && (b[i + 2] == '\\' || b[i + 2] == '/') {
            out.push_str("<ruta>");
            while i < b.len() && !b[i].is_whitespace() && b[i] != ')' && b[i] != '\'' && b[i] != '"' {
                i += 1;
            }
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out.chars().take(MAX_FIELD).collect()
}

/// Añade una línea y recorta el archivo para que no supere `max` bytes (conserva el final).
pub fn append_capped(path: &Path, line: &str, max: u64) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{line}")?;
    drop(f);
    if fs::metadata(path)?.len() > max {
        let data = fs::read(path)?;
        let keep = (max / 2) as usize;
        let tail = &data[data.len().saturating_sub(keep)..];
        // empezar en una línea completa
        let start = tail.iter().position(|b| *b == b'\n').map_or(0, |p| p + 1);
        fs::write(path, &tail[start..])?;
    }
    Ok(())
}

/// Registra un informe del frontend. Devuelve `false` si se superó el tope por ejecución.
pub fn report(path: &Path, kind: &str, message: &str) -> bool {
    if REPORTS.fetch_add(1, Ordering::SeqCst) >= MAX_REPORTS_PER_RUN {
        return false;
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("{secs} [{}] {}", sanitize(kind), sanitize(message));
    if let Err(e) = append_capped(path, &line, MAX_BYTES) {
        eprintln!("[miel] no se pudo escribir el registro de errores: {e}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("miel-errlog-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("frontend-errors.log")
    }

    #[test]
    fn sanitize_strips_paths_newlines_and_limits_length() {
        let s = sanitize("fallo en C:\\Users\\persona\\proyecto\\a.ts:3\nsegunda línea (D:/otra/ruta.js)");
        assert!(!s.contains("Users") && !s.contains("persona") && !s.contains("otra"), "{s}");
        assert!(s.contains("<ruta>") && !s.contains('\n'));
        assert_eq!(sanitize(&"x".repeat(5000)).chars().count(), MAX_FIELD);
        assert_eq!(sanitize("sin nada especial: 10:30"), "sin nada especial: 10:30", "no confunde horas con rutas");
    }

    #[test]
    fn file_is_capped_and_keeps_the_most_recent_lines() {
        let p = tmp("cap");
        for i in 0..2000 {
            append_capped(&p, &format!("línea número {i:05} con relleno para ocupar espacio"), 8 * 1024).unwrap();
        }
        let len = fs::metadata(&p).unwrap().len();
        assert!(len <= 8 * 1024, "{len}");
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("01999"), "conserva lo más reciente");
        assert!(!text.contains("00000"));
        assert!(text.lines().all(|l| l.starts_with("línea")), "sin líneas partidas");
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn report_writes_a_sanitized_line_and_is_rate_limited() {
        let p = tmp("rate");
        assert!(report(&p, "render", "boom en C:\\secreto\\x.ts"));
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("[render]") && !text.contains("secreto"));
        let mut accepted = 1;
        for _ in 0..200 {
            if report(&p, "x", "y") {
                accepted += 1;
            }
        }
        assert!(accepted <= MAX_REPORTS_PER_RUN, "{accepted}");
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }
}
