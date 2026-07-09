# AGENTS.md — Arbeitsregeln für dieses Repo

SnippetAffAIrs ist eine Tauri-v2-Desktop-App (Rust-Backend + React/TS-Frontend), die
espansos YAML-Match-Dateien liest und schreibt. Wer hier Code ändert, hält sich an
folgende Punkte.

## 1. Tauri-Command-Argumente heißen camelCase

`#[tauri::command]` erwartet die Argumente per Default in **camelCase**, obwohl die
Rust-Signatur snake_case ist:

```rust
pub fn save_snippet(file_path: String, expected_trigger: Option<String>) -> ...
```

```ts
invoke("save_snippet", { filePath, expectedTrigger }); // ✅
invoke("save_snippet", { file_path, expected_trigger }); // ❌ "missing required key filePath"
```

Der Fehler tritt **erst zur Laufzeit** auf — TypeScript und `cargo build` sind grün.
In v0.1.0 waren Speichern und Löschen dadurch komplett kaputt.

`src-tauri/src/ipc_tests.rs` ruft die Commands über die echte IPC-Schicht auf und hält
diese Konvention fest. **Neue Commands mit mehrwortigen Argumenten dort ergänzen.**

## 2. Fehler folgen dem errorcodebase-Standard

Master: `github.com/clauszeissler-dot/errorcodebase` (privat) · Skill `ki-affairs-error-codes`.

- **Codes nie erfinden.** Nur Codes aus `src/lib/errorcodebase-registry.json`.
- Das Backend gibt Fehler als `ECB:<code>|<detailtext>` zurück (`ecb()` in `espanso.rs`).
- Das Frontend löst sie in `src/lib/errors.ts` auf: `parseError` → `formatUser` → `ErrorDialog`.
- **Buttons kommen aus dem `actions`-Feld der Registry**, nie hartkodiert — sonst zeigt
  ein nicht-retryabler Fehler „Erneut versuchen".
- **Reports sind PII-frei** (`createReport`): kein Detailtext, keine Dateipfade, keine
  Snippet-Inhalte. Der Detailtext wird nur lokal im Dialog angezeigt.
- Fallback ist `AI-1956-CORE`, automatisch über `resolve()`.

Genutzte Codes:

| Code | Wann |
|---|---|
| `AI-2011-FIND` | espanso-Binary oder Config nicht gefunden |
| `AI-1955-INPUT` | ungültige Eingabe (leerer Trigger, Dateiname, schreibgeschütztes Match) |
| `AI-2017-CTX` | Liste veraltet — Datei extern geändert (Staleness-Guard) |
| `AI-1956-CORE` | IO-, YAML- und sonstige interne Fehler |
| `AI-2016-FLOW` | Engine-Aktion fehlgeschlagen (Service, Paket-Install) |
| `AI-1958-NET` / `AI-1969-LIMIT` | Hub nicht erreichbar / GitHub-Rate-Limit |

Die Registry ist **vendored** statt als npm-Dependency eingebunden: Dieses Repo ist
öffentlich, das Standard-Repo privat — ein Git-Install wäre in der öffentlichen CI und
für Fremd-Builds nicht auflösbar. Bei Registry-Änderungen `dist/registry.json` neu
kopieren.

## 3. Datenverlust ist der schlimmste Bug

Die App schreibt fremde Nutzerdateien. Deshalb gilt in `espanso.rs`:

- **Round-Trip-sicher:** unbekannte Felder liegen in `extra` (`#[serde(flatten)]`) und
  überleben jeden Schreibvorgang. Test: `round_trip_preserves_vars`.
- **Erweiterte Matches** (`vars`, `form`, `regex`, `image_path`, `triggers`) sind
  schreibgeschützt — im Frontend *und* im Backend (`is_advanced`).
- **Staleness-Guard:** `save_snippet`/`delete_snippet` bekommen `expectedTrigger` und
  brechen ab, wenn der Index nicht mehr auf das angezeigte Snippet zeigt.
- **Atomar schreiben:** temp + rename, vorher Re-Parse-Validierung.
- **Backups:** `.yml.bak` (letzter Stand) und einmalig `.yml.orig` (Original mit
  Kommentaren — serde_yaml kann Kommentare nicht erhalten).

Wer diese Garantien anfasst, ergänzt einen Test in `espanso.rs`.

## 4. Die espanso-CLI lügt über ihren Exit-Code — aber nicht überall

Verifiziert (espanso 2.3.0):

| Befehl | Exit-Code im Fehlerfall | Verlässlich? |
|---|---|---|
| `espanso --version` | **1** — obwohl erfolgreich | nein, stdout parsen |
| `espanso match exec` | **0** — trotz „Worker process is not running" | nein, Ausgabe prüfen (`cli_failed`) |
| `espanso service check` | 0 — Zustand nur im Klartext | nein, Text auswerten |
| `espanso install` | 2 | **ja** |
| `espanso uninstall` | 3 | **ja** |
| `espanso package update` | 5 | **ja** |
| `espanso stop` (läuft nicht) | 4 | **ja** |
| `espanso status` (läuft nicht) | 4 | **ja** |
| `espanso service register` (schon registriert) | 0 + „registered correctly" | idempotent |
| `espanso start` | **ungemessen** | unbelegt → `cli_failed` |
| `espanso service unregister` | **ungemessen** (destruktiv) | unbelegt |
| `espanso workaround secure-input` | **ungemessen** (verändert System) | unbelegt → `cli_failed` |

Für die ungemessenen Befehle gilt `cli_failed()`: es prüft `success` **und** die Ausgabe auf
„unable to"/„error". Lieber ein Fehlerdialog zu viel als ein Erfolg, den es nicht gab.

Bei jedem neuen CLI-Aufruf **einmal mit falschem Argument ausprobieren** und den Exit-Code
notieren, statt ihn anzunehmen. Die Tabelle oben ist so entstanden.

## 4a. Kein `cmd /C`, keine Shell — jedes CLI-Argument wird validiert

Die Engine-CLI wird **immer direkt** gespawnt (`Command::new(bin)`), auch unter Windows für
`.bat`/`.cmd`-Shims. **Nie `cmd /C`** davorsetzen: Rusts Batch-Argument-Escaping (Fix für
CVE-2024-24576, vollständig ab Rust 1.81) greift **nur**, wenn das gestartete Programm SELBST
die `.bat`/`.cmd`-Datei ist. Bei `cmd /C <shim> <args>` ist das Programm cmd.exe — Metazeichen
(`& | ^ > < " %`) in den Argumenten blieben ungeschützt und würden von cmd.exe interpretiert
(**Command Injection, CWE-78**). Deshalb steht die MSRV in `Cargo.toml` auf `rust-version = "1.81"`,
damit ältere Toolchains ohne dieses Escaping nicht still unsicher bauen.

Zweite Schicht: **jedes** an die CLI gereichte Argument aus fremder Quelle wird **vor** dem
Aufruf validiert (`espanso.rs`):

- **Paketnamen** (`package_install`/`package_uninstall`/`package_update`) → `validate_package_name`,
  erlaubt nur `[A-Za-z0-9._-]`. Quelle: espanso-Hub (GitHub-Pfad).
- **Trigger** (`match_exec`) → `validate_trigger_chars`, lehnt `& | ^ > < " %`, `\r`, `\n` ab.
  Bewusst **keine** Whitelist — Trigger dürfen legitim `: ! ? #` tragen. Quelle: fremde YAML-Dateien.

Jeden neuen CLI-Aufruf mit Argumenten aus fremder Quelle genauso absichern.

## 5. Destruktive Datei-Operationen liegen hinter `ensure_within`

`delete_match_file`, `rename_match_file`, `restore_backup`, `list_backups` **sowie die
schreibenden `save_snippet`/`delete_snippet`** prüfen über `ensure_within(path, match_dir())`,
dass der Pfad wirklich im match-Ordner liegt (kanonisiert, Symlinks aufgelöst). Bei
`save_snippet`/`delete_snippet` liegt die Prüfung in den inneren Funktionen `save_snippet_in`/
`delete_snippet_in` (der Command ermittelt `match_dir()` und delegiert; die Tests übergeben
einen Temp-Ordner als Basis — kein `#[cfg(test)]`-Sonderweg im Produktivcode). Neue
Datei-Operationen genauso absichern — auch wenn der Pfad „ja aus unserer eigenen UI kommt".

## 6. Einfache Variablen vs. Handarbeit

Der Editor kann `{{date}}` und `{{clipboard}}` einfügen; beim Speichern erzeugt das Backend
den `vars`-Block neu. Das gilt **nur** für exakt dieses Schema (`is_simple_var`). Jede andere
Variable — anderer Name, anderer Typ, zusätzliche Felder — macht das Match `advanced` und
damit schreibgeschützt. Diese Grenze nicht aufweichen: dahinter liegt Handarbeit des Nutzers,
die ein Rewrite zerstören würde.

## 7. White-Label

„espanso" erscheint in der UI **nur** im Installationshinweis. Sonst: „Engine",
„Text-Expander-Engine". Auch in Fehlertexten.

## 8. Marke und Logo sind von der Lizenz ausgenommen

Der Code steht unter CC BY 4.0, das Logo (`src/components/Logo.tsx`, `src-tauri/icons/`) und
die Namen „KI AffAIrs" / „SnippetAffAIrs" nicht — siehe LICENSE. Das Monogramm ist 1:1 aus dem
offiziellen Logopaket übernommen, nicht nachgezeichnet. Es wird nie generiert oder verändert.

## 9. Definition of Done

```bash
bun run verify    # ESLint · Vitest · tsc+vite · Clippy -D warnings · cargo test · Versionsabgleich
```

Eine Kette, ein Befehl. Sie läuft an drei Stellen, damit sie nicht vom Gedächtnis abhängt:

1. **lokal vor jedem Push** — `.githooks/pre-push` (aktivieren mit
   `git config core.hooksPath .githooks`; Notausgang: `git push --no-verify`),
2. **in der CI** bei jedem Push und PR (`.github/workflows/test.yml`, ubuntu + macOS),
3. **von Hand** vor einem Release-Tag.

Der Verifier-Durchgang für alles, was kein Linter sieht — Nähte, Exit-Codes, Parser,
Datenverlust, toter Code — steht in **`REVIEW.md`** und wird mit `/pruefen` ausgelöst.
Jede Zeile dort steht, weil sie in diesem Repo schon einmal einen echten Fehler gefunden hat.

Zwei bewusste ESLint-Ausnahmen (`react-hooks/set-state-in-effect` in `App.tsx` und
`Diagnostics.tsx`): die Regel durchdringt die `async`-Grenze nicht — dort wird State erst
nach `await` gesetzt, nicht synchron im Effekt.

## 10. Content Security Policy (CSP) ist eng — nicht heimlich aufweichen

`app.security.csp` in `src-tauri/tauri.conf.json` ist bewusst restriktiv. Sie erlaubt **nur
das, was im Code nachweislich gebraucht wird**. Wer einen neuen `fetch`, eine neue externe
Ressource (Font, Bild, Skript, WebSocket) einbaut, **muss die CSP dort mit erweitern** —
sonst blockt die App die Anfrage stumm zur Laufzeit (sichtbar nur in der Devtools-Konsole).

Produktions-CSP (`csp`) — jede Direktive mit Grund:

| Direktive | Wert | Warum |
|---|---|---|
| `default-src` | `'self'` | restriktiver Fallback für alles Nicht-Aufgeführte |
| `script-src` | `'self'` | Prod-`index.html` lädt nur ein gebündeltes `src`-Modul; Tauri hängt zur Compile-Zeit Nonce/Hash für sein IPC-Bootstrap-Skript selbst an. **Kein `'unsafe-inline'`** — bewusst. |
| `style-src` | `'self' 'unsafe-inline'` | React setzt `style={{…}}` zur Laufzeit; `'unsafe-inline'` deckt das defensiv ab. Kein Sicherheitsloch für JS (CSS führt keinen Code aus), da Exfiltration über `connect-src`/`img-src`/`font-src` eng bleibt. |
| `img-src` | `'self' data:` | nur gebündelte Assets; `data:` als kleine Reserve für inline/gebündelte Kleinbilder. Keine externen Bild-Hosts nötig. |
| `font-src` | `'self' data:` | **@fontsource ist gebündelt** (`src/main.tsx`). Vite legt große Font-Subsets als `/assets/*.woff2` (`'self'`) ab **und inlined kleine als `data:`** — beides nötig, sonst fehlen Zeichensätze. Kein CDN. |
| `connect-src` | `'self' ipc: http://ipc.localhost https://api.github.com https://raw.githubusercontent.com` | `ipc:`/`http://ipc.localhost` = Tauri-v2-`invoke` (offizielle Doku). Die beiden GitHub-Hosts sind der espanso-Hub (`src/lib/hub.ts`: Tree-API + raw-CDN). |
| `object-src` | `'none'` | keine Plugins/`<object>` |
| `base-uri` | `'self'` | verhindert `<base>`-Injection |

**Externe Links** (arxiv, wikipedia, … aus `errorcodebase-registry.json`) öffnen über
`@tauri-apps/plugin-opener` im Systembrowser (IPC), **nicht** im WebView — sie brauchen
darum **keinen** CSP-Host. Wer künftig eine URL im WebView selbst lädt (`<iframe>`, direkte
Navigation, `fetch`), muss die CSP entsprechend erweitern.

**Dev-CSP (`devCsp`) ist absichtlich lockerer** und gilt nur unter `bun run tauri dev`
(laut Tauri-Schema greift sonst `csp` auch im Dev). Sie erlaubt zusätzlich
`'unsafe-inline' 'unsafe-eval'` für Skripte (Vite/React-Refresh injizieren ein Inline-
Preamble-Skript) und `ws://localhost:*`/`http://localhost:*` für den HMR-WebSocket. Die
**erlaubte Host-Liste (GitHub-Hub, `ipc:`) ist in beiden identisch** — der Dev-Rauchtest
prüft also dieselbe Freigabe wie die Produktion. Diese Lockerung **nie** in `csp`
übernehmen.
