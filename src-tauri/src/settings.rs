//! Preferencias persistentes de Miel (archivo JSON local, gestionado solo desde Rust).
//!
//! Garantías:
//! - Ubicación: el directorio de configuración de la aplicación (API oficial de Tauri). Nada más.
//! - Esquema versionado (`schema_version`). Campos ausentes → valor por defecto; campos desconocidos
//!   (de versiones futuras) → se ignoran; valores con tipo erróneo → ese campo vuelve a su valor por
//!   defecto (no se descarta todo el archivo); valores fuera de rango → se ajustan al rango válido.
//! - Archivo ilegible o que no es un objeto JSON → se conserva una copia diagnóstica
//!   (`settings.corrupt-<unix>.json`, máx. 3) y se arranca con los valores por defecto.
//! - Archivo de una versión futura → se hace una copia (`settings.v<N>.bak.json`) antes de poder
//!   sobrescribirlo con el esquema actual.
//! - Escritura segura: archivo temporal en el mismo directorio + `sync_all` + reemplazo atómico
//!   (`std::fs::rename` usa `MoveFileExW` con reemplazo en Windows), con reintentos si un antivirus
//!   tiene el destino abierto.
//! - Sin escrituras constantes: solo se escribe cuando un valor cambia de verdad.
//! - No se guarda actividad de la persona: solo preferencias.
//! - `Hidden` NO se persiste: Miel siempre puede aparecer al iniciar.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 1;
pub const FILE_NAME: &str = "settings.json";
pub const ALLOWED_SIZES: [u32; 3] = [64, 96, 128];
const MAX_CORRUPT_BACKUPS: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settings {
    pub schema_version: u32,
    /// ¿Se permite el seguimiento del cursor?
    pub follow_enabled: bool,
    /// ¿Se muestran mensajes espontáneos?
    pub messages_enabled: bool,
    /// Tiempo mínimo entre mensajes (minutos, 1–1440).
    pub message_interval_minutes: u32,
    /// Lado del sprite en px: 64, 96 o 128.
    pub pet_size: u32,
    /// Velocidad máxima de movimiento (px lógicos/s, 60–600).
    pub speed: f64,
    /// Fuerza movimiento reducido (además de la preferencia del sistema).
    pub reduced_motion: bool,
    /// Miel estaba pausada.
    pub paused: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            follow_enabled: true,
            messages_enabled: true,
            message_interval_minutes: 12,
            pet_size: 96,
            speed: 260.0,
            reduced_motion: false,
            paused: false,
        }
    }
}

/// Resultado de interpretar un documento JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub settings: Settings,
    /// Campos presentes pero con tipo incorrecto (se usó el valor por defecto).
    pub invalid: Vec<&'static str>,
    /// Campos presentes pero fuera de rango (se ajustaron).
    pub clamped: Vec<&'static str>,
    /// Versión del esquema del archivo si es posterior a la que conoce esta aplicación.
    pub future_version: Option<u32>,
}

fn nearest_size(n: u32) -> u32 {
    *ALLOWED_SIZES.iter().min_by_key(|s| s.abs_diff(n)).unwrap_or(&96)
}

impl Settings {
    /// Interpreta un objeto JSON campo a campo (tolerante, ver cabecera del módulo).
    pub fn from_value(v: &Value) -> Option<Parsed> {
        let obj = v.as_object()?;
        let d = Settings::default();
        let mut invalid = Vec::new();
        let mut clamped = Vec::new();

        let mut get_bool = |key: &'static str, default: bool| -> bool {
            match obj.get(key) {
                None => default,
                Some(Value::Bool(b)) => *b,
                Some(_) => {
                    invalid.push(key);
                    default
                }
            }
        };
        let follow_enabled = get_bool("follow_enabled", d.follow_enabled);
        let messages_enabled = get_bool("messages_enabled", d.messages_enabled);
        let reduced_motion = get_bool("reduced_motion", d.reduced_motion);
        let paused = get_bool("paused", d.paused);

        let mut get_num = |key: &'static str| -> Option<f64> {
            match obj.get(key) {
                None => None,
                Some(Value::Number(n)) => match n.as_f64() {
                    Some(x) if x.is_finite() => Some(x),
                    _ => {
                        invalid.push(key);
                        None
                    }
                },
                Some(_) => {
                    invalid.push(key);
                    None
                }
            }
        };
        let interval = get_num("message_interval_minutes");
        let size = get_num("pet_size");
        let speed = get_num("speed");
        let version = get_num("schema_version");

        let message_interval_minutes = match interval {
            None => d.message_interval_minutes,
            Some(x) => {
                let c = x.round().clamp(1.0, 1440.0) as u32;
                if (c as f64 - x).abs() > 0.5 {
                    clamped.push("message_interval_minutes");
                }
                c
            }
        };
        let pet_size = match size {
            None => d.pet_size,
            Some(x) => {
                let n = x.round().clamp(0.0, 10_000.0) as u32;
                let s = nearest_size(n);
                if s != n {
                    clamped.push("pet_size");
                }
                s
            }
        };
        let speed = match speed {
            None => d.speed,
            Some(x) => {
                let c = x.clamp(60.0, 600.0);
                if (c - x).abs() > f64::EPSILON {
                    clamped.push("speed");
                }
                c
            }
        };
        let future_version = version.and_then(|x| {
            let n = x.max(0.0) as u32;
            (n > SCHEMA_VERSION).then_some(n)
        });

        Some(Parsed {
            settings: Settings {
                schema_version: SCHEMA_VERSION,
                follow_enabled,
                messages_enabled,
                message_interval_minutes,
                pet_size,
                speed,
                reduced_motion,
                paused,
            },
            invalid,
            clamped,
            future_version,
        })
    }
}

/// Qué ocurrió al cargar (para registrarlo y probarlo).
#[derive(Debug, Clone, PartialEq)]
pub enum LoadOutcome {
    /// No existía el archivo: valores por defecto (no se escribe hasta que algo cambie).
    Missing,
    Loaded { invalid: Vec<&'static str>, clamped: Vec<&'static str> },
    /// JSON ilegible o no es un objeto: valores por defecto; `backup` es la copia diagnóstica.
    Corrupt { backup: Option<PathBuf> },
    /// Archivo de una versión futura: valores conocidos + copia de seguridad.
    Future { version: u32, backup: Option<PathBuf> },
    /// No se pudo leer el archivo (permisos, etc.): valores por defecto.
    Unreadable(String),
}

fn unix_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn copy_diagnostic(path: &Path, name: String) -> Option<PathBuf> {
    let dest = path.parent()?.join(name);
    match fs::copy(path, &dest) {
        Ok(_) => Some(dest),
        Err(e) => {
            eprintln!("[miel] no se pudo guardar la copia diagnóstica de las preferencias: {e}");
            None
        }
    }
}

/// Conserva como mucho `MAX_CORRUPT_BACKUPS` copias diagnósticas (las más recientes).
fn prune_corrupt_backups(dir: &Path) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut names: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("settings.corrupt-") && n.ends_with(".json"))
        })
        .collect();
    names.sort();
    while names.len() > MAX_CORRUPT_BACKUPS {
        let _ = fs::remove_file(names.remove(0));
    }
}

/// Carga las preferencias. Nunca falla: ante cualquier problema devuelve valores válidos.
pub fn load_from(path: &Path) -> (Settings, LoadOutcome) {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (Settings::default(), LoadOutcome::Missing)
        }
        Err(e) => return (Settings::default(), LoadOutcome::Unreadable(e.to_string())),
    };
    let parsed = serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| Settings::from_value(&v));
    match parsed {
        None => {
            let backup = copy_diagnostic(path, format!("settings.corrupt-{}.json", unix_secs()));
            if let Some(dir) = path.parent() {
                prune_corrupt_backups(dir);
            }
            (Settings::default(), LoadOutcome::Corrupt { backup })
        }
        Some(p) => match p.future_version {
            Some(version) => {
                let backup = copy_diagnostic(path, format!("settings.v{version}.bak.json"));
                (p.settings, LoadOutcome::Future { version, backup })
            }
            None => (p.settings, LoadOutcome::Loaded { invalid: p.invalid, clamped: p.clamped }),
        },
    }
}

/// Escritura segura: temporal + `sync_all` + reemplazo atómico, con reintentos breves.
pub fn save_to(path: &Path, settings: &Settings) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| std::io::Error::other("ruta sin directorio"))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE_NAME}.tmp"));
    let json = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    let mut last = None;
    for attempt in 0..4 {
        match fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                sleep(Duration::from_millis(20 * (attempt + 1)));
            }
        }
    }
    let _ = fs::remove_file(&tmp);
    Err(last.unwrap_or_else(|| std::io::Error::other("rename falló")))
}

/// Estado persistente compartido. Solo escribe cuando algo cambia.
pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<Settings>,
    outcome: LoadOutcome,
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let (settings, outcome) = load_from(&path);
        Self { path, state: Mutex::new(settings), outcome }
    }

    pub fn outcome(&self) -> &LoadOutcome {
        &self.outcome
    }

    pub fn get(&self) -> Settings {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Aplica un cambio y lo persiste solo si el resultado difiere. `Ok(true)` si se escribió.
    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> std::io::Result<bool> {
        let new = {
            let mut g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let before = g.clone();
            f(&mut g);
            // se revalida (rangos) pasando por el parser: nunca se guarda un valor fuera de rango
            if let Some(p) = serde_json::to_value(&*g).ok().and_then(|v| Settings::from_value(&v)) {
                *g = p.settings;
            }
            if *g == before {
                return Ok(false);
            }
            g.clone()
        };
        save_to(&self.path, &new)?;
        Ok(true)
    }

    /// Refleja el estado real del motor (pausa y seguimiento) en las preferencias.
    pub fn mirror_runtime(&self, paused: bool, follow_enabled: bool) {
        if let Err(e) = self.update(|s| {
            s.paused = paused;
            s.follow_enabled = follow_enabled;
        }) {
            eprintln!("[miel] no se pudieron guardar las preferencias: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    /// Directorio temporal único por prueba (se borra al soltarlo).
    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "miel-test-{}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst),
                unix_secs()
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn file(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn defaults_are_reasonable_and_valid() {
        let d = Settings::default();
        assert_eq!(d.schema_version, SCHEMA_VERSION);
        assert!(d.follow_enabled && d.messages_enabled && !d.paused && !d.reduced_motion);
        assert_eq!((d.message_interval_minutes, d.pet_size), (12, 96));
        assert!((60.0..=600.0).contains(&d.speed));
        // el valor por defecto es un punto fijo de la validación
        let round = Settings::from_value(&serde_json::to_value(&d).unwrap()).unwrap();
        assert_eq!(round.settings, d);
        assert!(round.invalid.is_empty() && round.clamped.is_empty());
    }

    #[test]
    fn missing_file_gives_defaults_and_writes_nothing() {
        let t = TempDir::new();
        let store = SettingsStore::load(t.file());
        assert_eq!(*store.outcome(), LoadOutcome::Missing);
        assert_eq!(store.get(), Settings::default());
        assert!(!t.file().exists(), "arrancar no escribe");
    }

    #[test]
    fn save_then_load_roundtrips_every_field() {
        let t = TempDir::new();
        let s = Settings {
            follow_enabled: false,
            messages_enabled: false,
            message_interval_minutes: 45,
            pet_size: 128,
            speed: 333.0,
            reduced_motion: true,
            paused: true,
            ..Settings::default()
        };
        save_to(&t.file(), &s).unwrap();
        let (loaded, outcome) = load_from(&t.file());
        assert_eq!(loaded, s);
        assert_eq!(outcome, LoadOutcome::Loaded { invalid: vec![], clamped: vec![] });
    }

    #[test]
    fn write_is_atomic_and_leaves_no_temp_file() {
        let t = TempDir::new();
        save_to(&t.file(), &Settings::default()).unwrap();
        let s = Settings { paused: true, ..Settings::default() };
        save_to(&t.file(), &s).unwrap(); // reemplaza un archivo existente
        let names: Vec<String> =
            fs::read_dir(&t.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into()).collect();
        assert_eq!(names, [FILE_NAME], "sin .tmp residual: {names:?}");
        assert!(load_from(&t.file()).0.paused);
    }

    #[test]
    fn saved_file_is_valid_json_with_schema_version() {
        let t = TempDir::new();
        save_to(&t.file(), &Settings::default()).unwrap();
        let v: Value = serde_json::from_slice(&fs::read(t.file()).unwrap()).unwrap();
        assert_eq!(v["schema_version"], SCHEMA_VERSION);
        assert_eq!(v["pet_size"], 96);
    }

    #[test]
    fn corrupt_json_falls_back_to_defaults_and_keeps_a_diagnostic_copy() {
        let t = TempDir::new();
        fs::write(t.file(), b"{ esto no es json").unwrap();
        let (s, o) = load_from(&t.file());
        assert_eq!(s, Settings::default());
        let LoadOutcome::Corrupt { backup: Some(b) } = o else { panic!("{o:?}") };
        assert_eq!(fs::read(&b).unwrap(), b"{ esto no es json", "copia fiel para diagnóstico");
        assert!(t.file().exists(), "el original no se destruye al cargar");
    }

    #[test]
    fn non_object_json_and_empty_file_are_treated_as_corrupt() {
        for body in [&b"[1,2,3]"[..], b"42", b"\"hola\"", b"null", b""] {
            let t = TempDir::new();
            fs::write(t.file(), body).unwrap();
            let (s, o) = load_from(&t.file());
            assert_eq!(s, Settings::default());
            assert!(matches!(o, LoadOutcome::Corrupt { .. }), "{body:?} → {o:?}");
        }
    }

    #[test]
    fn only_the_latest_three_diagnostic_copies_are_kept() {
        let t = TempDir::new();
        for i in 0..6u32 {
            fs::write(t.0.join(format!("settings.corrupt-{}.json", 1000 + i)), b"x").unwrap();
        }
        fs::write(t.file(), b"basura").unwrap();
        load_from(&t.file());
        let n = fs::read_dir(&t.0)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("settings.corrupt-"))
            .count();
        assert_eq!(n, MAX_CORRUPT_BACKUPS);
        assert!(!t.0.join("settings.corrupt-1000.json").exists(), "se borran las más antiguas");
    }

    #[test]
    fn wrong_typed_fields_reset_only_that_field() {
        let v = json!({"follow_enabled": "sí", "paused": true, "pet_size": "grande", "speed": null,
                       "message_interval_minutes": 30});
        let p = Settings::from_value(&v).unwrap();
        assert!(p.settings.follow_enabled, "tipo erróneo → defecto");
        assert!(p.settings.paused, "el resto se conserva");
        assert_eq!(p.settings.message_interval_minutes, 30);
        assert_eq!(p.settings.pet_size, 96);
        assert_eq!(p.settings.speed, 260.0);
        for f in ["follow_enabled", "pet_size", "speed"] {
            assert!(p.invalid.contains(&f), "{f} en {:?}", p.invalid);
        }
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let p = Settings::from_value(&json!({"speed": 99999, "message_interval_minutes": 0, "pet_size": 100})).unwrap();
        assert_eq!(p.settings.speed, 600.0);
        assert_eq!(p.settings.message_interval_minutes, 1);
        assert_eq!(p.settings.pet_size, 96, "tamaño al valor permitido más cercano");
        assert_eq!(p.clamped.len(), 3);
        let p = Settings::from_value(&json!({"speed": -5, "message_interval_minutes": 99999, "pet_size": 9999})).unwrap();
        assert_eq!((p.settings.speed, p.settings.message_interval_minutes, p.settings.pet_size), (60.0, 1440, 128));
        let p = Settings::from_value(&json!({"pet_size": 64.4, "speed": 1e308})).unwrap();
        assert_eq!((p.settings.pet_size, p.settings.speed), (64, 600.0));
    }

    #[test]
    fn every_allowed_size_is_accepted_unchanged() {
        for s in ALLOWED_SIZES {
            let p = Settings::from_value(&json!({ "pet_size": s })).unwrap();
            assert_eq!(p.settings.pet_size, s);
            assert!(p.clamped.is_empty());
        }
    }

    #[test]
    fn missing_fields_use_defaults_and_unknown_fields_are_ignored() {
        let p = Settings::from_value(&json!({"paused": true, "campo_nuevo": {"a": 1}})).unwrap();
        assert!(p.settings.paused);
        assert_eq!(p.settings.follow_enabled, Settings::default().follow_enabled);
        assert!(p.invalid.is_empty() && p.future_version.is_none());
    }

    #[test]
    fn future_schema_is_loaded_tolerantly_and_backed_up_before_any_overwrite() {
        let t = TempDir::new();
        let body = br#"{"schema_version": 7, "paused": true, "pet_size": 128, "nuevo": [1,2]}"#;
        fs::write(t.file(), body).unwrap();
        let (s, o) = load_from(&t.file());
        assert!(s.paused && s.pet_size == 128);
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        let LoadOutcome::Future { version: 7, backup: Some(b) } = o else { panic!("{o:?}") };
        assert_eq!(fs::read(b).unwrap(), body, "los campos desconocidos se conservan en la copia");
    }

    #[test]
    fn store_writes_only_on_real_changes() {
        let t = TempDir::new();
        let store = SettingsStore::load(t.file());
        assert!(!store.update(|_| {}).unwrap(), "sin cambio no hay escritura");
        assert!(!t.file().exists());
        assert!(store.update(|s| s.paused = true).unwrap());
        let m1 = fs::metadata(t.file()).unwrap().modified().unwrap();
        for _ in 0..5 {
            assert!(!store.update(|s| s.paused = true).unwrap(), "mismo valor: sin escritura");
        }
        assert_eq!(fs::metadata(t.file()).unwrap().modified().unwrap(), m1);
        store.mirror_runtime(true, true); // igual que el estado actual: no escribe
        assert_eq!(fs::metadata(t.file()).unwrap().modified().unwrap(), m1);
    }

    #[test]
    fn store_never_persists_out_of_range_values() {
        let t = TempDir::new();
        let store = SettingsStore::load(t.file());
        store.update(|s| {
            s.speed = 1e9;
            s.pet_size = 77;
            s.message_interval_minutes = 0;
        })
        .unwrap();
        let on_disk = load_from(&t.file()).0;
        assert_eq!((on_disk.speed, on_disk.pet_size, on_disk.message_interval_minutes), (600.0, 64, 1));
        assert_eq!(store.get(), on_disk);
    }

    #[test]
    fn preferences_survive_a_restart() {
        let t = TempDir::new();
        {
            let a = SettingsStore::load(t.file());
            a.mirror_runtime(true, false); // el usuario pausó y desactivó el seguimiento desde la bandeja
        }
        let b = SettingsStore::load(t.file()); // "reinicio"
        let s = b.get();
        assert!(s.paused && !s.follow_enabled);
        assert!(matches!(b.outcome(), LoadOutcome::Loaded { .. }));
    }

    #[test]
    fn unreadable_path_is_reported_without_panicking() {
        let t = TempDir::new();
        // un directorio donde se espera el archivo → la lectura falla pero no es "no existe"
        fs::create_dir_all(t.file()).unwrap();
        let (s, o) = load_from(&t.file());
        assert_eq!(s, Settings::default());
        assert!(matches!(o, LoadOutcome::Unreadable(_) | LoadOutcome::Corrupt { .. }), "{o:?}");
    }

    #[test]
    fn save_failure_is_an_error_not_a_panic() {
        let t = TempDir::new();
        fs::create_dir_all(t.file()).unwrap(); // el destino es un directorio: el reemplazo falla
        let r = save_to(&t.file(), &Settings::default());
        assert!(r.is_err());
        assert!(!t.0.join(format!("{FILE_NAME}.tmp")).exists(), "no deja el temporal");
    }
}
