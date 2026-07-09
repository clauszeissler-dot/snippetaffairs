import { AppError } from "./errors";

// Zugriff auf den espanso-Hub direkt aus dem Frontend (CSP erlaubt externe Hosts).
// Struktur im Repo espanso/hub: packages/<name>/<version>/_manifest.yml
// Der rekursive Tree kommt in EINEM api.github.com-Call (Rate-Limit-schonend);
// Manifest-Details lazy über die raw-CDN (kein enges Limit).

const TREE_URL =
  "https://api.github.com/repos/espanso/hub/git/trees/main?recursive=1";
const RAW = "https://raw.githubusercontent.com/espanso/hub/main";

export interface HubPackage {
  name: string;
  version: string;
  title?: string;
  description?: string;
  author?: string;
  tags?: string[];
  homepage?: string;
}

/**
 * Semver-artiger Vergleich. >0 wenn a neuer als b.
 *
 * Ein führendes „v" wird abgeschnitten: `parseInt("v1")` ergibt NaN → 0, und
 * „v1" gälte damit als älter als „1" — die Oberfläche würde ein Update anbieten,
 * das keines ist.
 */
export function cmpVersion(a: string, b: string): number {
  const parts = (v: string) => v.trim().replace(/^v/i, "").split(".");
  const pa = parts(a).map((n) => parseInt(n, 10) || 0);
  const pb = parts(b).map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d !== 0) return d;
  }
  return 0;
}

/**
 * Parst die Ausgabe von `package list`. Verifiziertes Format je Zeile:
 *
 *     - all-emojis - version: 0.2.0 (espanso-hub)
 *
 * Liefert Name → Version. Zeilen ohne Versionsangabe werden tolerant mit
 * leerer Version übernommen (Formatdrift zwischen Engine-Versionen).
 *
 * Ersetzt die frühere Substring-Suche im Rohtext: die meldete ein Paket
 * namens `version` fälschlich als installiert, sobald irgendein Paket da war.
 */
export function parseInstalledPackages(raw: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const line of raw.split("\n")) {
    const withVersion = /^\s*-\s+(\S+)\s+-\s+version:\s*(\S+)/.exec(line);
    if (withVersion) {
      out.set(withVersion[1], withVersion[2]);
      continue;
    }
    const nameOnly = /^\s*-\s+(\S+)\s*$/.exec(line);
    if (nameOnly) out.set(nameOnly[1], "");
  }
  return out;
}

/** Liest den Hub-Index: alle Paketnamen + jeweils höchste Version. */
export async function fetchHubIndex(): Promise<HubPackage[]> {
  let res: Response;
  try {
    res = await fetch(TREE_URL, { headers: { Accept: "application/vnd.github+json" } });
  } catch (e) {
    throw new AppError("AI-1958-NET", `Hub nicht erreichbar: ${e}`);
  }
  if (!res.ok) {
    // Rate-Limit ist ein eigener Zustand: warten hilft, erneut sofort nicht.
    const code = res.status === 403 || res.status === 429 ? "AI-1969-LIMIT" : "AI-1958-NET";
    throw new AppError(code, `Hub-Index nicht erreichbar (HTTP ${res.status}).`);
  }
  const data = await res.json();
  const tree: { path: string }[] = data.tree || [];
  const latest = new Map<string, string>();
  for (const node of tree) {
    const m = node.path.match(/^packages\/([^/]+)\/([^/]+)\/_manifest\.yml$/);
    if (!m) continue;
    const [, name, version] = m;
    const cur = latest.get(name);
    if (!cur || cmpVersion(version, cur) > 0) latest.set(name, version);
  }
  return [...latest.entries()]
    .map(([name, version]) => ({ name, version }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

/** Minimal-Parser für die flachen espanso-Manifeste (title/description/…). */
export function parseManifest(text: string): Partial<HubPackage> {
  const out: Partial<HubPackage> = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const ci = line.indexOf(":");
    if (ci < 0) continue;
    const key = line.slice(0, ci).trim();
    let val = line.slice(ci + 1).trim();
    if (key === "tags") {
      const inner = val.replace(/^\[|\]$/g, "");
      out.tags = inner
        .split(",")
        .map((t) => t.trim().replace(/^["']|["']$/g, ""))
        .filter(Boolean);
      continue;
    }
    val = val.replace(/^["']|["']$/g, "");
    if (key === "title") out.title = val;
    else if (key === "description") out.description = val;
    else if (key === "author") out.author = val;
    else if (key === "homepage") out.homepage = val;
    else if (key === "version") out.version = val;
  }
  return out;
}

/** Lädt Detail-Metadaten eines Pakets (lazy, über die raw-CDN). */
export async function fetchManifest(
  name: string,
  version: string
): Promise<Partial<HubPackage>> {
  const url = `${RAW}/packages/${name}/${version}/_manifest.yml`;
  const res = await fetch(url);
  if (!res.ok) return {};
  return parseManifest(await res.text());
}
