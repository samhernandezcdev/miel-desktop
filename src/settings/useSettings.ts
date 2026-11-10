import { useCallback, useEffect, useRef, useState } from "react";
import type { EditableKey, SettingsApi, SettingsData } from "./api";

/** Segundos que permanece un aviso en pantalla. */
export const NOTICE_MS = 6000;

export type Notice =
  | { kind: "rejected"; key: EditableKey }
  | { kind: "adjusted"; key: EditableKey; applied: unknown };

export interface UseSettings {
  /** Último estado REAL conocido (de Rust); `null` mientras carga. */
  settings: SettingsData | null;
  loadError: boolean;
  notice: Notice | null;
  /**
   * Pide a Rust cambiar un ajuste. Resuelve `true` si Rust lo aplicó. Si lo rechaza, muestra un aviso,
   * vuelve a leer el valor real y resuelve `false` (el control debe mostrar `settings`).
   */
  commit(key: EditableKey, value: unknown): Promise<boolean>;
  reload(): void;
}

/**
 * Lógica de la ventana: carga con `get`, escucha el evento de cambio y escribe con `set`.
 * El listener se registra ANTES de leer (no se pierde ningún cambio); si llegó un evento antes de que
 * volviera la lectura inicial, la lectura (posiblemente más antigua) se descarta.
 */
export function useSettings(api: SettingsApi): UseSettings {
  const [settings, setSettings] = useState<SettingsData | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [attempt, setAttempt] = useState(0);
  const alive = useRef(true);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const showNotice = useCallback((n: Notice) => {
    setNotice(n);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setNotice(null), NOTICE_MS);
  }, []);

  useEffect(() => {
    alive.current = true;
    let sawEvent = false;
    let unlisten: (() => void) | undefined;
    setLoadError(false);
    void (async () => {
      try {
        const un = await api.onChanged((s) => {
          if (!alive.current) return;
          sawEvent = true;
          setSettings(s);
        });
        if (!alive.current) {
          un();
          return;
        }
        unlisten = un;
        const first = await api.get();
        if (alive.current && !sawEvent) setSettings(first);
      } catch {
        if (alive.current) setLoadError(true);
      }
    })();
    return () => {
      alive.current = false;
      unlisten?.();
      clearTimeout(timer.current);
    };
  }, [api, attempt]);

  const commit = useCallback(
    async (key: EditableKey, value: unknown): Promise<boolean> => {
      try {
        const applied = await api.set(key, value);
        if (!alive.current) return true;
        setSettings(applied);
        if (applied[key] !== value) showNotice({ kind: "adjusted", key, applied: applied[key] });
        else setNotice(null);
        return true;
      } catch {
        if (!alive.current) return false;
        showNotice({ kind: "rejected", key });
        try {
          const real = await api.get(); // restaura el control al valor real (el de Rust)
          if (alive.current) setSettings(real);
        } catch {
          /* se conserva el último estado real conocido */
        }
        return false;
      }
    },
    [api, showNotice],
  );

  const reload = useCallback(() => setAttempt((n) => n + 1), []);
  return { settings, loadError, notice, commit, reload };
}
