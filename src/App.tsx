import { useCallback, useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  api,
  type EspansoInfo,
  type FileGroup,
  type SnippetView,
  type TriggerConflict,
} from "./lib/api";
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
import TestSnippetModal from "./components/TestSnippetModal";
import BackupsModal from "./components/BackupsModal";
import Diagnostics from "./components/Diagnostics";
import Logo from "./components/Logo";

type View = "snippets" | "hub" | "diag";
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
  const [conflicts, setConflicts] = useState<TriggerConflict[]>([]);
  const [view, setView] = useState<View>("snippets");
  const [activeFile, setActiveFile] = useState<string | null>(null); // null = alle
  const [query, setQuery] = useState("");
  const [running, setRunning] = useState<boolean | null>(null);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [editor, setEditor] = useState<EditorTarget | null>(null);
  const [editorBusy, setEditorBusy] = useState(false);
  const [confirmDel, setConfirmDel] = useState<{
    file: string;
    index: number;
    trigger: string;
  } | null>(null);
  const [newFileOpen, setNewFileOpen] = useState(false);
  const [newFileBusy, setNewFileBusy] = useState(false);
  const [renameFile, setRenameFile] = useState<FileGroup | null>(null);
  const [renameBusy, setRenameBusy] = useState(false);
  const [confirmFileDel, setConfirmFileDel] = useState<FileGroup | null>(null);
  const [backupsFor, setBackupsFor] = useState<FileGroup | null>(null);
  const [testTrigger, setTestTrigger] = useState<string | null>(null);
  const [showConflicts, setShowConflicts] = useState(false);
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
    // Kollisionen sind eine Zusatzinfo — ihr Ausfall darf die Liste nicht kippen.
    try {
      setConflicts(await api.triggerConflicts());
    } catch {
      setConflicts([]);
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
    try {
      setAutostart(await api.autostartEnabled());
    } catch {
      setAutostart(null);
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

  /** Trigger, die mehr als einmal vergeben sind — für die Markierung in der Liste. */
  const conflictTriggers = useMemo(
    () => new Set(conflicts.map((c) => c.trigger)),
    [conflicts]
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

  const activeGroup = useMemo(
    () => groups.find((g) => g.path === activeFile) ?? null,
    [groups, activeFile]
  );

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

  async function toggleAutostart() {
    const turnOn = !autostart;
    setSvcBusy(true);
    try {
      const r = turnOn ? await api.autostartEnable() : await api.autostartDisable();
      if (!r.success) throw new AppError("AI-2016-FLOW", r.output);
      notify(turnOn ? "Autostart aktiviert." : "Autostart deaktiviert.");
    } catch (e) {
      fail(e, toggleAutostart);
    } finally {
      setSvcBusy(false);
      refreshStatus();
    }
  }

  async function fireTest(trigger: string) {
    setTestTrigger(null);
    try {
      // Das Backend erkennt auch die Fehler, die espanso mit Exit-Code 0 meldet.
      await api.matchExec(trigger);
      notify(`${trigger} ausgelöst.`);
    } catch (e) {
      fail(e);
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

  // --- Datei-Aktionen ------------------------------------------------------
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

  async function doRename(newName: string) {
    if (!renameFile) return;
    setRenameBusy(true);
    try {
      const path = await api.renameMatchFile(renameFile.path, newName);
      setRenameFile(null);
      notify("Datei umbenannt.");
      await loadSnippets();
      setActiveFile(path);
    } catch (e) {
      fail(e);
    } finally {
      setRenameBusy(false);
    }
  }

  async function doDeleteFile() {
    if (!confirmFileDel) return;
    const target = confirmFileDel;
    setConfirmFileDel(null);
    try {
      await api.deleteMatchFile(target.path);
      notify(`${target.name}.yml gelöscht.`);
      setActiveFile(null);
      await loadSnippets();
    } catch (e) {
      fail(e);
    }
  }

  const espansoMissing = info && !info.installed;

  return (
    <div className="app">
      {/* -------------------------------------------------- Sidebar */}
      <aside className="sidebar">
        <div className="brand">
          <Logo size={28} className="brand-logo" />
          <span className="brand-text">
            Snippet<span>Aff</span>
            <span className="glow-orange">AI</span>rs
          </span>
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
        <div
          className={`nav-item ${view === "diag" ? "active" : ""}`}
          onClick={() => setView("diag")}
        >
          <span>🩺 Diagnose</span>
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
          <label className="switch" title="Engine beim Anmelden automatisch starten">
            <input
              type="checkbox"
              checked={autostart === true}
              disabled={svcBusy || autostart === null}
              onChange={toggleAutostart}
            />
            <span>Autostart</span>
          </label>
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

          {view === "diag" ? (
            <Diagnostics
              info={info}
              running={running}
              autostart={autostart}
              notify={notify}
              onError={fail}
            />
          ) : view === "hub" ? (
            <HubBrowser notify={notify} onError={fail} onChanged={loadSnippets} />
          ) : (
            <>
              {conflicts.length > 0 && (
                <div className="banner warn">
                  <b>
                    {conflicts.length} Trigger{" "}
                    {conflicts.length === 1 ? "ist" : "sind"} doppelt vergeben.
                  </b>{" "}
                  Bei einer Doppelung expandiert die Engine nur eines der Snippets — ohne
                  Hinweis.{" "}
                  <button
                    className="linklike"
                    onClick={() => setShowConflicts((v) => !v)}
                  >
                    {showConflicts ? "Details ausblenden" : "Details anzeigen"}
                  </button>
                  {showConflicts && (
                    <ul className="conflict-list">
                      {conflicts.map((c) => (
                        <li key={c.trigger}>
                          <code>{c.trigger}</code> in{" "}
                          {c.sites.map((s) => s.source).join(", ")}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}

              <div className="content-head">
                <h2>{activeGroup ? activeGroup.name : "Alle Snippets"}</h2>
                <div className="search">
                  <span>🔍</span>
                  <input
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder="Snippets durchsuchen…"
                  />
                </div>
                <div className="spacer" />
                {activeGroup && (
                  <div className="file-actions">
                    <button
                      className="btn btn-sm btn-ghost"
                      onClick={() => setRenameFile(activeGroup)}
                    >
                      Umbenennen
                    </button>
                    <button
                      className="btn btn-sm btn-ghost"
                      onClick={() => setBackupsFor(activeGroup)}
                    >
                      Backups
                    </button>
                    <button
                      className="btn btn-sm btn-danger"
                      onClick={() => setConfirmFileDel(activeGroup)}
                    >
                      Datei löschen
                    </button>
                  </div>
                )}
                <button className="btn btn-cta" onClick={openNew}>
                  ＋ Neues Snippet
                </button>
              </div>

              {totalCount === 0 ? (
                <div className="empty">
                  <Logo size={72} className="empty-logo" />
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
                        {conflictTriggers.has(s.trigger) && (
                          <span
                            className="badge warn"
                            title="Dieser Trigger ist mehrfach vergeben — nur eines der Snippets wird ausgelöst."
                          >
                            ⚠ doppelt
                          </span>
                        )}
                        <span className="arrow">→</span>
                        <span className="replace">
                          {s.label && <span className="label">{s.label}</span>}
                          {s.replace}
                        </span>
                        {s.kind !== "text" && <span className="badge">{s.kind}</span>}
                        <div className="actions">
                          <button
                            className="btn btn-sm btn-ghost"
                            title="Snippet im aktiven Fenster auslösen"
                            disabled={running !== true}
                            onClick={() => setTestTrigger(s.trigger)}
                          >
                            ▶ Testen
                          </button>
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

      {/* -------------------------------------------------- Datei umbenennen */}
      {renameFile && (
        <PromptModal
          title={`${renameFile.name}.yml umbenennen`}
          label="Neuer Dateiname (ohne .yml)"
          initial={renameFile.name}
          hint="Vorhandene Backups (.yml.bak / .yml.orig) werden mit umbenannt."
          confirmLabel="Umbenennen"
          busy={renameBusy}
          onCancel={() => setRenameFile(null)}
          onConfirm={doRename}
        />
      )}

      {/* -------------------------------------------------- Backups */}
      {backupsFor && (
        <BackupsModal
          filePath={backupsFor.path}
          fileName={backupsFor.name}
          onClose={() => setBackupsFor(null)}
          onRestored={() => {
            setBackupsFor(null);
            notify("Backup wiederhergestellt.");
            loadSnippets();
          }}
          onError={fail}
        />
      )}

      {/* -------------------------------------------------- Snippet testen */}
      {testTrigger && (
        <TestSnippetModal
          trigger={testTrigger}
          onCancel={() => setTestTrigger(null)}
          onFire={() => fireTest(testTrigger)}
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

      {/* -------------------------------------------------- Datei löschen */}
      {confirmFileDel && (
        <div className="overlay" onMouseDown={() => setConfirmFileDel(null)}>
          <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
            <h3>Datei {confirmFileDel.name}.yml löschen?</h3>
            <div className="note">
              <b>
                {confirmFileDel.snippets.length} Snippet
                {confirmFileDel.snippets.length === 1 ? "" : "s"}
              </b>{" "}
              werden mitgelöscht — samt der Backups dieser Datei. Das lässt sich aus der App
              heraus nicht rückgängig machen.
            </div>
            <div className="modal-actions">
              <button className="btn btn-ghost" onClick={() => setConfirmFileDel(null)}>
                Abbrechen
              </button>
              <button className="btn btn-danger" onClick={doDeleteFile}>
                Endgültig löschen
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
