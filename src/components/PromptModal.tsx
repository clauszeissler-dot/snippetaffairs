import { useState } from "react";

interface Props {
  title: string;
  label: string;
  placeholder?: string;
  initial?: string;
  hint?: string;
  confirmLabel?: string;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: (value: string) => void;
}

/**
 * Ersetzt `window.prompt`: das native Dialogfenster blockiert die WebView,
 * ist plattformabhängig gestylt und passt nicht ins Design.
 */
export default function PromptModal({
  title,
  label,
  placeholder,
  initial = "",
  hint,
  confirmLabel = "Anlegen",
  busy = false,
  onCancel,
  onConfirm,
}: Props) {
  const [value, setValue] = useState(initial);
  const trimmed = value.trim();

  function submit() {
    if (trimmed && !busy) onConfirm(trimmed);
  }

  return (
    <div className="overlay" onMouseDown={onCancel}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <h3>{title}</h3>
        {hint && <div className="note">{hint}</div>}
        <div className="field mono">
          <label>{label}</label>
          <input
            value={value}
            placeholder={placeholder}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") submit();
              if (e.key === "Escape") onCancel();
            }}
            autoFocus
          />
        </div>
        <div className="modal-actions">
          <button className="btn btn-ghost" onClick={onCancel} disabled={busy}>
            Abbrechen
          </button>
          <button className="btn btn-cta" onClick={submit} disabled={busy || !trimmed}>
            {busy ? <span className="spin" /> : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
