import { useCallback, useEffect, useState } from "react";
import { api, type EspansoInfo } from "../lib/api";

interface Props {
  info: EspansoInfo | null;
  running: boolean | null;
  autostart: boolean | null;
  notify: (msg: string, kind?: "ok" | "err") => void;
  onError: (e: unknown, retry?: () => void) => void;
}

const IS_MAC = navigator.userAgent.includes("Mac");

export default function Diagnostics({ info, running, autostart, notify, onError }: Props) {
  const [log, setLog] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Ein fehlendes Log ist hier kein Zwischenfall, sondern selbst ein Befund —
  // es gehört ins Panel, nicht in einen Fehlerdialog über dem Panel.
  const loadLog = useCallback(async () => {
    setBusy(true);
    try {
      const r = await api.engineLog();
      setLog(r.output || "(Das Log ist leer.)");
    } catch (e) {
      setLog(`(Log nicht abrufbar: ${e instanceof Error ? e.message : String(e)})`);
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    // `loadLog` setzt State erst nach `await` — kein synchroner setState.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    loadLog();
  }, [loadLog]);

  async function secureInput() {
    setBusy(true);
    try {
      const r = await api.fixSecureInput();
      notify(r.output || "Versuch abgeschlossen — teste die Eingabe erneut.");
    } catch (e) {
      onError(e);
    } finally {
      setBusy(false);
    }
  }

  async function openAccessibility() {
    try {
      await api.openAccessibilitySettings();
    } catch (e) {
      onError(e);
    }
  }

  const rows: [string, string][] = [
    ["Engine gefunden", info?.installed ? "ja" : "nein"],
    ["Engine-Version", info?.version ?? "unbekannt"],
    ["Dienst läuft", running === true ? "ja" : running === false ? "nein" : "unbekannt"],
    ["Autostart", autostart === true ? "an" : autostart === false ? "aus" : "unbekannt"],
    ["Konfiguration", info?.config_path ?? "—"],
    ["Snippet-Ordner", info?.match_dir ?? "—"],
  ];

  return (
    <>
      <div className="content-head">
        <h2>Diagnose</h2>
        <div className="spacer" />
        <button className="btn btn-sm btn-ghost" disabled={busy} onClick={loadLog}>
          ⟳ Log neu laden
        </button>
      </div>

      <div className="card diag-facts">
        {rows.map(([k, v]) => (
          <div className="diag-row" key={k}>
            <span className="diag-key">{k}</span>
            <span className="diag-val">{v}</span>
          </div>
        ))}
      </div>

      {IS_MAC && (
        <div className="banner">
          <b>Nichts wird eingefügt?</b> Unter macOS braucht die Engine eine Freigabe unter
          Bedienungshilfen — ohne sie startet sie nicht („start: timed out"). Und nach einem
          Passwort-Dialog kann „Secure Input" die Eingabe systemweit blockieren.
          <div className="banner-actions">
            <button className="btn btn-sm" onClick={openAccessibility}>
              Bedienungshilfen öffnen
            </button>
            <button className="btn btn-sm" disabled={busy} onClick={secureInput}>
              Secure Input beheben
            </button>
          </div>
        </div>
      )}

      <div className="group-label">Log der Engine</div>
      <pre className="diag-log">{log ?? "…"}</pre>
    </>
  );
}
