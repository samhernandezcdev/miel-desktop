import { useEffect, useId, useState, type ReactNode } from "react";
import { tauriApi, type EditableKey, type SettingsApi } from "./api";
import { fill, pickLanguage, STRINGS, type Lang, type Strings } from "./strings";
import { useSettings, type Notice } from "./useSettings";

interface Props {
  /** Para pruebas: sustituye a la frontera real con Rust. */
  api?: SettingsApi;
  /** Para pruebas: fuerza el idioma (si no, el de la persona). */
  lang?: Lang;
}

function noticeText(n: Notice, t: Strings): string {
  const name = {
    messages_enabled: t.messagesEnabled.label,
    message_interval_minutes: t.messageInterval.label,
    pet_size: t.petSize.label,
    speed: t.speed.label,
    reduced_motion: t.reducedMotion.label,
    dnd_auto_enabled: t.dnd.label,
    sleep_enabled: t.sleep.label,
    sleep_after_minutes: t.sleep.after.label,
  }[n.key];
  return n.kind === "rejected" ? fill(t.rejected, { name }) : fill(t.adjusted, { name, value: String(n.applied) });
}

/** Campo: etiqueta, control, ayuda y la nota de "se aplica al reiniciar", asociados con ids. */
function Field(props: { id: string; label: string; hint: string; restart: string; children: (describedBy: string) => ReactNode; inline?: boolean }) {
  const { id, label, hint, restart, children, inline } = props;
  const describedBy = `${id}-hint ${id}-restart`;
  return (
    <div className={inline ? "field field-inline" : "field"}>
      <div className="field-head">
        {inline ? (
          <label htmlFor={id} className="field-label">
            {label}
          </label>
        ) : (
          <span className="field-label" id={`${id}-label`}>
            {label}
          </span>
        )}
        {children(describedBy)}
      </div>
      <p className="hint" id={`${id}-hint`}>
        {hint}
      </p>
      <p className="hint restart" id={`${id}-restart`}>
        {restart}
      </p>
    </div>
  );
}

/** Interruptor: casilla nativa con rol `switch` (teclado y lector de pantalla gratis). */
function Switch(props: { id: string; label: string; hint: string; restart: string; checked: boolean; onChange: (v: boolean) => void }) {
  const { id, checked, onChange } = props;
  return (
    <Field id={id} label={props.label} hint={props.hint} restart={props.restart} inline>
      {(describedBy) => (
        <input id={id} type="checkbox" role="switch" className="switch" checked={checked} aria-describedby={describedBy} onChange={(e) => onChange(e.target.checked)} />
      )}
    </Field>
  );
}

/** Selector de una lista cerrada de valores numéricos (el valor que se envía es el número, tal cual; Rust valida). */
function SelectField(props: {
  id: string;
  label: string;
  hint: string;
  restart: string;
  value: number;
  options: { value: number; text: string }[];
  onChange: (v: number) => void;
}) {
  const { id, value, options, onChange } = props;
  return (
    <Field id={id} label={props.label} hint={props.hint} restart={props.restart} inline>
      {(describedBy) => (
        <select id={id} className="select" value={String(value)} aria-describedby={describedBy} onChange={(e) => onChange(Number(e.target.value))}>
          {options.map((o) => (
            <option key={o.value} value={String(o.value)}>
              {o.text}
            </option>
          ))}
        </select>
      )}
    </Field>
  );
}

/** Valores permitidos del tiempo de sueño (los mismos que acepta Rust). */
export const SLEEP_MINUTES = [5, 10, 15, 30, 60] as const;

/**
 * Campo numérico. El texto se envía TAL CUAL lo escribió la persona (como número si lo es); si no es
 * un número se envía el texto y es Rust quien lo rechaza. Nada se corrige aquí.
 */
function NumberField(props: {
  id: string;
  label: string;
  hint: string;
  restart: string;
  unit: string;
  value: number;
  step?: string;
  onCommit: (raw: unknown) => Promise<boolean>;
}) {
  const { id, value, unit, onCommit } = props;
  const [draft, setDraft] = useState<string | null>(null);
  const shown = draft ?? String(value);
  const commit = async () => {
    if (draft === null || draft === String(value)) {
      setDraft(null);
      return;
    }
    const text = draft.trim();
    const asNumber = Number(text);
    await onCommit(text !== "" && Number.isFinite(asNumber) ? asNumber : draft);
    setDraft(null); // vuelve a mostrar el valor real (el aplicado, o el de siempre si Rust lo rechazó)
  };
  return (
    <Field id={id} label={props.label} hint={props.hint} restart={props.restart} inline>
      {(describedBy) => (
        <span className="number-wrap">
          <input
            id={id}
            type="number"
            inputMode="decimal"
            step={props.step ?? "1"}
            className="number"
            value={shown}
            aria-describedby={describedBy}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={() => void commit()}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void commit();
              } else if (e.key === "Escape") {
                setDraft(null);
              }
            }}
          />
          <span className="unit" aria-hidden="true">
            {unit}
          </span>
        </span>
      )}
    </Field>
  );
}

export default function SettingsApp({ api = tauriApi, lang }: Props) {
  const language: Lang = lang ?? pickLanguage(typeof navigator === "undefined" ? undefined : navigator.languages);
  const t = STRINGS[language];
  const { settings, loadError, notice, commit, reload } = useSettings(api);
  const uid = useId();

  useEffect(() => {
    document.documentElement.lang = language;
    document.title = t.windowTitle;
  }, [language, t.windowTitle]);

  const set = (key: EditableKey) => (v: unknown) => commit(key, v);

  return (
    <main className="page" aria-labelledby={`${uid}-h1`}>
      <h1 id={`${uid}-h1`}>{t.heading}</h1>
      <p className="intro">{t.intro}</p>
      <p className="banner">{t.restartBanner}</p>
      <p className="muted">{t.trayNote}</p>

      {/* región viva: los avisos se anuncian sin mover el foco */}
      <div className="notice-slot" role="status" aria-live="polite">
        {notice ? <p className="notice">{noticeText(notice, t)}</p> : null}
      </div>

      {settings === null ? (
        loadError ? (
          <div className="state" role="alert">
            <p>{t.loadError}</p>
            <button type="button" className="button" onClick={reload}>
              {t.retry}
            </button>
          </div>
        ) : (
          <p className="state" role="status">
            {t.loading}
          </p>
        )
      ) : (
        <form className="sections" onSubmit={(e) => e.preventDefault()} aria-label={t.heading}>
          <section aria-labelledby={`${uid}-appearance`}>
            <h2 id={`${uid}-appearance`}>{t.sections.appearance}</h2>
            <Field id={`${uid}-size`} label={t.petSize.label} hint={t.petSize.hint} restart={t.appliesOnRestart}>
              {(describedBy) => (
                <div role="radiogroup" aria-labelledby={`${uid}-size-label`} aria-describedby={describedBy} className="radios">
                  {(["64", "96", "128"] as const).map((v) => (
                    <label key={v} className="radio">
                      <input
                        type="radio"
                        name={`${uid}-size`}
                        value={v}
                        checked={String(settings.pet_size) === v}
                        onChange={() => void set("pet_size")(Number(v))}
                      />
                      <span>{t.petSize.options[v]}</span>
                    </label>
                  ))}
                </div>
              )}
            </Field>
            <Switch
              id={`${uid}-motion`}
              label={t.reducedMotion.label}
              hint={t.reducedMotion.hint}
              restart={t.appliesOnRestart}
              checked={settings.reduced_motion}
              onChange={(v) => void set("reduced_motion")(v)}
            />
          </section>

          <section aria-labelledby={`${uid}-movement`}>
            <h2 id={`${uid}-movement`}>{t.sections.movement}</h2>
            <NumberField
              id={`${uid}-speed`}
              label={t.speed.label}
              hint={t.speed.hint}
              restart={t.appliesOnRestart}
              unit={t.speed.unit}
              value={settings.speed}
              step="any"
              onCommit={set("speed")}
            />
          </section>

          <section aria-labelledby={`${uid}-messages`}>
            <h2 id={`${uid}-messages`}>{t.sections.messages}</h2>
            <Switch
              id={`${uid}-msg`}
              label={t.messagesEnabled.label}
              hint={t.messagesEnabled.hint}
              restart={t.appliesOnRestart}
              checked={settings.messages_enabled}
              onChange={(v) => void set("messages_enabled")(v)}
            />
            <NumberField
              id={`${uid}-interval`}
              label={t.messageInterval.label}
              hint={t.messageInterval.hint}
              restart={t.appliesOnRestart}
              unit={t.messageInterval.unit}
              value={settings.message_interval_minutes}
              onCommit={set("message_interval_minutes")}
            />
          </section>

          <section aria-labelledby={`${uid}-dnd`}>
            <h2 id={`${uid}-dnd`}>{t.sections.dnd}</h2>
            <Switch
              id={`${uid}-dnd-auto`}
              label={t.dnd.label}
              hint={t.dnd.hint}
              restart={t.appliesNow}
              checked={settings.dnd_auto_enabled}
              onChange={(v) => void set("dnd_auto_enabled")(v)}
            />
          </section>

          <section aria-labelledby={`${uid}-sleep`}>
            <h2 id={`${uid}-sleep`}>{t.sections.sleep}</h2>
            <Switch
              id={`${uid}-sleep-on`}
              label={t.sleep.label}
              hint={t.sleep.hint}
              restart={t.appliesNow}
              checked={settings.sleep_enabled}
              onChange={(v) => void set("sleep_enabled")(v)}
            />
            <SelectField
              id={`${uid}-sleep-after`}
              label={t.sleep.after.label}
              hint={t.sleep.after.hint}
              restart={t.appliesNow}
              value={settings.sleep_after_minutes}
              options={SLEEP_MINUTES.map((n) => ({ value: n, text: fill(t.sleep.after.option, { n }) }))}
              onChange={(v) => void set("sleep_after_minutes")(v)}
            />
          </section>
        </form>
      )}

      <section aria-labelledby={`${uid}-privacy`} className="privacy">
        <h2 id={`${uid}-privacy`}>{t.sections.privacy}</h2>
        <ul>
          {t.privacy.items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
        <p className="muted">{t.privacy.storageNote}</p>
      </section>
    </main>
  );
}
