import { useEffect, useState } from "react";
import { ErrorBoundary } from "@/components/ErrorBoundary/ErrorBoundary";
import { MascotStage } from "@/components/MascotStage/MascotStage";
import { DEFAULT_PREFS, loadDisplayPrefs, type DisplayPrefs } from "@/services/displayPrefs";
import { subscribeView } from "@/services/mascotEvents";
import type { MascotView } from "@/types/view";
import "@/styles/app.css";

/** Antes de recibir el primer estado de Rust se muestra la expresión neutra. */
const INITIAL: MascotView = {
  version: -1,
  behavior: "idle",
  expression: "idle",
  paused: false,
  hidden: false,
  followEnabled: true,
  message: null,
};

export default function App() {
  const [view, setView] = useState<MascotView>(INITIAL);
  const [prefs, setPrefs] = useState<DisplayPrefs>(DEFAULT_PREFS);

  // La suscripción vive FUERA del Error Boundary: si la escena falla, los eventos siguen
  // llegando y permiten reintentar el renderizado con la vista siguiente.
  useEffect(() => subscribeView(setView), []);
  useEffect(() => {
    let alive = true;
    void loadDisplayPrefs().then((p) => alive && setPrefs(p));
    return () => {
      alive = false;
    };
  }, []);

  return (
    <ErrorBoundary resetKey={view.version}>
      <MascotStage view={view} size={prefs.petSize} reducedMotion={prefs.reducedMotion} />
    </ErrorBoundary>
  );
}
