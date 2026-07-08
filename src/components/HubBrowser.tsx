import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { fetchHubIndex, fetchManifest, type HubPackage } from "../lib/hub";

interface Props {
  notify: (msg: string, kind?: "ok" | "err") => void;
  onChanged: () => void; // nach install/uninstall Snippets neu laden
}

const PAGE = 45;

export default function HubBrowser({ notify, onChanged }: Props) {
  const [index, setIndex] = useState<HubPackage[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [installedRaw, setInstalledRaw] = useState("");
  const [busyPkg, setBusyPkg] = useState<string | null>(null);
  const details = useRef<Map<string, Partial<HubPackage>>>(new Map());
  const [, force] = useState(0);

  async function loadInstalled() {
    try {
      const r = await api.packageList();
      setInstalledRaw(r.output || "");
    } catch {
      /* egal */
    }
  }

  useEffect(() => {
    (async () => {
      try {
        setLoading(true);
        const idx = await fetchHubIndex();
        setIndex(idx);
        setError(null);
      } catch (e: any) {
        setError(String(e?.message ?? e));
      } finally {
        setLoading(false);
      }
      loadInstalled();
    })();
  }, []);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const base = q
      ? index.filter(
          (p) =>
            p.name.toLowerCase().includes(q) ||
            (details.current.get(p.name)?.title ?? "").toLowerCase().includes(q) ||
            (details.current.get(p.name)?.description ?? "").toLowerCase().includes(q)
        )
      : index;
    return base.slice(0, PAGE);
  }, [query, index, installedRaw]);

  // Lazy-Details für die sichtbaren Karten nachladen
  useEffect(() => {
    let cancelled = false;
    (async () => {
      await Promise.all(
        filtered.map(async (p) => {
          if (details.current.has(p.name)) return;
          const m = await fetchManifest(p.name, p.version);
          if (!cancelled) details.current.set(p.name, m);
        })
      );
      if (!cancelled) force((n) => n + 1);
    })();
    return () => {
      cancelled = true;
    };
  }, [filtered]);

  function isInstalled(name: string): boolean {
    const re = new RegExp(`(^|[^a-z0-9-])${name}([^a-z0-9-]|$)`, "i");
    return re.test(installedRaw);
  }

  async function doInstall(name: string) {
    setBusyPkg(name);
    try {
      const r = await api.packageInstall(name);
      notify(
        r.success ? `Paket „${name}" installiert.` : `Fehler: ${r.output}`,
        r.success ? "ok" : "err"
      );
      await loadInstalled();
      onChanged();
    } catch (e: any) {
      notify(String(e), "err");
    } finally {
      setBusyPkg(null);
    }
  }

  async function doUninstall(name: string) {
    setBusyPkg(name);
    try {
      const r = await api.packageUninstall(name);
      notify(
        r.success ? `Paket „${name}" entfernt.` : `Fehler: ${r.output}`,
        r.success ? "ok" : "err"
      );
      await loadInstalled();
      onChanged();
    } catch (e: any) {
      notify(String(e), "err");
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

      {error && (
        <div className="banner">
          <b>Hub nicht erreichbar.</b> {error}
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
            const d = details.current.get(p.name) ?? {};
            const installed = isInstalled(p.name);
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
                  {installed ? (
                    <>
                      <span className="pkg-installed">✓ installiert</span>
                      <div className="spacer" style={{ flex: 1 }} />
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
