import { useEffect, useState } from "react";
import { api, type BackupInfo } from "../lib/api";

interface Props {
  filePath: string;
  fileName: string;
  onClose: () => void;
  onRestored: () => void;
  onError: (e: unknown) => void;
}

const KIND_LABEL: Record<BackupInfo["kind"], string> = {
  bak: "Letzter Stand vor der jüngsten Änderung",
  orig: "Original, bevor SnippetAffAIrs die Datei zum ersten Mal geändert hat",
};

function formatDate(unixSeconds: number): string {
  if (!unixSeconds) return "unbekannt";
  return new Date(unixSeconds * 1000).toLocaleString("de-DE", {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

export default function BackupsModal({
  filePath,
  fileName,
  onClose,
  onRestored,
  onError,
}: Props) {
  const [backups, setBackups] = useState<BackupInfo[] | null>(null);
  const [confirm, setConfirm] = useState<BackupInfo | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.listBackups(filePath).then(setBackups).catch(onError);
  }, [filePath, onError]);

  async function restore(b: BackupInfo) {
    setBusy(true);
    try {
      await api.restoreBackup(filePath, b.kind);
      onRestored();
    } catch (e) {
      onError(e);
    } finally {
      setBusy(false);
      setConfirm(null);
    }
  }

  return (
    <div className="overlay" onMouseDown={onClose}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <h3>Backups von {fileName}.yml</h3>

        {confirm ? (
          <>
            <div className="note">
              <b>{KIND_LABEL[confirm.kind]}</b> vom {formatDate(confirm.modified)} mit{" "}
              {confirm.snippet_count} Snippet{confirm.snippet_count === 1 ? "" : "s"}{" "}
              zurückspielen? Der jetzige Stand wird vorher als{" "}
              <code>.yml.bak</code> gesichert — du kommst also wieder zurück.
            </div>
            <div className="modal-actions">
              <button className="btn btn-ghost" onClick={() => setConfirm(null)} disabled={busy}>
                Abbrechen
              </button>
              <button className="btn btn-cta" onClick={() => restore(confirm)} disabled={busy}>
                {busy ? <span className="spin" /> : "Wiederherstellen"}
              </button>
            </div>
          </>
        ) : (
          <>
            {backups === null ? (
              <div className="note">
                <span className="spin" /> Backups werden gesucht…
              </div>
            ) : backups.length === 0 ? (
              <div className="note">
                Für diese Datei gibt es noch kein Backup. Sobald du hier ein Snippet
                änderst, wird das Original gesichert.
              </div>
            ) : (
              <div className="backup-list">
                {backups.map((b) => (
                  <div className="card backup" key={b.kind}>
                    <div>
                      <div className="backup-kind">
                        {b.kind === "orig" ? "Original" : "Vorheriger Stand"}
                      </div>
                      <div className="backup-meta">
                        {formatDate(b.modified)} · {b.snippet_count} Snippet
                        {b.snippet_count === 1 ? "" : "s"}
                      </div>
                      <div className="backup-hint">{KIND_LABEL[b.kind]}</div>
                    </div>
                    <div className="spacer" style={{ flex: 1 }} />
                    <button className="btn btn-sm" onClick={() => setConfirm(b)}>
                      Wiederherstellen
                    </button>
                  </div>
                ))}
              </div>
            )}
            <div className="modal-actions">
              <button className="btn btn-ghost" onClick={onClose}>
                Schließen
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
