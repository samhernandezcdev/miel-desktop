/**
 * Textos de la ventana de ajustes (D9): español e inglés en UN solo módulo.
 *
 * El proyecto no tenía antes un mecanismo de idioma (la mascota y la bandeja están solo en español),
 * así que este módulo es el mecanismo: `pickLanguage` elige el primer idioma de la persona que Miel
 * soporta (es/en) y, si ninguno lo es, español.
 *
 * Cuidado con la veracidad: todo lo que dice la sección de privacidad debe ser cierto HOY (v0.1.0 +
 * ajustes de E2/E3). Cada entrega posterior (No molestar, sueño, proximidad, franjas) añade su propio
 * texto cuando su función exista.
 */

export type Lang = "es" | "en";
export const LANGS: readonly Lang[] = ["es", "en"];
export const DEFAULT_LANG: Lang = "es";

/** Primer idioma de la lista (p. ej. `navigator.languages`) que Miel soporta; si no hay, español. */
export function pickLanguage(languages: readonly string[] | undefined): Lang {
  for (const raw of languages ?? []) {
    const primary = String(raw).toLowerCase().split(/[-_]/)[0];
    if (primary === "es" || primary === "en") return primary;
  }
  return DEFAULT_LANG;
}

export interface Strings {
  windowTitle: string;
  heading: string;
  intro: string;
  restartBanner: string;
  trayNote: string;
  loading: string;
  loadError: string;
  retry: string;
  appliesOnRestart: string;
  sections: { appearance: string; movement: string; messages: string; privacy: string };
  petSize: { label: string; hint: string; options: Record<"64" | "96" | "128", string> };
  reducedMotion: { label: string; hint: string };
  speed: { label: string; hint: string; unit: string };
  messagesEnabled: { label: string; hint: string };
  messageInterval: { label: string; hint: string; unit: string };
  /** `{name}` = nombre del ajuste; `{value}` = valor aplicado. */
  rejected: string;
  adjusted: string;
  privacy: { items: string[]; storageNote: string };
}

export const STRINGS: Record<Lang, Strings> = {
  es: {
    windowTitle: "Miel · Ajustes",
    heading: "Ajustes de Miel",
    intro: "Los cambios se guardan al instante.",
    restartBanner:
      "Estos ajustes se aplican la próxima vez que inicies Miel: ciérrala desde el menú de la bandeja («Cerrar Miel») y ábrela de nuevo.",
    trayNote: "«Seguir cursor» y «Pausar» no están aquí: se cambian desde el menú de la bandeja y actúan al momento.",
    loading: "Cargando ajustes…",
    loadError: "No se pudieron leer los ajustes.",
    retry: "Reintentar",
    appliesOnRestart: "Se aplica al reiniciar Miel.",
    sections: { appearance: "Apariencia", movement: "Movimiento", messages: "Mensajes", privacy: "Privacidad" },
    petSize: {
      label: "Tamaño de Miel",
      hint: "Tamaño de la abeja en la pantalla.",
      options: { "64": "Pequeña (64 px)", "96": "Mediana (96 px)", "128": "Grande (128 px)" },
    },
    reducedMotion: {
      label: "Movimiento reducido",
      hint: "Hace las animaciones más tranquilas, además de la preferencia de tu sistema.",
    },
    speed: { label: "Velocidad", hint: "Velocidad máxima al moverse; entre 60 y 600 píxeles por segundo.", unit: "px/s" },
    messagesEnabled: { label: "Mostrar mensajes", hint: "Miel dice de vez en cuando una frase corta en una burbuja." },
    messageInterval: {
      label: "Tiempo entre mensajes",
      hint: "Mínimo entre un mensaje y el siguiente; entre 1 y 1440 minutos.",
      unit: "min",
    },
    rejected: "No se pudo cambiar «{name}»: el valor no es válido. Se restauró el valor actual.",
    adjusted: "«{name}» se ajustó a {value}, el valor permitido más cercano.",
    privacy: {
      items: [
        "Miel funciona sin conexión: no usa red, cuentas, inteligencia artificial ni telemetría, y no envía nada a ningún sitio.",
        "No lee lo que escribes, no hace capturas de pantalla y no lee los títulos ni el contenido de tus ventanas o de tus archivos.",
        "Hoy solo consulta la posición del cursor (únicamente cuando «Seguir cursor» está activado) y los monitores, el área de trabajo y la escala de pantalla para colocarse.",
        "No guarda historial de lo que haces. Lo único que guarda son estas preferencias.",
        "WebView2, el componente de Microsoft que dibuja la ventana, tiene su propio comportamiento, fuera del control de Miel.",
      ],
      storageNote: "Las preferencias están en un archivo local, settings.json, en la carpeta lat.bkode.miel de tus datos de aplicación (%APPDATA%).",
    },
  },
  en: {
    windowTitle: "Miel · Settings",
    heading: "Miel settings",
    intro: "Changes are saved instantly.",
    restartBanner:
      "These settings take effect the next time you start Miel: close it from the tray menu (“Cerrar Miel”) and open it again.",
    trayNote: "“Seguir cursor” (follow cursor) and “Pausar” (pause) are not here: change them from the tray menu; they take effect immediately.",
    loading: "Loading settings…",
    loadError: "The settings could not be read.",
    retry: "Try again",
    appliesOnRestart: "Applies when Miel restarts.",
    sections: { appearance: "Appearance", movement: "Movement", messages: "Messages", privacy: "Privacy" },
    petSize: {
      label: "Miel size",
      hint: "Size of the bee on your screen.",
      options: { "64": "Small (64 px)", "96": "Medium (96 px)", "128": "Large (128 px)" },
    },
    reducedMotion: {
      label: "Reduced motion",
      hint: "Makes animations calmer, in addition to your system preference.",
    },
    speed: { label: "Speed", hint: "Top speed when moving; between 60 and 600 pixels per second.", unit: "px/s" },
    messagesEnabled: { label: "Show messages", hint: "Now and then Miel says a short phrase in a bubble." },
    messageInterval: {
      label: "Time between messages",
      hint: "Minimum time between one message and the next; between 1 and 1440 minutes.",
      unit: "min",
    },
    rejected: "“{name}” could not be changed: the value is not valid. The current value was restored.",
    adjusted: "“{name}” was adjusted to {value}, the nearest allowed value.",
    privacy: {
      items: [
        "Miel works offline: it uses no network, accounts, artificial intelligence or telemetry, and sends nothing anywhere.",
        "It does not read what you type, take screenshots, or read the titles or contents of your windows or files.",
        "Today it only queries the cursor position (only when “Seguir cursor”, follow cursor, is on) and the monitors, work area and display scale to place itself.",
        "It keeps no history of what you do. The only thing it stores is these preferences.",
        "WebView2, the Microsoft component that draws the window, has its own behaviour, outside Miel's control.",
      ],
      storageNote: "Preferences are in a local file, settings.json, in the lat.bkode.miel folder of your application data (%APPDATA%).",
    },
  },
};

/** Sustituye `{clave}` por su valor (sin evaluar nada: solo texto). */
export function fill(template: string, values: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (m, k: string) => (k in values ? String(values[k]) : m));
}
