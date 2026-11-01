// Celda de inspección visual (solo desarrollo, fuera del build). Los ids del rig son únicos por
// documento, por eso index.html incrusta una celda por <iframe> (estado × tema).
import { createRoot } from "react-dom/client";
import { Bee } from "@/components/Bee/Bee";
import { BEE_STATES, type BeeState } from "@/types/view";

const params = new URLSearchParams(location.search);
const q = params.get("state") as BeeState | null;
const state: BeeState = q && BEE_STATES.includes(q) ? q : "idle";
const theme = params.get("theme") === "dark" ? "dark" : "light";

createRoot(document.getElementById("root")!).render(
  <section className={theme}>
    <div className="row">
      {[64, 96, 128].map((n) => (
        <div className="cell" key={n}>
          <Bee state={state} size={n} />
          <span>{state} · {n}px</span>
        </div>
      ))}
    </div>
  </section>,
);
