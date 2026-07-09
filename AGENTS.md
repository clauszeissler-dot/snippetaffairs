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
| `espanso match exec` | **0** — trotz „Worker process is not running" | nein, Ausgabe prüfen (`exec_failed`) |
| `espanso service check` | 0 — Zustand nur im Klartext | nein, Text auswerten |
| `espanso install` | 2 | **ja** |
| `espanso uninstall` | 3 | **ja** |
| `espanso package update` | 5 | **ja** |

Bei jedem neuen CLI-Aufruf **einmal mit falschem Argument ausprobieren** und den Exit-Code
notieren, statt ihn anzunehmen. Die Tabelle oben ist so entstanden.

## 5. Destruktive Datei-Operationen liegen hinter `ensure_within`

`delete_match_file`, `rename_match_file`, `restore_backup` und `list_backups` prüfen über
`ensure_within(path, match_dir())`, dass der Pfad wirklich im match-Ordner liegt (kanonisiert,
Symlinks aufgelöst). Neue Datei-Operationen genauso absichern — auch wenn der Pfad „ja aus
unserer eigenen UI kommt".

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

## 9. Tests und Linter

```bash
bun run lint                                        # ESLint (0 Fehler)
bun run test                                        # 28 Tests: Fehler-Resolver, Hub-Parser, cmpVersion
bun run build                                       # tsc + vite
cd src-tauri && cargo clippy --all-targets -- -D warnings
cd src-tauri && cargo test                          # 24 Tests: Datenintegrität, Pfad-Schutz, IPC-Naht
```

Alle fünf laufen in der CI (`.github/workflows/test.yml`) und müssen grün sein, bevor ein
Release getaggt wird. Clippy läuft dort mit `-D warnings`.

Zwei bewusste ESLint-Ausnahmen (`react-hooks/set-state-in-effect` in `App.tsx` und
`Diagnostics.tsx`): die Regel durchdringt die `async`-Grenze nicht — dort wird State erst
nach `await` gesetzt, nicht synchron im Effekt.
