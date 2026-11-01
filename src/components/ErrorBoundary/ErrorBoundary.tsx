import { Component, type ErrorInfo, type ReactNode } from "react";
import { reportError } from "@/services/errorReport";

/** Reintentos tras un fallo (cada uno ocurre cuando llega una vista nueva de Rust). */
export const MAX_RETRIES = 3;

interface Props {
  children: ReactNode;
  /** Cambia cuando Rust publica un estado nuevo: habilita un reintento tras un fallo. */
  resetKey?: unknown;
  /** Para pruebas: sustituye al informe real. */
  onReport?: (kind: string, message: string) => void;
}

interface State {
  failed: boolean;
  retries: number;
}

/**
 * Captura errores de RENDERIZADO de sus hijos. Mientras falla no pinta nada (la ventana es
 * transparente: no queda ningún rectángulo ni mensaje) y la bandeja nativa sigue operativa, ya que
 * vive en Rust. Reintenta como máximo `MAX_RETRIES` veces, cuando llega una vista nueva; después se
 * queda en blanco para no entrar en un bucle de errores visuales.
 *
 * LÍMITES (ver docs/ARCHITECTURE.md): no captura errores en manejadores de eventos, código
 * asíncrono, promesas rechazadas ni fallos del proceso de WebView2; los dos primeros se informan
 * con `installGlobalErrorReporting`; un fallo de WebView2 solo lo cubre la bandeja ("Cerrar Miel").
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { failed: false, retries: 0 };

  static getDerivedStateFromError(): Partial<State> {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Solo nombre, mensaje y las primeras líneas de la pila de componentes: sin datos de la persona.
    const frames = (info.componentStack ?? "").split("\n").slice(0, 4).join(" ");
    const report = this.props.onReport ?? ((k, m) => void reportError(k, m));
    report("render", `${error.name}: ${error.message} | ${frames}`);
  }

  componentDidUpdate(prev: Props): void {
    if (this.state.failed && prev.resetKey !== this.props.resetKey && this.state.retries < MAX_RETRIES) {
      this.setState((s) => ({ failed: false, retries: s.retries + 1 }));
    }
  }

  render(): ReactNode {
    return this.state.failed ? null : this.props.children;
  }
}
