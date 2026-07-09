import { useEffect, useRef, useState } from "react";

interface Props {
  trigger: string;
  /** Sekunden bis zur Expansion. */
  delay?: number;
  onCancel: () => void;
  onFire: () => void;
}

/**
 * `espanso match exec` expandiert in das Fenster, das GERADE den Fokus hat —
 * das wäre sonst SnippetAffAIrs selbst, und der Text landete im Suchfeld.
 * Deshalb ein Countdown: der Nutzer wechselt in die Ziel-App, dann feuern wir.
 */
export default function TestSnippetModal({ trigger, delay = 3, onCancel, onFire }: Props) {
  const [left, setLeft] = useState(delay);
  const fired = useRef(false);

  useEffect(() => {
    const tick = window.setInterval(() => setLeft((n) => n - 1), 1000);
    return () => window.clearInterval(tick);
  }, []);

  useEffect(() => {
    if (left <= 0 && !fired.current) {
      fired.current = true;
      onFire();
    }
  }, [left, onFire]);

  return (
    <div className="overlay" onMouseDown={onCancel}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <h3>Snippet testen</h3>
        <div className="note">
          Klick jetzt in das Fenster, in dem der Text erscheinen soll — ein Editor, ein
          Browser-Feld, egal was. Nach dem Countdown wird{" "}
          <code style={{ color: "var(--ki-orange)" }}>{trigger}</code> dort ausgelöst.
        </div>

        <div className="countdown" aria-live="polite">
          {Math.max(left, 0)}
        </div>

        <div className="modal-actions">
          <button className="btn btn-ghost" onClick={onCancel}>
            Abbrechen
          </button>
        </div>
      </div>
    </div>
  );
}
