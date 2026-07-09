import type { ActionId, UserFacingError } from "../lib/errors";

interface Props {
  ui: UserFacingError;
  /** Technische Zusatzinfo aus dem Backend (kann Pfade enthalten → nie in den Report). */
  detail: string;
  onAction: (id: ActionId) => void;
  onClose: () => void;
}

const SEVERITY_ICON: Record<UserFacingError["severity"], string> = {
  info: "ℹ️",
  warning: "⚠️",
  error: "⛑️",
};

export default function ErrorDialog({ ui, detail, onAction, onClose }: Props) {
  // "back" heißt in dieser App "Dialog schließen". Bietet die Registry kein
  // "back" an, braucht der Dialog trotzdem einen Ausgang → Ghost-Button.
  const hasBack = ui.actions.some((a) => a.id === "back");

  return (
    <div className="overlay" onMouseDown={onClose}>
      <div
        className={`modal error-modal sev-${ui.severity}`}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <h3>
          <span className="err-icon">{SEVERITY_ICON[ui.severity]}</span> {ui.title}
        </h3>

        <p className="err-message">{ui.message}</p>

        {detail && <div className="note err-detail">{detail}</div>}

        <div className="err-reference" title="Bitte bei einer Meldung mitschicken">
          {ui.reference}
        </div>

        <div className="modal-actions">
          {!hasBack && (
            <button className="btn btn-ghost" onClick={onClose}>
              Schließen
            </button>
          )}
          {ui.actions.map((a) => (
            <button
              key={a.id}
              className={a.id === "retry" || a.id === "reload" ? "btn btn-cta" : "btn btn-ghost"}
              onClick={() => onAction(a.id)}
            >
              {a.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
