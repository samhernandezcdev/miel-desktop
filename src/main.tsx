import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { installGlobalErrorReporting } from "./services/errorReport";

installGlobalErrorReporting();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
