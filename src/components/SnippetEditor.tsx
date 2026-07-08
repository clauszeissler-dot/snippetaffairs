import { useState } from "react";
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

export default function SnippetEditor({ target, onCancel, onSave, busy }: Props) {
  const s = target.snippet;
  const [trigger, setTrigger] = useState(s?.advanced ? s.trigger : s?.trigger ?? "");
  const [replace, setReplace] = useState(s && !s.advanced ? s.replace : "");
  const [label, setLabel] = useState(s?.label ?? "");

  const advanced = !!s?.advanced;
  const isNew = !s;

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
              ? "dynamische Variablen"
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
            value={replace}
            onChange={(e) => setReplace(e.target.value)}
            placeholder="Text, der eingefügt wird…"
            disabled={advanced}
          />
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
