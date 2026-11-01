import { invoke } from "@tauri-apps/api/core";
import { sanitizeSize, type PetSize } from "@/components/MascotStage/layout";

export interface DisplayPrefs {
  petSize: PetSize;
  reducedMotion: boolean;
}

export const DEFAULT_PREFS: DisplayPrefs = { petSize: 96, reducedMotion: false };

/** Preferencias de presentación decididas por Rust. Ante cualquier fallo, los valores estables por defecto. */
export async function loadDisplayPrefs(): Promise<DisplayPrefs> {
  try {
    const raw = await invoke<{ petSize?: unknown; reducedMotion?: unknown }>("get_display_prefs");
    return { petSize: sanitizeSize(raw?.petSize), reducedMotion: raw?.reducedMotion === true };
  } catch {
    return DEFAULT_PREFS;
  }
}
