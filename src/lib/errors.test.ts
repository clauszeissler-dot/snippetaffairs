import { describe, expect, it } from "vitest";
import {
  AppError,
  FALLBACK_CODE,
  createReport,
  createTraceId,
  formatUser,
  getError,
  parseError,
  resolve,
} from "./errors";

describe("parseError", () => {
  it("liest Code und Detail aus dem Backend-Format", () => {
    const r = parseError("ECB:AI-2017-CTX|Die Liste ist veraltet.");
    expect(r.code).toBe("AI-2017-CTX");
    expect(r.detail).toBe("Die Liste ist veraltet.");
  });

  it("verkraftet mehrzeilige Details (espanso-Ausgaben)", () => {
    const r = parseError("ECB:AI-1956-CORE|Zeile 1\nZeile 2");
    expect(r.code).toBe("AI-1956-CORE");
    expect(r.detail).toBe("Zeile 1\nZeile 2");
  });

  it("fällt bei unbekanntem Code auf AI-1956-CORE zurück, behält aber das Detail", () => {
    const r = parseError("ECB:AI-9999-FAKE|irgendwas");
    expect(r.code).toBe(FALLBACK_CODE);
    expect(r.detail).toBe("irgendwas");
  });

  it("behandelt rohe Strings ohne Präfix als internen Fehler", () => {
    expect(parseError("kaputt").code).toBe(FALLBACK_CODE);
    expect(parseError(new Error("boom")).detail).toBe("boom");
  });

  it("erkennt AppError aus dem Frontend", () => {
    const r = parseError(new AppError("AI-1958-NET", "kein Netz"));
    expect(r.code).toBe("AI-1958-NET");
    expect(r.detail).toBe("kein Netz");
  });
});

describe("resolve", () => {
  it("liefert echte Einträge", () => {
    expect(resolve("AI-2011-FIND").internal).toBe("RESOURCE_NOT_FOUND");
  });

  it("fällt auf den Fallback zurück, nie auf undefined", () => {
    expect(resolve("gibts-nicht").code).toBe(FALLBACK_CODE);
    expect(resolve(null).code).toBe(FALLBACK_CODE);
  });

  it("getError meldet Unbekanntes ehrlich als undefined", () => {
    expect(getError("gibts-nicht")).toBeUndefined();
  });

  it("kennt alle Codes, die das Rust-Backend sendet", () => {
    for (const code of ["AI-2011-FIND", "AI-1955-INPUT", "AI-2017-CTX", "AI-1956-CORE"]) {
      expect(getError(code), code).toBeDefined();
    }
  });
});

describe("createTraceId", () => {
  it("folgt dem Standardformat trace_YYYYMMDD_ + 12 Hex", () => {
    const id = createTraceId(new Date(2026, 6, 9));
    expect(id).toMatch(/^trace_20260709_[0-9a-f]{12}$/);
  });

  it("ist pro Aufruf verschieden", () => {
    expect(createTraceId()).not.toBe(createTraceId());
  });
});

describe("createReport", () => {
  const report = createReport("AI-2017-CTX", { appVersion: "0.1.1", route: "snippets" });

  it("enthält Registry-Metadaten und Kontext", () => {
    expect(report.code).toBe("AI-2017-CTX");
    expect(report.internal).toBe("CONTEXT_STATE_LOST");
    expect(report.appVersion).toBe("0.1.1");
    expect(report.route).toBe("snippets");
    expect(report.traceId).toMatch(/^trace_\d{8}_[0-9a-f]{12}$/);
  });

  it("nimmt keinen Detailtext auf — dort können Dateipfade stehen", () => {
    const serialized = JSON.stringify(report);
    expect(serialized).not.toContain("/Users/");
    expect(Object.keys(report)).not.toContain("detail");
  });
});

describe("formatUser", () => {
  it("baut Referenz aus Code und traceId", () => {
    const ui = formatUser("AI-2017-CTX", { traceId: "trace_20260709_abcdef123456" });
    expect(ui.reference).toBe("AI-2017-CTX · trace_20260709_abcdef123456");
  });

  it("leitet Buttons aus der Registry ab, nicht hartkodiert", () => {
    // AI-1955-INPUT ist nicht retryable → kein „Erneut versuchen".
    const input = formatUser("AI-1955-INPUT");
    expect(input.retryable).toBe(false);
    expect(input.actions.map((a) => a.id)).not.toContain("retry");

    // AI-2017-CTX ist der Staleness-Fall → „Neu laden" muss dabei sein.
    const stale = formatUser("AI-2017-CTX");
    expect(stale.actions.map((a) => a.id)).toContain("reload");
  });

  it("filtert Aktionen weg, die diese App nicht ausführen kann", () => {
    // AI-1969-LIMIT bietet laut Registry „wait" an — dafür gibt es keinen Button.
    const limit = formatUser("AI-1969-LIMIT");
    expect(limit.actions.map((a) => a.id)).not.toContain("wait");
    expect(limit.actions.map((a) => a.id)).toContain("retry");
  });

  it("liefert deutschen Text und kennt Englisch", () => {
    expect(formatUser("AI-2011-FIND").title).toBe("Nichts gefunden.");
    expect(formatUser("AI-2011-FIND", { locale: "en" }).title).toBe("Nothing found.");
  });

  it("fällt bei fehlender Sprache auf Deutsch zurück", () => {
    expect(formatUser("AI-2011-FIND", { locale: "fr" }).title).toBe("Nichts gefunden.");
  });
});
