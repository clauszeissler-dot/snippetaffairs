import { useRef, useState } from "react";
import type { SnippetView } from "../lib/api";

export interface EditorTarget {
  filePath: string;
  fileName: string;
  snippet: SnippetView | null; // null = neu
}

interface Props {
  target: EditorTarget;
  onCancel: () => void;
  onSave: (data: { trigger: string; replace: string; label: string }) => void;
  busy: boolean;
}

/** Bausteine, die das Backend beim Speichern in einen `vars`-Block übersetzt. */
const BUILDING_BLOCKS: { token: string; label: string; hint: string }[] = [
  { token: "{{date}}", label: "📅 Datum", hint: "Fügt das heutige Datum ein" },
  {
    token: "{{clipboard}}",
    label: "📋 Zwischenablage",
    hint: "Fügt den aktuellen Inhalt der Zwischenablage ein",
  },
];

export default function SnippetEditor({ target, onCancel, onSave, busy }: Props) {
  const s = target.snippet;
  const [trigger, setTrigger] = useState(s?.advanced ? s.trigger : s?.trigger ?? "");
  const [replace, setReplace] = useState(s && !s.advanced ? s.replace : "");
  const [label, setLabel] = useState(s?.label ?? "");
  const replaceRef = useRef<HTMLTextAreaElement>(null);

  const advanced = !!s?.advanced;
  const isNew = !s;

  /** Setzt den Baustein an der Cursorposition ein, nicht stumpf ans Ende. */
  function insert(token: string) {
    const el = replaceRef.current;
    if (!el) {
      setReplace((r) => r + token);
      return;
    }
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const next = replace.slice(0, start) + token + replace.slice(end);
    setReplace(next);
    // Cursor hinter den eingefügten Baustein setzen
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(start + token.length, start + token.length);
    });
  }

  return (
    <div className="overlay" onMouseDown={onCancel}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <h3>{isNew ? "Neues Snippet" : "Snippet bearbeiten"}</h3>

        {advanced && (
          <div className="note">
            Dieses Snippet nutzt ein <b>erweitertes Match</b> (
            {s?.kind === "form"
              ? "Formular"
              : s?.kind === "vars"
              ? "eigene Variablen"
              : s?.kind === "regex"
              ? "Regex"
              : s?.kind === "image"
              ? "Bild"
              : "Sonderfunktion"}
            ). Es ist hier schreibgeschützt, damit nichts verloren geht — bearbeite es
            vorerst direkt in der YAML-Datei.
          </div>
        )}

        <div className="field mono">
          <label>Trigger (Kürzel)</label>
          <input
            value={trigger}
            onChange={(e) => setTrigger(e.target.value)}
            placeholder=":kürzel"
            disabled={advanced}
            autoFocus
          />
        </div>

        <div className="field mono">
          <label>Ersetzung</label>
          <textarea
            ref={replaceRef}
            value={replace}
            onChange={(e) => setReplace(e.target.value)}
            placeholder="Text, der eingefügt wird…"
            disabled={advanced}
          />
          {!advanced && (
            <div className="blocks">
              <span className="blocks-label">Bausteine:</span>
              {BUILDING_BLOCKS.map((b) => (
                <button
                  key={b.token}
                  type="button"
                  className="btn btn-sm btn-ghost"
                  title={b.hint}
                  onClick={() => insert(b.token)}
                >
                  {b.label}
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="field">
          <label>Label (optional)</label>
          <input
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="Kurzbeschreibung für die Suche"
            disabled={advanced}
          />
        </div>

        <div className="modal-actions">
          <button className="btn btn-ghost" onClick={onCancel} disabled={busy}>
            Abbrechen
          </button>
          {!advanced && (
            <button
              className="btn btn-cta"
              disabled={busy || !trigger.trim()}
              onClick={() => onSave({ trigger, replace, label })}
            >
              {busy ? <span className="spin" /> : "Speichern"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
