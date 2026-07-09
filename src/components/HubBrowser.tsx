import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { AppError } from "../lib/errors";
import {
  cmpVersion,
  fetchHubIndex,
  fetchManifest,
  parseInstalledPackages,
  type HubPackage,
} from "../lib/hub";

interface Props {
  notify: (msg: string, kind?: "ok" | "err") => void;
  onError: (e: unknown, retry?: () => void) => void;
  onChanged: () => void; // nach install/uninstall Snippets neu laden
}

const PAGE = 45;

export default function HubBrowser({ notify, onError, onChanged }: Props) {
  const [index, setIndex] = useState<HubPackage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<unknown>(null);
  const [query, setQuery] = useState("");
  const [installed, setInstalled] = useState<Map<string, string>>(new Map());
  const [busyPkg, setBusyPkg] = useState<string | null>(null);
  // Details liegen im State, nicht in einem Ref: sonst rechnet `filtered` nicht
  // neu, wenn Titel und Beschreibungen nachträglich eintreffen — die Suche über
  // diese Felder liefe dann auf einem veralteten Stand.
  const [details, setDetails] = useState<Map<string, Partial<HubPackage>>>(new Map());

  const loadInstalled = useCallback(async () => {
    try {
      const r = await api.packageList();
      setInstalled(parseInstalledPackages(r.output || ""));
    } catch {
      // Kein Blocker: ohne Liste fehlt nur die „installiert"-Markierung.
      setInstalled(new Map());
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        setLoading(true);
        const idx = await fetchHubIndex();
        setIndex(idx);
        setError(null);
      } catch (e) {
        setError(e);
      } finally {
        setLoading(false);
      }
      loadInstalled();
    })();
  }, [loadInstalled]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const base = q
      ? index.filter(
          (p) =>
            p.name.toLowerCase().includes(q) ||
            (details.get(p.name)?.title ?? "").toLowerCase().includes(q) ||
            (details.get(p.name)?.description ?? "").toLowerCase().includes(q)
        )
      : index;
    return base.slice(0, PAGE);
  }, [query, index, details]);

  // Lazy-Details für die sichtbaren Karten nachladen. Sind alle da, ist
  // `missing` leer und der Effekt läuft folgenlos aus — keine Schleife.
  useEffect(() => {
    const missing = filtered.filter((p) => !details.has(p.name));
    if (missing.length === 0) return;

    let cancelled = false;
    (async () => {
      const loaded = await Promise.all(
        missing.map(async (p) => [p.name, await fetchManifest(p.name, p.version)] as const)
      );
      if (cancelled) return;
      setDetails((prev) => {
        const next = new Map(prev);
        for (const [name, manifest] of loaded) next.set(name, manifest);
        return next;
      });
    })();
    return () => {
      cancelled = true;
    };
  }, [filtered, details]);



  async function doInstall(name: string) {
    setBusyPkg(name);
    try {
      const r = await api.packageInstall(name);
      if (!r.success) throw new AppError("AI-2016-FLOW", r.output);
      notify(`Paket „${name}" installiert.`);
      await loadInstalled();
      onChanged();
    } catch (e) {
      onError(e, () => doInstall(name));
    } finally {
      setBusyPkg(null);
    }
  }

  async function doUpdate(name: string) {
    setBusyPkg(name);
    try {
      const r = await api.packageUpdate(name);
      if (!r.success) throw new AppError("AI-2016-FLOW", r.output);
      notify(`Paket „${name}" aktualisiert.`);
      await loadInstalled();
      onChanged();
    } catch (e) {
      onError(e, () => doUpdate(name));
    } finally {
      setBusyPkg(null);
    }
  }

  async function doUninstall(name: string) {
    setBusyPkg(name);
    try {
      const r = await api.packageUninstall(name);
      if (!r.success) throw new AppError("AI-2016-FLOW", r.output);
      notify(`Paket „${name}" entfernt.`);
      await loadInstalled();
      onChanged();
    } catch (e) {
      onError(e, () => doUninstall(name));
    } finally {
      setBusyPkg(null);
    }
  }

  return (
    <>
      <div className="content-head">
        <h2>Paket-Hub</h2>
        <div className="search">
          <span>🔍</span>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Pakete durchsuchen…"
          />
        </div>
        <div className="spacer" />
        <span className="pkg-meta">
          {loading ? "lädt…" : `${index.length} Pakete`}
        </span>
      </div>

      {error != null && (
        <div className="banner">
          <b>Hub nicht erreichbar.</b>{" "}
          {error instanceof Error ? error.message : String(error)}
        </div>
      )}

      {loading ? (
        <div className="empty">
          <div className="big">
            <span className="spin" />
          </div>
          Hub-Index wird geladen…
        </div>
      ) : (
        <div className="hub-grid">
          {filtered.map((p) => {
            const d = details.get(p.name) ?? {};
            const installedVersion = installed.get(p.name);
            const isInstalled = installedVersion !== undefined;
            // Leere Version = Formatdrift der CLI-Ausgabe → kein Update anbieten,
            // statt fälschlich „veraltet" zu behaupten.
            const outdated = !!installedVersion && cmpVersion(p.version, installedVersion) > 0;
            const busy = busyPkg === p.name;
            return (
              <div className="card pkg" key={p.name}>
                <div className="pkg-title">{d.title ?? p.name}</div>
                <div className="pkg-meta">
                  {p.name} · v{p.version}
                  {d.author ? ` · ${d.author}` : ""}
                </div>
                <div className="pkg-desc">
                  {d.description ?? "Beschreibung wird geladen…"}
                </div>
                {d.tags && d.tags.length > 0 && (
                  <div className="pkg-tags">
                    {d.tags.slice(0, 5).map((t) => (
                      <span className="badge" key={t}>
                        {t}
                      </span>
                    ))}
                  </div>
                )}
                <div className="pkg-foot">
                  {isInstalled ? (
                    <>
                      <span className={`pkg-installed ${outdated ? "outdated" : ""}`}>
                        {outdated ? "↑ Update verfügbar" : "✓ installiert"}
                        {installedVersion ? ` · v${installedVersion}` : ""}
                      </span>
                      <div className="spacer" style={{ flex: 1 }} />
                      {outdated && (
                        <button
                          className="btn btn-cta btn-sm"
                          disabled={busy}
                          title={`Auf v${p.version} aktualisieren`}
                          onClick={() => doUpdate(p.name)}
                        >
                          {busy ? <span className="spin" /> : `Auf v${p.version}`}
                        </button>
                      )}
                      <button
                        className="btn btn-danger btn-sm"
                        disabled={busy}
                        onClick={() => doUninstall(p.name)}
                      >
                        {busy ? <span className="spin" /> : "Entfernen"}
                      </button>
                    </>
                  ) : (
                    <>
                      <div className="spacer" style={{ flex: 1 }} />
                      <button
                        className="btn btn-cta btn-sm"
                        disabled={busy}
                        onClick={() => doInstall(p.name)}
                      >
                        {busy ? <span className="spin" /> : "Installieren"}
                      </button>
                    </>
                  )}
                </div>
              </div>
            );
          })}
          {filtered.length === 0 && (
            <div className="empty">Keine Pakete für „{query}"</div>
          )}
        </div>
      )}
    </>
  );
}
