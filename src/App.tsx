import { useCallback, useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, type EspansoInfo, type FileGroup, type SnippetView } from "./lib/api";
import {
  AppError,
  createReport,
  formatUser,
  parseError,
  type ActionId,
  type Report,
  type UserFacingError,
} from "./lib/errors";
import SnippetEditor, { type EditorTarget } from "./components/SnippetEditor";
import HubBrowser from "./components/HubBrowser";
import ErrorDialog from "./components/ErrorDialog";
import PromptModal from "./components/PromptModal";

type View = "snippets" | "hub";
type Toast = { msg: string; kind: "ok" | "err" } | null;

/** Ein aufgelöster Fehler samt Kontext für Report und Wiederholung. */
interface ErrorState {
  ui: UserFacingError;
  detail: string;
  report: Report;
  retry?: () => void;
}

const SUPPORT_MAIL = "info@affairs-consulting.de";

export default function App() {
  const [info, setInfo] = useState<EspansoInfo | null>(null);
  const [groups, setGroups] = useState<FileGroup[]>([]);
  const [view, setView] = useState<View>("snippets");
  const [activeFile, setActiveFile] = useState<string | null>(null); // null = alle
  const [query, setQuery] = useState("");
  const [running, setRunning] = useState<boolean | null>(null);
  const [editor, setEditor] = useState<EditorTarget | null>(null);
  const [editorBusy, setEditorBusy] = useState(false);
  const [confirmDel, setConfirmDel] = useState<{
    file: string;
    index: number;
    trigger: string;
  } | null>(null);
  const [newFileOpen, setNewFileOpen] = useState(false);
  const [newFileBusy, setNewFileBusy] = useState(false);
  const [toast, setToast] = useState<Toast>(null);
  const [svcBusy, setSvcBusy] = useState(false);
  const [error, setError] = useState<ErrorState | null>(null);
  const [appVersion, setAppVersion] = useState("0.0.0");

  const notify = useCallback((msg: string, kind: "ok" | "err" = "ok") => {
    setToast({ msg, kind });
    window.setTimeout(() => setToast(null), 3800);
  }, []);

  /**
   * Einziger Weg, wie ein Fehler beim Nutzer landet: Backend-Code auflösen,
   * Report (PII-frei) erzeugen, Dialog mit den Aktionen der Registry zeigen.
   */
  const fail = useCallback(
    (e: unknown, retry?: () => void) => {
      const { code, detail } = parseError(e);
      const report = createReport(code, { appVersion, route: view });
      setError({
        ui: formatUser(code, { locale: "de", traceId: report.traceId }),
        detail,
        report,
        retry,
      });
    },
    [appVersion, view]
  );

  const loadInfo = useCallback(async () => {
    try {
      setInfo(await api.info());
    } catch (e) {
      fail(e);
    }
  }, [fail]);

  const loadSnippets = useCallback(async () => {
    try {
      setGroups(await api.listSnippets());
    } catch (e) {
      fail(e);
    }
  }, [fail]);

  const refreshStatus = useCallback(async () => {
    try {
      const r = await api.serviceStatus();
      const out = r.output.toLowerCase();
      const on = out.includes("running") && !out.includes("not running");
      setRunning(on);
    } catch {
      setRunning(null);
    }
  }, []);

  const reloadAll = useCallback(() => {
    loadSnippets();
    refreshStatus();
    loadInfo();
  }, [loadSnippets, refreshStatus, loadInfo]);

  useEffect(() => {
    getVersion()
      .then(setAppVersion)
      .catch(() => {
        /* außerhalb von Tauri (vite dev im Browser) — Default bleibt stehen */
      });
    loadInfo();
    loadSnippets();
    refreshStatus();
  }, [loadInfo, loadSnippets, refreshStatus]);

  async function onErrorAction(id: ActionId) {
    const current = error;
    if (!current) return;
    switch (id) {
      case "back":
        setError(null);
        break;
      case "retry":
        setError(null);
        current.retry?.();
        break;
      case "reload":
        setError(null);
        reloadAll();
        break;
      case "copy":
        try {
          await navigator.clipboard.writeText(JSON.stringify(current.report, null, 2));
          notify("Fehlerdetails kopiert.");
        } catch {
          notify("Kopieren nicht möglich.", "err");
        }
        break;
      case "report": {
        // Nur der PII-freie Report geht mit — kein Dateipfad, kein Snippet-Inhalt.
        const subject = `SnippetAffAIrs — ${current.report.code} (${current.report.traceId})`;
        const body = JSON.stringify(current.report, null, 2);
        try {
          await openUrl(
            `mailto:${SUPPORT_MAIL}?subject=${encodeURIComponent(
              subject
            )}&body=${encodeURIComponent(body)}`
          );
        } catch {
          notify("Mail-Programm ließ sich nicht öffnen.", "err");
        }
        break;
      }
    }
  }

  const totalCount = useMemo(
    () => groups.reduce((n, g) => n + g.snippets.length, 0),
    [groups]
  );

  // Sichtbare Gruppen: nach Datei-Filter + Suchfilter
  const visibleGroups = useMemo(() => {
    const q = query.trim().toLowerCase();
    return groups
      .filter((g) => !activeFile || g.path === activeFile)
      .map((g) => ({
        ...g,
        snippets: q
          ? g.snippets.filter(
              (s) =>
                s.trigger.toLowerCase().includes(q) ||
                s.replace.toLowerCase().includes(q) ||
                (s.label ?? "").toLowerCase().includes(q)
            )
          : g.snippets,
      }))
      .filter((g) => g.snippets.length > 0);
  }, [groups, activeFile, query]);

  // --- Service-Aktionen ----------------------------------------------------
  async function svc(action: "start" | "stop" | "restart") {
    setSvcBusy(true);
    try {
      const r =
        action === "start"
          ? await api.serviceStart()
          : action === "stop"
          ? await api.serviceStop()
          : await api.serviceRestart();
      if (!r.success) {
        // Die Engine hat geantwortet, aber die Aktion schlug fehl (z. B. macOS:
        // fehlende Freigabe unter Bedienungshilfen → "start: timed out").
        throw new AppError("AI-2016-FLOW", r.output);
      }
      const actionLabel =
        action === "start" ? "gestartet" : action === "stop" ? "gestoppt" : "neu gestartet";
      notify(r.output || `Engine ${actionLabel}`);
    } catch (e) {
      fail(e, () => svc(action));
    } finally {
      setSvcBusy(false);
      refreshStatus();
    }
  }

  // --- Snippet-Aktionen ----------------------------------------------------
  function openNew() {
    const targetPath = activeFile ?? (groups[0] ? groups[0].path : null);
    if (!targetPath) {
      notify("Lege zuerst eine Match-Datei an.", "err");
      return;
    }
    const g = groups.find((x) => x.path === targetPath)!;
    setEditor({ filePath: g.path, fileName: g.name, snippet: null });
  }

  function openEdit(g: FileGroup, s: SnippetView) {
    setEditor({ filePath: g.path, fileName: g.name, snippet: s });
  }

  async function saveSnippet(data: { trigger: string; replace: string; label: string }) {
    if (!editor) return;
    setEditorBusy(true);
    try {
      await api.saveSnippet({
        filePath: editor.filePath,
        index: editor.snippet ? editor.snippet.index : null,
        trigger: data.trigger,
        replace: data.replace,
        label: data.label.trim() ? data.label : null,
        // Beim Bearbeiten prüft das Backend, ob der Index noch auf dieses
        // Snippet zeigt — sonst hat jemand die Datei zwischenzeitlich geändert.
        expectedTrigger: editor.snippet ? editor.snippet.trigger : null,
      });
      setEditor(null);
      notify(editor.snippet ? "Snippet aktualisiert." : "Snippet angelegt.");
      await loadSnippets();
    } catch (e) {
      fail(e);
    } finally {
      setEditorBusy(false);
    }
  }

  async function doDelete() {
    if (!confirmDel) return;
    const target = confirmDel;
    setConfirmDel(null);
    try {
      await api.deleteSnippet(target.file, target.index, target.trigger);
      notify("Snippet gelöscht (Backup als .yml.bak abgelegt).");
      await loadSnippets();
    } catch (e) {
      fail(e);
    }
  }

  async function createFile(name: string) {
    setNewFileBusy(true);
    try {
      const path = await api.createMatchFile(name);
      setNewFileOpen(false);
      notify("Datei angelegt.");
      await loadSnippets();
      setActiveFile(path);
      setView("snippets");
    } catch (e) {
      fail(e);
    } finally {
      setNewFileBusy(false);
    }
  }

  const espansoMissing = info && !info.installed;

  return (
    <div className="app">
      {/* -------------------------------------------------- Sidebar */}
      <aside className="sidebar">
        <div className="brand">
          Snippet<span>Aff</span>
          <span className="glow-orange">AI</span>rs
        </div>
        <div className="brand-sub">Text-Expander · KI AffAIrs</div>

        <div className="nav-section-label">Match-Dateien</div>
        <div
          className={`nav-item ${view === "snippets" && !activeFile ? "active" : ""}`}
          onClick={() => {
            setActiveFile(null);
            setView("snippets");
          }}
        >
          <span>Alle Snippets</span>
          <span className="count">{totalCount}</span>
        </div>
        {groups.map((g) => (
          <div
            key={g.path}
            className={`nav-item ${
              view === "snippets" && activeFile === g.path ? "active" : ""
            }`}
            onClick={() => {
              setActiveFile(g.path);
              setView("snippets");
            }}
          >
            <span>{g.name}</span>
            <span className="count">{g.snippets.length}</span>
          </div>
        ))}
        <div className="nav-item" onClick={() => setNewFileOpen(true)}>
          <span>＋ Neue Datei</span>
        </div>

        <div className="nav-section-label">Erweiterung</div>
        <div
          className={`nav-item ${view === "hub" ? "active" : ""}`}
          onClick={() => setView("hub")}
        >
          <span>🧩 Paket-Hub</span>
        </div>

        <div className="sidebar-foot">
          {info?.installed ? (
            <>
              Engine <span className="ver">v{info.version ?? "?"}</span>
              <br />
              {info.config_path}
            </>
          ) : (
            "Engine nicht gefunden"
          )}
        </div>
      </aside>

      {/* -------------------------------------------------- Main */}
      <main className="main">
        <div className="servicebar">
          <span
            className={`status-dot ${
              running === true ? "on" : running === false ? "off" : ""
            }`}
          />
          <span className="status-text">
            Snippets:{" "}
            <b>{running === true ? "aktiv" : running === false ? "inaktiv" : "?"}</b>
          </span>
          <div className="spacer" />
          <button className="btn btn-sm" disabled={svcBusy} onClick={() => svc("start")}>
            Start
          </button>
          <button className="btn btn-sm" disabled={svcBusy} onClick={() => svc("stop")}>
            Stop
          </button>
          <button className="btn btn-sm" disabled={svcBusy} onClick={() => svc("restart")}>
            Neustart
          </button>
          <button className="btn btn-sm btn-ghost" onClick={reloadAll}>
            ⟳
          </button>
        </div>

        <div className="content">
          {espansoMissing && (
            <div className="banner">
              <b>Die Text-Expander-Engine ist nicht installiert.</b> SnippetAffAIrs setzt auf
              die freie Engine <b>espanso</b> auf — einmalig installieren, macOS:{" "}
              <code>brew install --cask espanso</code>. Danach in dieser Leiste auf <b>Start</b>.
            </div>
          )}

          {view === "hub" ? (
            <HubBrowser notify={notify} onError={fail} onChanged={loadSnippets} />
          ) : (
            <>
              <div className="content-head">
                <h2>
                  {activeFile
                    ? groups.find((g) => g.path === activeFile)?.name
                    : "Alle Snippets"}
                </h2>
                <div className="search">
                  <span>🔍</span>
                  <input
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder="Snippets durchsuchen…"
                  />
                </div>
                <div className="spacer" />
                <button className="btn btn-cta" onClick={openNew}>
                  ＋ Neues Snippet
                </button>
              </div>

              {totalCount === 0 ? (
                <div className="empty">
                  <div className="big">⌨️</div>
                  Noch keine Snippets. Leg mit <b>＋ Neues Snippet</b> los.
                </div>
              ) : visibleGroups.length === 0 ? (
                <div className="empty">Keine Treffer für „{query}"</div>
              ) : (
                visibleGroups.map((g) => (
                  <div key={g.path}>
                    {!activeFile && <div className="group-label">{g.name}.yml</div>}
                    {g.snippets.map((s) => (
                      <div className="card snippet" key={`${g.path}:${s.index}`}>
                        <span className="trigger" title={s.trigger}>
                          {s.trigger}
                        </span>
                        <span className="arrow">→</span>
                        <span className="replace">
                          {s.label && <span className="label">{s.label}</span>}
                          {s.replace}
                        </span>
                        {s.advanced && <span className="badge">{s.kind}</span>}
                        <div className="actions">
                          <button
                            className="btn btn-sm btn-ghost"
                            onClick={() => openEdit(g, s)}
                          >
                            {s.advanced ? "Ansehen" : "Bearbeiten"}
                          </button>
                          <button
                            className="btn btn-sm btn-danger"
                            onClick={() =>
                              setConfirmDel({
                                file: g.path,
                                index: s.index,
                                trigger: s.trigger,
                              })
                            }
                          >
                            Löschen
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                ))
              )}
            </>
          )}
        </div>
      </main>

      {/* -------------------------------------------------- Editor */}
      {editor && (
        <SnippetEditor
          target={editor}
          busy={editorBusy}
          onCancel={() => setEditor(null)}
          onSave={saveSnippet}
        />
      )}

      {/* -------------------------------------------------- Neue Datei */}
      {newFileOpen && (
        <PromptModal
          title="Neue Match-Datei"
          label="Dateiname (ohne .yml)"
          placeholder="meine-snippets"
          initial="meine-snippets"
          hint="Erlaubt sind Buchstaben, Ziffern, - und _. Alles andere wird zu einem Bindestrich."
          busy={newFileBusy}
          onCancel={() => setNewFileOpen(false)}
          onConfirm={createFile}
        />
      )}

      {/* -------------------------------------------------- Delete-Confirm */}
      {confirmDel && (
        <div className="overlay" onMouseDown={() => setConfirmDel(null)}>
          <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
            <h3>Snippet löschen?</h3>
            <div className="note">
              Trigger{" "}
              <code style={{ color: "var(--ki-orange)" }}>{confirmDel.trigger}</code> wird
              entfernt. Ein Backup der Datei wird als <code>.yml.bak</code> abgelegt.
            </div>
            <div className="modal-actions">
              <button className="btn btn-ghost" onClick={() => setConfirmDel(null)}>
                Abbrechen
              </button>
              <button className="btn btn-danger" onClick={doDelete}>
                Löschen
              </button>
            </div>
          </div>
        </div>
      )}

      {/* -------------------------------------------------- Fehler */}
      {error && (
        <ErrorDialog
          ui={error.ui}
          detail={error.detail}
          onAction={onErrorAction}
          onClose={() => setError(null)}
        />
      )}

      {/* -------------------------------------------------- Toast */}
      {toast && <div className={`toast ${toast.kind}`}>{toast.msg}</div>}
    </div>
  );
}
