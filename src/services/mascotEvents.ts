import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { MascotView } from "@/types/view";

export const VIEW_EVENT = "mascot://view";

/** Descarta cualquier vista que no sea estrictamente más reciente que la actual. */
export function reduceView(current: MascotView | null, next: MascotView): MascotView | null {
  return current === null || next.version > current.version ? next : current;
}

/**
 * Suscripción fiable: primero se registra el listener (no se pierden cambios posteriores) y
 * DESPUÉS se pide el snapshot actual. Ambos pasan por `reduceView`, así que el orden de llegada
 * no importa: gana siempre la versión más alta.
 * Devuelve una función de limpieza que elimina el listener (también si se llama antes de
 * que termine la suscripción).
 */
export function subscribeView(onView: (v: MascotView) => void): () => void {
  let disposed = false;
  let unlisten: UnlistenFn | undefined;
  let latest: MascotView | null = null;

  const accept = (v: MascotView) => {
    if (disposed) return;
    const next = reduceView(latest, v);
    if (next !== latest && next) {
      latest = next;
      onView(next);
    }
  };

  void (async () => {
    try {
      const un = await listen<MascotView>(VIEW_EVENT, (e) => accept(e.payload));
      if (disposed) {
        un();
        return;
      }
      unlisten = un;
      accept(await invoke<MascotView>("get_view"));
    } catch (err) {
      console.error("[miel] no se pudo sincronizar el estado inicial:", err);
    }
  })();

  return () => {
    disposed = true;
    unlisten?.();
  };
}
