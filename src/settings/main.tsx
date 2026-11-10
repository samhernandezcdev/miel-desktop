import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import SettingsApp from "./SettingsApp";
import "./settings.css";

// Ventana de ajustes (E3): segunda entrada de build. No instala el informe de errores de la mascota
// (su comando `report_frontend_error` es solo de la ventana `main`; esta ventana no tiene ese permiso).
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SettingsApp />
  </StrictMode>,
);
