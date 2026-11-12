import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Evento de Rust con las preferencias completas tras un cambio real (`settings::EVENT_CHANGED`). */
export const CHANGED_EVENT = "settings://changed";

/** Claves que esta ventana puede cambiar (subconjunto de la lista cerrada de `set_setting` en Rust). */
export type EditableKey =
  | "messages_enabled"
  | "message_interval_minutes"
  | "pet_size"
  | "speed"
  | "reduced_motion"
  | "dnd_auto_enabled";

/** Preferencias tal como las devuelve Rust (snake_case). Solo se declaran las que esta ventana muestra. */
export interface SettingsData {
  messages_enabled: boolean;
  message_interval_minutes: number;
  pet_size: number;
  speed: number;
  reduced_motion: boolean;
  dnd_auto_enabled: boolean;
  [other: string]: unknown;
}

/**
 * Frontera con Rust. Rust es la única fuente de verdad: aquí no se valida ni se corrige nada.
 * `set` rechaza (promesa rechazada) cuando Rust rechaza la clave o el valor.
 */
export interface SettingsApi {
  get(): Promise<SettingsData>;
  set(key: EditableKey, value: unknown): Promise<SettingsData>;
  /** Registra el listener del evento de cambio; resuelve con la función que lo elimina. */
  onChanged(cb: (s: SettingsData) => void): Promise<() => void>;
}

export const tauriApi: SettingsApi = {
  get: () => invoke<SettingsData>("get_settings"),
  set: (key, value) => invoke<SettingsData>("set_setting", { key, value }),
  onChanged: (cb) => listen<SettingsData>(CHANGED_EVENT, (e) => cb(e.payload)),
};
