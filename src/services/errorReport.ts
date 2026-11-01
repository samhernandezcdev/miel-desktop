import { invoke } from "@tauri-apps/api/core";

/** Recorta y aplana: nunca se envía más de lo necesario (Rust vuelve a sanear y limitar). */
export function compact(text: string, max = 400): string {
  return text.replace(/\s+/g, " ").trim().slice(0, max);
}

/**
 * Informa de un error al registro local de Rust (archivo acotado, sin red). Nunca lanza:
 * un fallo al informar no debe provocar otro fallo.
 */
export async function reportError(kind: string, message: string): Promise<void> {
  try {
    await invoke("report_frontend_error", { kind: compact(kind, 60), message: compact(message) });
  } catch {
    // sin canal de informe: se ignora a propósito (documentado en docs/ARCHITECTURE.md)
  }
}

/**
 * Errores que un Error Boundary NO captura: excepciones fuera del renderizado y promesas
 * rechazadas. Solo se informan (no se puede "recuperar" una promesa ajena). Devuelve la limpieza.
 */
export function installGlobalErrorReporting(): () => void {
  const onError = (e: ErrorEvent) => void reportError("error", `${e.message} @ ${e.lineno}:${e.colno}`);
  const onRejection = (e: PromiseRejectionEvent) => {
    const r: unknown = e.reason;
    void reportError("unhandledrejection", r instanceof Error ? `${r.name}: ${r.message}` : String(r));
  };
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
  };
}
