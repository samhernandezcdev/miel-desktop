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
//! - Esquema 2 (v0.2.0, D5): un archivo de esquema 1 (o sin `schema_version`) se carga igual (campos
//!   nuevos por defecto) y se copia a `settings.v1.bak.json` ANTES de que ningún guardado lo
//!   reescriba como esquema 2. Esa copia nunca se sobrescribe (se conserva la más antigua).
//! - IPC: solo `SettingsStore::set_setting` (lista cerrada de claves, tipos estrictos, rangos
//!   saneados en Rust, devuelve el valor aplicado). El frontend no decide nada.
//! - Escritura segura: archivo temporal en el mismo directorio + `sync_all` + reemplazo atómico
//!   (`std::fs::rename` usa `MoveFileExW` con reemplazo en Windows), con reintentos si un antivirus
//!   tiene el destino abierto.
//! - Sin escrituras constantes: solo se escribe cuando un valor cambia de verdad.
//! - No se guarda actividad de la persona: solo preferencias.
//! - `Hidden` NO se persiste: Miel siempre puede aparecer al iniciar.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

pub const SCHEMA_VERSION: u32 = 2;
pub const FILE_NAME: &str = "settings.json";
pub const ALLOWED_SIZES: [u32; 3] = [64, 96, 128];
/// Umbrales de inactividad para dormir (minutos; D8). Predeterminado: 10.
pub const ALLOWED_SLEEP_MINUTES: [u32; 5] = [5, 10, 15, 30, 60];
/// Copia del archivo de esquema 1 hecha antes de la primera migración (D5). Nunca se sobrescribe.
pub const V1_BACKUP_NAME: &str = "settings.v1.bak.json";
/// Evento (Rust → ventana `settings`) con las preferencias completas tras un cambio real.
pub const EVENT_CHANGED: &str = "settings://changed";
/// Lista cerrada de claves que admite `set_setting`. Excluye `schema_version` y `paused`/`follow_enabled`
/// (estado en ejecución que refleja el motor; su cambio desde la ventana lo decide E3).
pub const SETTABLE_KEYS: [&str; 10] = [
    "messages_enabled",
    "message_interval_minutes",
    "pet_size",
    "speed",
    "reduced_motion",
    "dnd_auto_enabled",
    "sleep_enabled",
    "sleep_after_minutes",
    "proximity_enabled",
    "time_of_day_enabled",
];
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
    // ---- esquema 2 (v0.2.0). Ninguna de estas funciones existe todavía: E4, E5, E6 y E6b las consumen. ----
    /// No molestar automático (E4). Por defecto activado.
    pub dnd_auto_enabled: bool,
    /// Dormir por inactividad (E5). Por defecto activado.
    pub sleep_enabled: bool,
    /// Minutos de inactividad para dormir: 5, 10, 15, 30 o 60 (D8).
    pub sleep_after_minutes: u32,
    /// Proximidad del cursor (E6). Por defecto activada.
    pub proximity_enabled: bool,
    /// Mensajes por franja horaria local (E6b). Por defecto activado.
    pub time_of_day_enabled: bool,
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
            dnd_auto_enabled: true,
            sleep_enabled: true,
            sleep_after_minutes: 10,
            proximity_enabled: true,
            time_of_day_enabled: true,
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
    /// El documento es de esquema 1 (o no declara `schema_version`): hay que migrarlo a 2 (D5).
    pub migrated: bool,
}

fn nearest_size(n: u32) -> u32 {
    *ALLOWED_SIZES.iter().min_by_key(|s| s.abs_diff(n)).unwrap_or(&96)
}

/// Umbral permitido más cercano (empate → el menor).
fn nearest_sleep(n: u32) -> u32 {
    *ALLOWED_SLEEP_MINUTES.iter().min_by_key(|s| s.abs_diff(n)).unwrap_or(&10)
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
        let dnd_auto_enabled = get_bool("dnd_auto_enabled", d.dnd_auto_enabled);
        let sleep_enabled = get_bool("sleep_enabled", d.sleep_enabled);
        let proximity_enabled = get_bool("proximity_enabled", d.proximity_enabled);
        let time_of_day_enabled = get_bool("time_of_day_enabled", d.time_of_day_enabled);

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
        let sleep_after = get_num("sleep_after_minutes");
        let version = get_num("schema_version");

        let sleep_after_minutes = match sleep_after {
            None => d.sleep_after_minutes,
            Some(x) => {
                let n = x.round().clamp(0.0, 10_000.0) as u32;
                let s = nearest_sleep(n);
                if s != n {
                    clamped.push("sleep_after_minutes");
                }
                s
            }
        };
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
        // Sin `schema_version` (o con tipo erróneo) se trata como esquema 1: v0.1.0 siempre lo escribía.
        let migrated = future_version.is_none() && version.is_none_or(|x| x < f64::from(SCHEMA_VERSION));

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
                dnd_auto_enabled,
                sleep_enabled,
                sleep_after_minutes,
                proximity_enabled,
                time_of_day_enabled,
            },
            invalid,
            clamped,
            future_version,
            migrated,
        })
    }
}

/// Qué ocurrió al cargar (para registrarlo y probarlo).
#[derive(Debug, Clone, PartialEq)]
pub enum LoadOutcome {
    /// No existía el archivo: valores por defecto (no se escribe hasta que algo cambie).
    Missing,
    Loaded { invalid: Vec<&'static str>, clamped: Vec<&'static str> },
    /// Archivo de esquema 1: cargado con los campos nuevos por defecto. `backup` es `settings.v1.bak.json`
    /// (`None` si no se pudo crear: el primer guardado lo reintenta y, si falla, no sobrescribe el original).
    /// El archivo NO se reescribe al cargar: pasa a esquema 2 en el primer guardado.
    Migrated { backup: Option<PathBuf>, invalid: Vec<&'static str>, clamped: Vec<&'static str> },
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

/// Copia `path` (esquema 1) a `settings.v1.bak.json` junto a él, SIN sobrescribir nunca una copia
/// existente (se conserva la más antigua; `create_new` lo garantiza de forma atómica).
/// `Ok(None)` si el archivo original ya no existe (no hay nada que copiar).
fn ensure_v1_backup(path: &Path) -> std::io::Result<Option<PathBuf>> {
    let dest = path.parent().ok_or_else(|| std::io::Error::other("ruta sin directorio"))?.join(V1_BACKUP_NAME);
    if dest.exists() {
        return Ok(Some(dest));
    }
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut f = match fs::OpenOptions::new().write(true).create_new(true).open(&dest) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(Some(dest)),
        Err(e) => return Err(e),
    };
    let written = f.write_all(&bytes).and_then(|()| f.sync_all());
    drop(f);
    if let Err(e) = written {
        let _ = fs::remove_file(&dest); // nunca queda una copia a medias
        return Err(e);
    }
    Ok(Some(dest))
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
            None if p.migrated => {
                let backup = ensure_v1_backup(path).unwrap_or_else(|e| {
                    eprintln!("[miel] no se pudo guardar la copia settings.v1.bak.json: {e}");
                    None
                });
                (p.settings, LoadOutcome::Migrated { backup, invalid: p.invalid, clamped: p.clamped })
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
    /// El archivo en disco es de esquema 1 y su copia aún no existe: hay que hacerla antes de guardar.
    v1_backup_pending: AtomicBool,
}

/// Por qué `set_setting` rechazó la petición (la configuración no cambia en ningún caso).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetError {
    /// La clave no está en `SETTABLE_KEYS`.
    UnknownKey(String),
    /// El valor no tiene el tipo que exige la clave.
    WrongType { key: &'static str, expected: &'static str },
    /// No se pudo guardar (el estado en memoria ya refleja el cambio).
    Io(String),
}

impl std::fmt::Display for SetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownKey(k) => write!(f, "clave de ajuste desconocida: {k:?}"),
            Self::WrongType { key, expected } => write!(f, "{key}: se esperaba {expected}"),
            Self::Io(e) => write!(f, "no se pudo guardar: {e}"),
        }
    }
}

/// Aplica un cambio tipado a `s` (sin rangos: los sanea `SettingsStore::update` al revalidar).
fn apply_change(s: &mut Settings, key: &str, v: &Value) -> Result<(), SetError> {
    let key_static = SETTABLE_KEYS.iter().copied().find(|k| *k == key).ok_or_else(|| SetError::UnknownKey(key.to_string()))?;
    let as_bool = || v.as_bool().ok_or(SetError::WrongType { key: key_static, expected: "booleano" });
    let as_num = || match v {
        Value::Number(n) => n.as_f64().filter(|x| x.is_finite()),
        _ => None,
    }
    .ok_or(SetError::WrongType { key: key_static, expected: "número" });
    match key_static {
        "messages_enabled" => s.messages_enabled = as_bool()?,
        "reduced_motion" => s.reduced_motion = as_bool()?,
        "dnd_auto_enabled" => s.dnd_auto_enabled = as_bool()?,
        "sleep_enabled" => s.sleep_enabled = as_bool()?,
        "proximity_enabled" => s.proximity_enabled = as_bool()?,
        "time_of_day_enabled" => s.time_of_day_enabled = as_bool()?,
        "message_interval_minutes" => s.message_interval_minutes = as_num()?.round().clamp(0.0, 1e6) as u32,
        "pet_size" => s.pet_size = as_num()?.round().clamp(0.0, 1e6) as u32,
        "sleep_after_minutes" => s.sleep_after_minutes = as_num()?.round().clamp(0.0, 1e6) as u32,
        "speed" => s.speed = as_num()?,
        _ => return Err(SetError::UnknownKey(key.to_string())), // inalcanzable: SETTABLE_KEYS ya filtró
    }
    Ok(())
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let (settings, outcome) = load_from(&path);
        let pending = matches!(outcome, LoadOutcome::Migrated { backup: None, .. });
        Self { path, state: Mutex::new(settings), outcome, v1_backup_pending: AtomicBool::new(pending) }
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
        // D5: el archivo de esquema 1 se copia antes de que este primer guardado lo reescriba como 2.
        // Si la copia no es posible, NO se sobrescribe el original (el cambio sigue en memoria).
        if self.v1_backup_pending.load(Ordering::SeqCst) {
            ensure_v1_backup(&self.path)?;
            self.v1_backup_pending.store(false, Ordering::SeqCst);
        }
        save_to(&self.path, &new)?;
        Ok(true)
    }

    /// Cambia UN ajuste por clave (IPC de la ventana de ajustes). Valida tipo y clave, sanea rangos y
    /// devuelve las preferencias aplicadas y si hubo un cambio real (solo entonces se escribe y se avisa).
    /// Un valor rechazado no modifica nada ni escribe.
    pub fn set_setting(&self, key: &str, value: &Value) -> Result<(Settings, bool), SetError> {
        apply_change(&mut self.get(), key, value)?; // valida clave y tipo sobre una copia
        let changed = self
            .update(|s| {
                let _ = apply_change(s, key, value); // ya validado arriba
            })
            .map_err(|e| SetError::Io(e.to_string()))?;
        Ok((self.get(), changed))
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

    // ================= E2: esquema 2, migración, IPC =================
    // Todas usan directorios temporales propios: ninguna toca el `settings.json` real de la persona.

    /// Archivo literal de v0.1.0 (esquema 1, los 7 campos que escribía).
    const V1_FILE: &str = r#"{
  "schema_version": 1,
  "follow_enabled": false,
  "messages_enabled": false,
  "message_interval_minutes": 45,
  "pet_size": 128,
  "speed": 333.0,
  "reduced_motion": true,
  "paused": true
}
"#;

    fn names(t: &TempDir) -> Vec<String> {
        let mut v: Vec<String> =
            fs::read_dir(&t.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into()).collect();
        v.sort();
        v
    }

    #[test]
    fn schema_2_defaults_are_the_documented_ones() {
        let d = Settings::default();
        assert_eq!(d.schema_version, 2);
        assert!(d.dnd_auto_enabled && d.sleep_enabled && d.proximity_enabled && d.time_of_day_enabled);
        assert_eq!(d.sleep_after_minutes, 10);
        assert_eq!(ALLOWED_SLEEP_MINUTES, [5, 10, 15, 30, 60]);
    }

    #[test]
    fn schema_2_roundtrips_every_field_through_the_file() {
        let t = TempDir::new();
        let s = Settings {
            follow_enabled: false,
            messages_enabled: false,
            message_interval_minutes: 45,
            pet_size: 64,
            speed: 123.0,
            reduced_motion: true,
            paused: true,
            dnd_auto_enabled: false,
            sleep_enabled: false,
            sleep_after_minutes: 30,
            proximity_enabled: false,
            time_of_day_enabled: false,
            ..Settings::default()
        };
        save_to(&t.file(), &s).unwrap();
        let (loaded, outcome) = load_from(&t.file());
        assert_eq!(loaded, s);
        assert_eq!(outcome, LoadOutcome::Loaded { invalid: vec![], clamped: vec![] }, "esquema 2: sin migración");
        assert_eq!(names(&t), [FILE_NAME], "un archivo de esquema 2 no genera copias");
    }

    #[test]
    fn v1_file_loads_clean_keeps_its_values_and_is_not_rewritten_at_load() {
        let t = TempDir::new();
        fs::write(t.file(), V1_FILE).unwrap();
        let (s, o) = load_from(&t.file());
        // valores de v1 intactos
        assert!(!s.follow_enabled && !s.messages_enabled && s.reduced_motion && s.paused);
        assert_eq!((s.message_interval_minutes, s.pet_size, s.speed), (45, 128, 333.0));
        // campos nuevos por defecto; en memoria ya es esquema 2
        assert!(s.dnd_auto_enabled && s.sleep_enabled && s.proximity_enabled && s.time_of_day_enabled);
        assert_eq!((s.schema_version, s.sleep_after_minutes), (2, 10));
        let LoadOutcome::Migrated { backup: Some(b), invalid, clamped } = o else { panic!("{o:?}") };
        assert!(invalid.is_empty() && clamped.is_empty(), "un archivo de v0.1.0 carga sin avisos");
        assert_eq!(b, t.0.join(V1_BACKUP_NAME));
        assert_eq!(fs::read(&b).unwrap(), V1_FILE.as_bytes(), "copia fiel del original");
        assert_eq!(fs::read(t.file()).unwrap(), V1_FILE.as_bytes(), "cargar no reescribe: sigue siendo esquema 1");
    }

    #[test]
    fn v1_empty_partial_and_extra_field_files_migrate() {
        // vacío (objeto sin campos), solo versión, parcial y con campos desconocidos
        let cases: [(&str, &str); 4] = [
            ("{}", "vacío"),
            (r#"{"schema_version": 1}"#, "solo versión"),
            (r#"{"schema_version": 1, "paused": true, "speed": 200}"#, "parcial"),
            (r#"{"schema_version": 1, "paused": true, "campo_extra": {"a": [1, 2]}, "otro": "x"}"#, "campos extra"),
        ];
        for (body, label) in cases {
            let t = TempDir::new();
            fs::write(t.file(), body).unwrap();
            let store = SettingsStore::load(t.file());
            let LoadOutcome::Migrated { backup: Some(b), invalid, clamped } = store.outcome().clone() else {
                panic!("{label}: {:?}", store.outcome())
            };
            assert!(invalid.is_empty() && clamped.is_empty(), "{label}");
            assert_eq!(fs::read(b).unwrap(), body.as_bytes(), "{label}: la copia conserva hasta los campos extra");
            let s = store.get();
            assert_eq!(s.schema_version, 2, "{label}");
            assert_eq!(s.sleep_after_minutes, 10, "{label}");
            assert!(s.dnd_auto_enabled && s.sleep_enabled && s.proximity_enabled && s.time_of_day_enabled, "{label}");
            if label == "parcial" {
                assert!(s.paused && s.speed == 200.0);
                assert_eq!(s.pet_size, 96, "ausente → defecto");
            }
            // primer guardado → esquema 2 en disco, sin campos desconocidos, y la copia no cambia
            assert!(store.update(|s| s.messages_enabled = false).unwrap(), "{label}");
            let v: Value = serde_json::from_slice(&fs::read(t.file()).unwrap()).unwrap();
            assert_eq!(v["schema_version"], 2, "{label}");
            assert_eq!(v["sleep_after_minutes"], 10, "{label}");
            assert!(v.get("campo_extra").is_none(), "{label}: lo desconocido no se reescribe (queda en la copia)");
            assert_eq!(fs::read(t.0.join(V1_BACKUP_NAME)).unwrap(), body.as_bytes(), "{label}");
        }
    }

    #[test]
    fn schema_version_missing_or_mistyped_is_treated_as_schema_1() {
        for body in [r#"{"paused": true}"#, r#"{"schema_version": "1", "paused": true}"#, r#"{"schema_version": 0}"#] {
            let t = TempDir::new();
            fs::write(t.file(), body).unwrap();
            let (_, o) = load_from(&t.file());
            assert!(matches!(o, LoadOutcome::Migrated { .. }), "{body} → {o:?}");
        }
    }

    #[test]
    fn v1_migration_with_bad_values_still_migrates_and_reports_them() {
        let t = TempDir::new();
        fs::write(t.file(), r#"{"schema_version": 1, "speed": 99999, "paused": "sí"}"#).unwrap();
        let (s, o) = load_from(&t.file());
        assert_eq!((s.speed, s.paused), (600.0, false));
        let LoadOutcome::Migrated { backup: Some(_), invalid, clamped } = o else { panic!("{o:?}") };
        assert_eq!((invalid, clamped), (vec!["paused"], vec!["speed"]));
    }

    #[test]
    fn first_save_after_migration_writes_schema_2_and_keeps_the_original_in_the_backup() {
        let t = TempDir::new();
        fs::write(t.file(), V1_FILE).unwrap();
        let store = SettingsStore::load(t.file());
        assert_eq!(fs::read(t.file()).unwrap(), V1_FILE.as_bytes());
        assert!(!store.update(|_| {}).unwrap(), "sin cambio no se escribe: sigue siendo esquema 1");
        assert_eq!(fs::read(t.file()).unwrap(), V1_FILE.as_bytes());
        assert!(store.update(|s| s.sleep_after_minutes = 60).unwrap());
        let (on_disk, o) = load_from(&t.file());
        assert_eq!((on_disk.schema_version, on_disk.sleep_after_minutes), (2, 60));
        assert!(on_disk.paused && on_disk.pet_size == 128, "sin pérdida de los valores de v1");
        assert_eq!(o, LoadOutcome::Loaded { invalid: vec![], clamped: vec![] });
        assert_eq!(fs::read(t.0.join(V1_BACKUP_NAME)).unwrap(), V1_FILE.as_bytes());
        assert_eq!(names(&t), [FILE_NAME, V1_BACKUP_NAME], "sin temporales residuales");
    }

    #[test]
    fn an_existing_v1_backup_is_never_overwritten() {
        let t = TempDir::new();
        let old = b"COPIA ANTIGUA - no tocar";
        fs::write(t.0.join(V1_BACKUP_NAME), old).unwrap();
        fs::write(t.file(), V1_FILE).unwrap();
        let store = SettingsStore::load(t.file());
        let LoadOutcome::Migrated { backup: Some(b), .. } = store.outcome().clone() else { panic!() };
        assert_eq!(b, t.0.join(V1_BACKUP_NAME));
        assert_eq!(fs::read(&b).unwrap(), old, "al cargar");
        assert!(store.update(|s| s.paused = false).unwrap());
        assert_eq!(fs::read(&b).unwrap(), old, "tras el primer guardado");
        // ciclo bajar de versión y volver a subir: el archivo vuelve a ser v1, la primera copia sigue intacta
        let newer = br#"{"schema_version": 1, "paused": true, "speed": 111}"#;
        fs::write(t.file(), newer).unwrap();
        let again = SettingsStore::load(t.file());
        assert!(again.update(|s| s.speed = 222.0).unwrap());
        assert_eq!(fs::read(&b).unwrap(), old, "tras una segunda migración");
    }

    #[test]
    fn the_backup_is_made_before_the_first_save_even_if_it_failed_at_load() {
        let t = TempDir::new();
        fs::write(t.file(), V1_FILE).unwrap();
        let store = SettingsStore::load(t.file());
        // se simula que la copia del arranque no existió (p. ej. disco lleno) y se reintenta al guardar
        fs::remove_file(t.0.join(V1_BACKUP_NAME)).unwrap();
        store.v1_backup_pending.store(true, Ordering::SeqCst);
        assert!(store.update(|s| s.paused = false).unwrap());
        assert_eq!(fs::read(t.0.join(V1_BACKUP_NAME)).unwrap(), V1_FILE.as_bytes());
        assert!(!store.v1_backup_pending.load(Ordering::SeqCst));
    }

    #[test]
    fn corrupt_file_keeps_its_copy_and_is_never_deleted_by_loading() {
        let t = TempDir::new();
        let body = b"{ \"schema_version\": 1, \"paused\": tru";
        fs::write(t.file(), body).unwrap();
        let store = SettingsStore::load(t.file());
        let LoadOutcome::Corrupt { backup: Some(b) } = store.outcome().clone() else { panic!("{:?}", store.outcome()) };
        assert_eq!(store.get(), Settings::default());
        assert_eq!(fs::read(&b).unwrap(), body);
        assert_eq!(fs::read(t.file()).unwrap(), body, "el original sigue ahí tras cargar");
        assert!(!t.0.join(V1_BACKUP_NAME).exists(), "un archivo corrupto no es una migración");
        // el primer guardado lo reemplaza, pero la copia diagnóstica sobrevive
        assert!(store.update(|s| s.paused = true).unwrap());
        assert_eq!(fs::read(&b).unwrap(), body);
        assert!(load_from(&t.file()).0.paused);
    }

    #[test]
    fn v2_file_with_invalid_new_fields_uses_defaults_and_reports_them() {
        let v = json!({"schema_version": 2, "dnd_auto_enabled": "no", "sleep_after_minutes": "mucho",
                       "proximity_enabled": 1, "sleep_enabled": false, "time_of_day_enabled": null});
        let p = Settings::from_value(&v).unwrap();
        assert!(!p.migrated, "esquema 2 no se migra");
        assert!(p.settings.dnd_auto_enabled && p.settings.proximity_enabled && p.settings.time_of_day_enabled);
        assert_eq!(p.settings.sleep_after_minutes, 10);
        assert!(!p.settings.sleep_enabled, "el campo válido se conserva");
        for f in ["dnd_auto_enabled", "sleep_after_minutes", "proximity_enabled", "time_of_day_enabled"] {
            assert!(p.invalid.contains(&f), "{f} en {:?}", p.invalid);
        }
    }

    #[test]
    fn sleep_threshold_accepts_only_the_allowed_values() {
        for m in ALLOWED_SLEEP_MINUTES {
            let p = Settings::from_value(&json!({ "schema_version": 2, "sleep_after_minutes": m })).unwrap();
            assert_eq!(p.settings.sleep_after_minutes, m);
            assert!(p.clamped.is_empty(), "{m}");
        }
        for (given, expect) in [(0, 5), (1, 5), (7, 5), (8, 10), (12, 10), (13, 15), (20, 15), (22, 15), (23, 30), (45, 30), (46, 60), (9999, 60)] {
            let p = Settings::from_value(&json!({ "sleep_after_minutes": given })).unwrap();
            assert_eq!(p.settings.sleep_after_minutes, expect, "{given}");
            assert_eq!(p.clamped, ["sleep_after_minutes"], "{given}");
        }
        let p = Settings::from_value(&json!({ "sleep_after_minutes": -5 })).unwrap();
        assert_eq!(p.settings.sleep_after_minutes, 5);
    }

    #[test]
    fn stale_temp_file_from_a_crash_does_not_break_the_save_or_survive_it() {
        let t = TempDir::new();
        let s1 = Settings { paused: true, ..Settings::default() };
        save_to(&t.file(), &s1).unwrap();
        let good = fs::read(t.file()).unwrap();
        // un .tmp a medias dejado por una caída: el original sigue íntegro y el siguiente guardado funciona
        fs::write(t.0.join(format!("{FILE_NAME}.tmp")), b"{ a medias").unwrap();
        assert_eq!(fs::read(t.file()).unwrap(), good);
        assert_eq!(load_from(&t.file()).0, s1);
        save_to(&t.file(), &Settings { speed: 200.0, ..s1.clone() }).unwrap();
        assert_eq!(names(&t), [FILE_NAME]);
        assert_eq!(load_from(&t.file()).0.speed, 200.0);
    }

    // ---------- set_setting (IPC) ----------

    #[test]
    fn set_setting_applies_persists_and_returns_the_applied_value() {
        let t = TempDir::new();
        let store = SettingsStore::load(t.file());
        let cases: [(&str, Value); 10] = [
            ("messages_enabled", json!(false)),
            ("message_interval_minutes", json!(30)),
            ("pet_size", json!(128)),
            ("speed", json!(300.0)),
            ("reduced_motion", json!(true)),
            ("dnd_auto_enabled", json!(false)),
            ("sleep_enabled", json!(false)),
            ("sleep_after_minutes", json!(30)),
            ("proximity_enabled", json!(false)),
            ("time_of_day_enabled", json!(false)),
        ];
        assert_eq!(cases.len(), SETTABLE_KEYS.len());
        for (k, v) in &cases {
            assert!(SETTABLE_KEYS.contains(k), "{k}");
            let (applied, changed) = store.set_setting(k, v).unwrap();
            assert!(changed, "{k}");
            assert_eq!(serde_json::to_value(&applied).unwrap()[*k], *v, "{k}: devuelve el valor aplicado");
        }
        // round-trip: lo escrito es exactamente lo devuelto y sobrevive a un reinicio
        let on_disk = load_from(&t.file()).0;
        assert_eq!(on_disk, store.get());
        assert_eq!(SettingsStore::load(t.file()).get(), on_disk);
        assert_eq!(on_disk.schema_version, 2);
    }

    #[test]
    fn set_setting_clamps_out_of_range_values_and_returns_what_was_applied() {
        let t = TempDir::new();
        let store = SettingsStore::load(t.file());
        let got = |k: &str, v: Value| serde_json::to_value(store.set_setting(k, &v).unwrap().0).unwrap()[k].clone();
        assert_eq!(got("speed", json!(1e9)), json!(600.0));
        assert_eq!(got("speed", json!(-3)), json!(60.0));
        assert_eq!(got("message_interval_minutes", json!(0)), json!(1));
        assert_eq!(got("message_interval_minutes", json!(99999)), json!(1440));
        assert_eq!(got("pet_size", json!(100)), json!(96));
        assert_eq!(got("pet_size", json!(-1)), json!(64));
        assert_eq!(got("sleep_after_minutes", json!(7)), json!(5));
        assert_eq!(got("sleep_after_minutes", json!(1e12)), json!(60));
        assert_eq!(got("sleep_after_minutes", json!(14.6)), json!(15));
        // nada fuera de rango llegó al disco
        let d = load_from(&t.file());
        assert_eq!(d.0, store.get());
        assert_eq!(d.1, LoadOutcome::Loaded { invalid: vec![], clamped: vec![] });
    }

    #[test]
    fn set_setting_rejects_unknown_keys_and_wrong_types_without_touching_anything() {
        let t = TempDir::new();
        let store = SettingsStore::load(t.file());
        let before = store.get();
        for k in ["", "paused", "follow_enabled", "schema_version", "hidden", "dnd_auto", "Speed", "pet_size ", "__proto__", "../x"] {
            let r = store.set_setting(k, &json!(true));
            assert_eq!(r, Err(SetError::UnknownKey(k.to_string())), "{k:?}");
        }
        for (k, bad) in [
            ("dnd_auto_enabled", json!("true")),
            ("sleep_enabled", json!(1)),
            ("proximity_enabled", json!(null)),
            ("time_of_day_enabled", json!([true])),
            ("messages_enabled", json!({"a": 1})),
            ("speed", json!("fast")),
            ("speed", json!(true)),
            ("pet_size", json!(null)),
            ("sleep_after_minutes", json!("10")),
            ("message_interval_minutes", json!([5])),
        ] {
            assert!(matches!(store.set_setting(k, &bad), Err(SetError::WrongType { .. })), "{k} = {bad}");
        }
        assert_eq!(store.get(), before);
        assert!(!t.file().exists(), "un rechazo no escribe nada");
    }

    #[test]
    fn set_setting_reports_no_change_and_does_not_write_for_the_same_value() {
        let t = TempDir::new();
        let store = SettingsStore::load(t.file());
        let (_, changed) = store.set_setting("dnd_auto_enabled", &json!(true)).unwrap(); // ya era true
        assert!(!changed);
        assert!(!t.file().exists());
        assert!(store.set_setting("dnd_auto_enabled", &json!(false)).unwrap().1);
        let m1 = fs::metadata(t.file()).unwrap().modified().unwrap();
        assert!(!store.set_setting("dnd_auto_enabled", &json!(false)).unwrap().1);
        assert_eq!(fs::metadata(t.file()).unwrap().modified().unwrap(), m1);
    }

    #[test]
    fn set_setting_on_a_v1_file_makes_the_backup_first() {
        let t = TempDir::new();
        fs::write(t.file(), V1_FILE).unwrap();
        let store = SettingsStore::load(t.file());
        store.set_setting("sleep_enabled", &json!(false)).unwrap();
        assert_eq!(fs::read(t.0.join(V1_BACKUP_NAME)).unwrap(), V1_FILE.as_bytes());
        let s = load_from(&t.file()).0;
        assert!(!s.sleep_enabled && s.pet_size == 128 && s.paused);
    }

    #[test]
    fn settable_keys_are_real_fields_and_exclude_runtime_state() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        for k in SETTABLE_KEYS {
            assert!(v.get(k).is_some(), "{k} no es un campo de Settings");
        }
        for k in ["paused", "follow_enabled", "schema_version"] {
            assert!(!SETTABLE_KEYS.contains(&k), "{k} no debe poder cambiarse por IPC");
        }
    }

    // ---------- compatibilidad hacia atrás: copia CONGELADA del lector de v0.1.0 ----------
    // Reproduce `from_value`/`load_from`/guardado de v0.1.0 (esquema 1) para comprobar qué haría esa
    // versión ante un archivo de esquema 2. NO se actualiza con el código nuevo: es la referencia.
    mod v010 {
        use super::*;

        #[derive(Debug, Clone, PartialEq, serde::Serialize)]
        pub struct Settings {
            pub schema_version: u32,
            pub follow_enabled: bool,
            pub messages_enabled: bool,
            pub message_interval_minutes: u32,
            pub pet_size: u32,
            pub speed: f64,
            pub reduced_motion: bool,
            pub paused: bool,
        }

        pub fn from_value(v: &Value) -> Option<(Settings, Option<u32>)> {
            let obj = v.as_object()?;
            let b = |k: &str, d: bool| obj.get(k).and_then(Value::as_bool).unwrap_or(d);
            let n = |k: &str| obj.get(k).and_then(Value::as_f64).filter(|x| x.is_finite());
            let size = n("pet_size").map_or(96, |x| {
                let x = x.round().clamp(0.0, 10_000.0) as u32;
                *[64u32, 96, 128].iter().min_by_key(|s| s.abs_diff(x)).unwrap()
            });
            let future = n("schema_version").and_then(|x| ((x.max(0.0) as u32) > 1).then_some(x.max(0.0) as u32));
            Some((
                Settings {
                    schema_version: 1,
                    follow_enabled: b("follow_enabled", true),
                    messages_enabled: b("messages_enabled", true),
                    message_interval_minutes: n("message_interval_minutes").map_or(12, |x| x.round().clamp(1.0, 1440.0) as u32),
                    pet_size: size,
                    speed: n("speed").map_or(260.0, |x| x.clamp(60.0, 600.0)),
                    reduced_motion: b("reduced_motion", false),
                    paused: b("paused", false),
                },
                future,
            ))
        }

        /// `load_from` de v0.1.0: si el esquema es futuro, copia a `settings.v<N>.bak.json` (SOBRESCRIBE).
        pub fn load(path: &Path) -> (Settings, Option<u32>, Option<PathBuf>) {
            let v: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            let (s, future) = from_value(&v).unwrap();
            let backup = future.map(|n| {
                let dest = path.parent().unwrap().join(format!("settings.v{n}.bak.json"));
                fs::copy(path, &dest).unwrap();
                dest
            });
            (s, future, backup)
        }

        /// Guardado de v0.1.0 (p. ej. `mirror_runtime`): reescribe el archivo con SU esquema.
        pub fn save(path: &Path, s: &Settings) {
            fs::write(path, serde_json::to_vec_pretty(s).unwrap()).unwrap();
        }
    }

    #[test]
    fn what_v0_1_0_does_with_a_schema_2_file_is_documented_by_a_frozen_reader() {
        let t = TempDir::new();
        let v2 = Settings {
            paused: true,
            pet_size: 128,
            dnd_auto_enabled: false,
            sleep_enabled: false,
            sleep_after_minutes: 60,
            proximity_enabled: false,
            time_of_day_enabled: false,
            ..Settings::default()
        };
        save_to(&t.file(), &v2).unwrap();
        let original = fs::read(t.file()).unwrap();

        // (1) v0.1.0 detecta esquema futuro (2), carga lo que conoce y copia el archivo a settings.v2.bak.json
        let (old, future, backup) = v010::load(&t.file());
        assert_eq!(future, Some(2));
        assert!(old.paused && old.pet_size == 128, "los campos comunes sobreviven");
        let backup = backup.unwrap();
        assert_eq!(backup.file_name().unwrap(), "settings.v2.bak.json");
        assert_eq!(fs::read(&backup).unwrap(), original, "la configuración nueva queda en esa copia");

        // (2) su siguiente guardado reescribe el archivo como esquema 1 y SIN los campos nuevos
        v010::save(&t.file(), &old);
        let after: Value = serde_json::from_slice(&fs::read(t.file()).unwrap()).unwrap();
        assert_eq!(after["schema_version"], 1);
        for k in ["dnd_auto_enabled", "sleep_enabled", "sleep_after_minutes", "proximity_enabled", "time_of_day_enabled"] {
            assert!(after.get(k).is_none(), "{k} se pierde del archivo principal");
        }

        // (3) al volver a v0.2.0: migra de nuevo y los interruptores regresan a sus defectos (activados)
        let (back, outcome) = load_from(&t.file());
        assert!(matches!(outcome, LoadOutcome::Migrated { .. }), "{outcome:?}");
        assert!(back.paused && back.pet_size == 128, "lo común se conserva");
        assert!(back.dnd_auto_enabled && back.sleep_enabled && back.proximity_enabled && back.time_of_day_enabled);
        assert_eq!(back.sleep_after_minutes, 10);
        // y la configuración apagada sigue recuperable a mano en settings.v2.bak.json
        assert!(!load_from(&backup).0.dnd_auto_enabled);
    }

    #[test]
    fn the_frozen_v0_1_0_reader_agrees_with_the_current_parser_on_a_v1_file() {
        // control de la copia congelada: sobre un archivo de v0.1.0 ambos lectores coinciden en lo común
        let v: Value = serde_json::from_str(V1_FILE).unwrap();
        let (old, future) = v010::from_value(&v).unwrap();
        let cur = Settings::from_value(&v).unwrap().settings;
        assert_eq!(future, None);
        assert_eq!(
            (old.follow_enabled, old.messages_enabled, old.message_interval_minutes, old.pet_size, old.speed, old.reduced_motion, old.paused),
            (cur.follow_enabled, cur.messages_enabled, cur.message_interval_minutes, cur.pet_size, cur.speed, cur.reduced_motion, cur.paused)
        );
    }
}
