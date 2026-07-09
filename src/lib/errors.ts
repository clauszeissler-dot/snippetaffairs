/**
 * Fehlercode-Auflösung nach dem KI-AffAIrs-Standard `errorcodebase`.
 *
 * Das Rust-Backend liefert Fehler als `ECB:<code>|<detail>`. Hier wird der Code
 * gegen die vendorte Registry aufgelöst und in eine user-taugliche Anzeige
 * übersetzt (freundlicher Titel + Klartext + Referenz + Aktionen).
 *
 * Warum vendored statt `@ki-affairs/errorcodebase`: SnippetAffAIrs ist ein
 * öffentliches Repo mit öffentlicher CI, das Standard-Repo ist privat. Ein
 * Git-Install wäre für Fremd-Builds nicht auflösbar. `registry.json` wird
 * deshalb ins Repo kopiert — siehe AGENTS.md.
 *
 * Codes werden NIE hier erfunden, nur aus der Registry gelesen.
 */
import registryJson from "./errorcodebase-registry.json";

export const FALLBACK_CODE = "AI-1956-CORE";

/** Aktionen, die diese App tatsächlich ausführen kann. */
export type ActionId = "retry" | "reload" | "report" | "copy" | "back";

const ACTION_LABELS: Record<ActionId, string> = {
  retry: "Erneut versuchen",
  reload: "Neu laden",
  report: "Fehler melden",
  copy: "Details kopieren",
  back: "Schließen",
};

const KNOWN_ACTIONS = Object.keys(ACTION_LABELS) as ActionId[];

interface Localized {
  title: string;
  message: string;
  actions: string[];
}

export interface ErrorEntry {
  code: string;
  internal: string;
  category: string;
  severity: "info" | "warning" | "error";
  retryable: boolean;
  i18n: Record<string, Localized>;
}

interface Registry {
  version: string;
  count: number;
  codes: ErrorEntry[];
}

const registry = registryJson as unknown as Registry;

const BY_CODE = new Map<string, ErrorEntry>(registry.codes.map((e) => [e.code, e]));

/** Registry-Eintrag oder `undefined` — ohne Fallback. */
export function getError(code: string): ErrorEntry | undefined {
  return BY_CODE.get(code);
}

/** Registry-Eintrag mit garantiertem Fallback auf `AI-1956-CORE`. */
export function resolve(code: string | undefined | null): ErrorEntry {
  const hit = code ? BY_CODE.get(code) : undefined;
  return hit ?? BY_CODE.get(FALLBACK_CODE)!;
}

/** Fehler, den das Frontend selbst wirft (z. B. Netzwerkfehler im Hub). */
export class AppError extends Error {
  constructor(
    public readonly code: string,
    public readonly detail: string
  ) {
    super(`${code}: ${detail}`);
    this.name = "AppError";
  }
}

export interface ParsedError {
  code: string;
  /** Technische Zusatzinfo für die Anzeige — geht NIE in einen Report (kann Pfade enthalten). */
  detail: string;
}

/**
 * Nimmt alles entgegen, was in einem `catch` landen kann, und macht daraus
 * einen Registry-Code + Detailtext. Unbekanntes wird zum Fallback-Code.
 */
export function parseError(e: unknown): ParsedError {
  if (e instanceof AppError) {
    return { code: resolve(e.code).code, detail: e.detail };
  }
  const raw = e instanceof Error ? e.message : String(e);
  const match = /^ECB:([A-Z0-9-]+)\|([\s\S]*)$/.exec(raw.trim());
  if (match) {
    // Unbekannter Code → Fallback, aber Detailtext behalten.
    return { code: resolve(match[1]).code, detail: match[2] };
  }
  return { code: FALLBACK_CODE, detail: raw };
}

/** `trace_YYYYMMDD_` + 12 Hex-Zeichen — Format des errorcodebase-Standards. */
export function createTraceId(now: Date = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  const bytes = new Uint8Array(6);
  crypto.getRandomValues(bytes);
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `trace_${y}${m}${d}_${hex}`;
}

export interface Report {
  traceId: string;
  code: string;
  internal: string;
  category: string;
  severity: string;
  appVersion: string;
  route: string;
  occurredAt: string;
}

/**
 * PII-sicherer Report: enthält ausschließlich Registry-Metadaten + Kontext.
 * Der `detail`-Text wird bewusst NICHT übernommen — er kann Dateipfade und
 * damit den Benutzernamen enthalten.
 */
export function createReport(
  code: string,
  ctx: { appVersion: string; route: string },
  now: Date = new Date()
): Report {
  const entry = resolve(code);
  return {
    traceId: createTraceId(now),
    code: entry.code,
    internal: entry.internal,
    category: entry.category,
    severity: entry.severity,
    appVersion: ctx.appVersion,
    route: ctx.route,
    occurredAt: now.toISOString(),
  };
}

export interface UserFacingError {
  title: string;
  message: string;
  /** z. B. „AI-2017-CTX · trace_20260709_a1b2c3d4e5f6" */
  reference: string;
  actions: { id: ActionId; label: string }[];
  severity: "info" | "warning" | "error";
  retryable: boolean;
}

/**
 * Baut die Anzeige. Buttons kommen aus dem `actions`-Feld der Registry —
 * nie hartkodiert, sonst zeigt ein nicht-retryabler Fehler „Erneut versuchen".
 * Aktionen, die diese App nicht ausführen kann (login, updatePayment, wait),
 * werden übersprungen.
 */
export function formatUser(
  code: string,
  opts: { locale?: string; traceId?: string } = {}
): UserFacingError {
  const entry = resolve(code);
  const locale = opts.locale ?? "de";
  const loc = entry.i18n[locale] ?? entry.i18n["de"];
  const reference = opts.traceId ? `${entry.code} · ${opts.traceId}` : entry.code;

  const actions = loc.actions
    .filter((a): a is ActionId => (KNOWN_ACTIONS as string[]).includes(a))
    .map((id) => ({ id, label: ACTION_LABELS[id] }));

  return {
    title: loc.title,
    message: loc.message,
    reference,
    actions,
    severity: entry.severity,
    retryable: entry.retryable,
  };
}
