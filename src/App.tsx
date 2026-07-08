import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type EspansoInfo, type FileGroup, type SnippetView } from "./lib/api";
import SnippetEditor, { type EditorTarget } from "./components/SnippetEditor";
import HubBrowser from "./components/HubBrowser";

type View = "snippets" | "hub";
type Toast = { msg: string; kind: "ok" | "err" } | null;

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
  const [toast, setToast] = useState<Toast>(null);
  const [svcBusy, setSvcBusy] = useState(false);

  const notify = useCallback((msg: string, kind: "ok" | "err" = "ok") => {
    setToast({ msg, kind });
    window.setTimeout(() => setToast(null), 3800);
  }, []);

  const loadInfo = useCallback(async () => {
    try {
      setInfo(await api.info());
    } catch (e: any) {
      notify(String(e), "err");
    }
  }, [notify]);

  const loadSnippets = useCallback(async () => {
    try {
      setGroups(await api.listSnippets());
    } catch (e: any) {
      notify(String(e), "err");
    }
  }, [notify]);

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

  useEffect(() => {
    loadInfo();
    loadSnippets();
    refreshStatus();
  }, [loadInfo, loadSnippets, refreshStatus]);

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
      const actionLabel =
        action === "start" ? "gestartet" : action === "stop" ? "gestoppt" : "neu gestartet";
      notify(r.output || `Engine ${actionLabel}`, r.success ? "ok" : "err");
    } catch (e: any) {
      notify(String(e), "err");
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
        file_path: editor.filePath,
        index: editor.snippet ? editor.snippet.index : null,
        trigger: data.trigger,
        replace: data.replace,
        label: data.label.trim() ? data.label : null,
      });
      setEditor(null);
      notify(editor.snippet ? "Snippet aktualisiert." : "Snippet angelegt.");
      await loadSnippets();
    } catch (e: any) {
      notify(String(e), "err");
    } finally {
      setEditorBusy(false);
    }
  }

  async function doDelete() {
    if (!confirmDel) return;
    try {
      await api.deleteSnippet(confirmDel.file, confirmDel.index);
      notify("Snippet gelöscht (Backup als .yml.bak abgelegt).");
      await loadSnippets();
    } catch (e: any) {
      notify(String(e), "err");
    } finally {
      setConfirmDel(null);
    }
  }

  async function newFile() {
    const name = window.prompt("Name der neuen Match-Datei (ohne .yml):", "meine-snippets");
    if (!name) return;
    try {
      const path = await api.createMatchFile(name);
      notify("Datei angelegt.");
      await loadSnippets();
      setActiveFile(path);
      setView("snippets");
    } catch (e: any) {
      notify(String(e), "err");
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
        <div className="nav-item" onClick={newFile}>
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
          <button
            className="btn btn-sm btn-ghost"
            onClick={() => {
              loadSnippets();
              refreshStatus();
              loadInfo();
            }}
          >
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
            <HubBrowser notify={notify} onChanged={loadSnippets} />
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

      {/* -------------------------------------------------- Toast */}
      {toast && <div className={`toast ${toast.kind}`}>{toast.msg}</div>}
    </div>
  );
}
