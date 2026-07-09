import { describe, expect, it } from "vitest";
import { cmpVersion, parseInstalledPackages } from "./hub";

// Verifiziertes Ausgabeformat von `espanso package list`.
const REAL_OUTPUT = `- all-emojis - version: 0.2.0 (espanso-hub)
- basic-html - version: 0.1.0 (espanso-hub)
- typofixer-de - version: 1.2.3 (espanso-hub)`;

describe("parseInstalledPackages", () => {
  it("liest Name und Version je Zeile", () => {
    const m = parseInstalledPackages(REAL_OUTPUT);
    expect(m.size).toBe(3);
    expect(m.get("all-emojis")).toBe("0.2.0");
    expect(m.get("typofixer-de")).toBe("1.2.3");
  });

  it("liefert eine leere Map bei leerer Ausgabe", () => {
    expect(parseInstalledPackages("").size).toBe(0);
    expect(parseInstalledPackages("\n\n").size).toBe(0);
  });

  it("ignoriert Kopfzeilen und Freitext", () => {
    const m = parseInstalledPackages("Installed packages:\n\n- foo - version: 1.0.0\n");
    expect([...m.keys()]).toEqual(["foo"]);
  });

  it("markiert NICHT installierte Pakete nicht als installiert", () => {
    // Der frühere Substring-Regex meldete ein Paket namens `version` als
    // installiert, sobald irgendein Paket in der Liste stand.
    const m = parseInstalledPackages(REAL_OUTPUT);
    expect(m.has("version")).toBe(false);
    expect(m.has("espanso-hub")).toBe(false);
    expect(m.has("html")).toBe(false); // Teilstring von "basic-html"
    expect(m.has("emojis")).toBe(false); // Teilstring von "all-emojis"
  });

  it("übernimmt Zeilen ohne Version tolerant (Formatdrift)", () => {
    const m = parseInstalledPackages("- nur-name\n");
    expect(m.get("nur-name")).toBe("");
  });
});

describe("cmpVersion", () => {
  it("erkennt neuere Versionen", () => {
    expect(cmpVersion("1.0.1", "1.0.0")).toBeGreaterThan(0);
    expect(cmpVersion("1.1.0", "1.0.9")).toBeGreaterThan(0);
    expect(cmpVersion("2.0.0", "1.9.9")).toBeGreaterThan(0);
  });

  it("erkennt Gleichstand — sonst würde die App dauerhaft Updates behaupten", () => {
    expect(cmpVersion("1.2.3", "1.2.3")).toBe(0);
    expect(cmpVersion("1.2", "1.2.0")).toBe(0);
  });

  it("erkennt ältere Versionen", () => {
    expect(cmpVersion("1.0.0", "1.0.1")).toBeLessThan(0);
  });

  it("verkraftet unsaubere Versionsangaben", () => {
    expect(cmpVersion("", "")).toBe(0);
    expect(cmpVersion("1.0.0", "")).toBeGreaterThan(0);
  });

  it("ignoriert ein führendes v — sonst gäbe es ein Update, das keines ist", () => {
    expect(cmpVersion("v1.2.3", "1.2.3")).toBe(0);
    expect(cmpVersion("1.2.3", "v1.2.3")).toBe(0);
    expect(cmpVersion("v1.3.0", "v1.2.9")).toBeGreaterThan(0);
  });
});
