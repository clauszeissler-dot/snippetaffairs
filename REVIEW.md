# Verifier-Checkliste

Die Checkliste für den adversarialen Prüfdurchgang (`/pruefen`), nach dem
Code-Security-Quality-Rezept der [Loop Library](https://github.com/clauszeissler-dot/loopAffAIrs):
Builder → **Verifier** → Loop-Controller.

**Jede Zeile hier steht, weil sie in diesem Repo schon einmal einen echten Fehler gefunden hat.**
Keine generischen „achte auf Qualität"-Punkte. Wer einen neuen Fehlertyp findet, ergänzt eine Zeile.

Der Verifier fragt „**was stimmt hier nicht?**", nicht „ist das gut?". Ein Punkt gilt erst als
geprüft, wenn ein Kommando-Output oder ein Test ihn belegt — nicht, wenn der Code plausibel aussieht.

---

## Severity

| Stufe | Bedeutung | Blockiert? |
|---|---|---|
| **CRITICAL** | Datenverlust, stiller Fehlschlag, Sicherheitsloch | ja |
| **HIGH** | Funktion tut nicht, was sie meldet | ja |
| **MEDIUM** | Falsche Anzeige, toter Code, fehlender Test | nein, wird dokumentiert |
| **LOW** | Stil, Formulierung | nein |

**Stop-Bedingung:** Quality-Streak N=2 (zwei aufeinanderfolgende Durchläufe ohne CRITICAL/HIGH),
Fixed Cap N=3 Runden. Wird der Cap erreicht, ohne dass die Schwelle unterschritten wird:
**Eskalation an den Menschen**, kein stiller Weiterbau.

---

## 1. Nähte — CRITICAL

Jede Prozess-, Sprach- oder Systemgrenze (Tauri-IPC, CLI-Aufruf, HTTP, Worker, MCP).

- [ ] Gibt es einen Test, der die Grenze **real durchquert**, mit exakt dem Payload des echten Clients?
- [ ] Assertet der Test auf den **Effekt** (Datei geschrieben, Zeile in der DB) — nicht nur auf „kein Fehler"?
- [ ] Gibt es einen **Negativtest** (falscher Payload ⇒ muss scheitern)?
- [ ] Wenn ein Test grün ist: ist er aus dem **richtigen Grund** grün?

> Gefunden: v0.1.0 konnte nicht speichern. `#[tauri::command]` erwartet `filePath`, gesendet
> wurde `file_path`. `cargo test` rief die Funktionen direkt auf und übersprang die Naht.
> Ein Zwischenstand des IPC-Tests war grün, weil er still an der ACL scheiterte — der
> Negativtest deckte das auf.

## 2. Fremde Kommandos — CRITICAL

- [ ] Wurde der Befehl **einmal mit falschem Argument ausgeführt** und der Exit-Code notiert?
- [ ] Steht das Ergebnis in der Tabelle in `AGENTS.md` §4?
- [ ] Wird bei unzuverlässigem Exit-Code die **Ausgabe** ausgewertet?
- [ ] Wird die CLI **direkt** gespawnt (kein `cmd /C`) und **jedes** Argument aus fremder Quelle **vor** dem Aufruf validiert?
- [ ] Arbeitet der Befehl mit einem **Cache**, der veralten kann? Gibt es eine Option, ihn zu umgehen?

> Gefunden: `espanso match exec` liefert Exit-Code **0**, auch bei „Worker process is not
> running". `espanso --version` liefert **1**, obwohl erfolgreich. `install`/`uninstall`/
> `package update` sind dagegen ehrlich — beides nur durch Messen feststellbar.
>
> Gefunden (im echten Betrieb): `espanso install` nutzt ohne `--refresh-index` einen
> zwischengespeicherten Hub-Index. Ist der veraltet, scheitert die Installation mit
> „signature mismatch" — einer Meldung, die die Ursache verschweigt.
>
> Gefunden: `espanso_command` baute unter Windows `cmd /C <shim>` — damit greift Rusts
> Batch-Escaping (Fix für CVE-2024-24576, ab Rust 1.81) NICHT, denn das Programm ist dann
> cmd.exe. Metazeichen (`& | ^ > < " %`) in Paketnamen (Hub) bzw. Triggern (fremde YAML) wären
> Command Injection (**CWE-78**). Fix: direkter Spawn (`Command::new(bin)`) + MSRV `1.81` +
> `validate_package_name`/`validate_trigger_chars` vor jedem CLI-Aufruf.

## 3. Erfolgsmeldungen — HIGH

- [ ] Stammt jedes „erfolgreich", jeder grüne Toast aus **beobachtetem Zustand**?
- [ ] Oder nur daraus, dass kein Fehler geworfen wurde?

> Grundsatz aus CLAUDE.md: Jede Erfolgsmeldung ist eine Hypothese, bis der echte Output sie belegt.

## 4. Parser — HIGH

Jeder Parser fremder Ausgaben (CLI-Text, YAML, Manifest, Version).

- [ ] Gegen **echte** Ausgabe getestet, nicht gegen erfundene?
- [ ] Negativfälle: Teilstring-Treffer, leere Eingabe, Formatdrift, Präfixe?

> Gefunden: Die alte Hub-Heuristik meldete ein Paket namens `version` als installiert, sobald
> irgendeines installiert war. `cmpVersion("v1.2.3", "1.2.3")` ergab „älter" → die App hätte
> ein Update angeboten, das keines ist.

## 5. Datenverlust — CRITICAL

Die App schreibt fremde Nutzerdateien.

- [ ] Round-Trip: überleben unbekannte Felder (`extra`) jeden Schreibvorgang?
- [ ] Schreibgeschützte Fälle (`is_advanced`) im **Backend** durchgesetzt, nicht nur in der UI?
- [ ] Atomar geschrieben (temp + rename), vorher re-parsed?
- [ ] Staleness-Guard: kann ein veralteter Index ein fremdes Snippet überschreiben?
- [ ] Backup vorhanden **und** wiederherstellbar?

## 6. Pfade — CRITICAL

- [ ] Jede destruktive Datei-Operation hinter `ensure_within(path, match_dir())`?
- [ ] Kanonisiert (Symlinks aufgelöst), `..`-Traversal getestet?
- [ ] Auch die **schreibenden** Commands (`save_snippet`, `delete_snippet`) — nicht nur die löschenden/umbenennenden?

> Gefunden: `save_snippet`/`delete_snippet` nahmen `file_path` ungeprüft vom Frontend, während
> `ensure_within` nur bei rename/delete/list/restore griff. Ein Symlink `match/x.yml → ~/.zshrc`
> hätte beim Speichern nach außen durchgeschrieben (**CWE-22**, Path Traversal). Fix: Logik in
> `save_snippet_in`/`delete_snippet_in(base, …)` ausgelagert, beide prüfen `ensure_within` gegen
> `match_dir()`; die Unit-Tests übergeben ihren Temp-Ordner als Basis.

## 7. Toter Code — MEDIUM

- [ ] Ungenutzte Exports, Commands, CSS-Klassen gesucht?
- [ ] Markiert der tote Code eine **fehlende Feature-Verbindung** statt Müll?

> Gefunden: `package_update` existierte in Backend und API, wurde nie aufgerufen. Angeschlossen
> statt gelöscht → „Update verfügbar" im Paket-Hub.

## 8. Linter — MEDIUM

- [ ] `bun run verify` grün? (ESLint, Clippy `-D warnings`, beide Testsuiten, Build, Versionen)
- [ ] Falsch-Positive **dokumentiert abgeschaltet**, statt den Code zu verbiegen?

> `react-hooks/set-state-in-effect` durchdringt die `async`-Grenze nicht.
> Der echte Fund derselben Regel-Familie: ein Ref, das während des Renderns gelesen wurde.

## 9. Öffentliches Repo — HIGH

- [ ] Marke/Logo von der Lizenz ausgenommen?
- [ ] Keine Secrets, keine PII in Reports (`createReport`), keine privaten Pfade in Artefakten?

## 10. Git — HIGH

- [ ] Vor Commit/Push: `git status` + `git branch --show-current` geprüft?
- [ ] Bei unerwartetem Dateizustand **zuerst `git reflog` lesen**, nichts überschreiben.
- [ ] Divergenz per `git pull --rebase`, nie Force-Push.

> Gefunden: Eine parallele Session hatte den Arbeitsbaum auf einen fremden Branch gestellt.
> „Alte" Dateiinhalte waren ein Branch-Signal, kein Datenverlust.
