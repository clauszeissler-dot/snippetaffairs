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

function cmpVersion(a: string, b: string): number {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** Liest den Hub-Index: alle Paketnamen + jeweils höchste Version. */
export async function fetchHubIndex(): Promise<HubPackage[]> {
  const res = await fetch(TREE_URL, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!res.ok) {
    throw new Error(
      `Hub-Index nicht erreichbar (HTTP ${res.status}). GitHub-Rate-Limit? Später erneut versuchen.`
    );
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
function parseManifest(text: string): Partial<HubPackage> {
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
