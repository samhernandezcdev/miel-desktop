// Inspección visual de la burbuja (solo desarrollo, fuera del build). Una escena por documento.
import { createRoot } from "react-dom/client";
import { MascotStage } from "@/components/MascotStage/MascotStage";
import { BEE_STATES, type BeeState } from "@/types/view";
import "@/styles/app.css";

const q = new URLSearchParams(location.search);
const state = (BEE_STATES as readonly string[]).includes(q.get("state") ?? "") ? (q.get("state") as BeeState) : "idle";
const text = q.get("text");
const size = Number(q.get("size") ?? 96);
document.body.className = q.get("theme") === "dark" ? "dark" : "light";
// app.css fija pointer-events:none y fondo transparente: aquí el fondo lo pone el body
createRoot(document.getElementById("root")!).render(
  <>
    <div className="frame">
      <MascotStage
        size={size}
        view={{ version: 1, behavior: "idle", expression: state, paused: false, hidden: false, message: text ? { id: "demo", text } : null }}
      />
    </div>
    <div className="cap">{state} · {size}px{text ? ` · ${text.length} car.` : " · sin mensaje"}</div>
  </>,
);
