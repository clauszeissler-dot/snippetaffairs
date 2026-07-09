---
description: Adversarialer Prüfdurchgang gegen REVIEW.md (Loop-Library-Rezept 3)
---

Führe einen **Verifier-Durchlauf** für SnippetAffAIrs aus. Du bist nicht der Builder —
du bist der adversariale Kritiker. Deine Frage ist **„was stimmt hier nicht?"**, nicht
„ist das gut?".

## Ablauf

1. **Deterministische Basis zuerst:** `bun run verify`. Ist die rot, ist das dein erster
   Befund — nicht weitersuchen, bevor sie grün ist.

2. **Lies `REVIEW.md`** und arbeite die zehn Abschnitte ab. Für jeden Punkt gilt:
   Ein Häkchen darf nur setzen, wer einen **Kommando-Output oder Test** vorweisen kann.
   Plausibel aussehender Code ist kein Beleg. Wo du nur vermuten kannst, schreibe
   „unbelegt" — nicht „ok".

3. **Prüfe die Annahmen, die seit dem letzten Durchgang dazugekommen sind:**
   - Neue Tauri-Commands mit mehrwortigen Argumenten → in `ipc_tests.rs` ergänzt?
   - Neue Aufrufe fremder Kommandos → Exit-Code im Fehlerfall **gemessen** (einmal mit
     falschem Argument ausführen) und in die Tabelle in `AGENTS.md` §4 eingetragen?
   - Neue Parser → gegen echte Ausgabe getestet, inklusive Negativfällen?
   - Neue Erfolgsmeldungen → aus beobachtetem Zustand oder nur aus „kein Fehler geworfen"?

4. **Suche toten Code:** ungenutzte Exports, Commands ohne Aufrufer, CSS-Klassen ohne
   Verwendung. Frage bei jedem Fund: markiert er eine fehlende Feature-Verbindung?

## Ausgabe

Eine Liste von Befunden, jeder mit:

- **Severity** (CRITICAL / HIGH / MEDIUM / LOW — Definition in `REVIEW.md`)
- **Beleg** (Kommando + Output, oder Datei:Zeile)
- **Fehlerszenario**: konkrete Eingabe/Zustand → falsches Verhalten

Findest du nichts, sage das klar. Ein sauberer Durchlauf ist ein gültiges Ergebnis —
erfinde keine Befunde, um Arbeit zu zeigen.

## Stop-Bedingung

- **Quality-Streak N=2:** zwei aufeinanderfolgende Durchläufe ohne CRITICAL/HIGH.
- **Fixed Cap N=3 Runden.** Wird der Cap erreicht, ohne dass die Schwelle unterschritten
  wird: **an Claus eskalieren**, nicht still weiterbauen.
- MEDIUM/LOW blockieren nicht, werden aber dokumentiert.

Am Ende: Wenn du einen neuen Fehlertyp gefunden hast, den `REVIEW.md` noch nicht kennt,
**ergänze dort eine Zeile** — mit dem konkreten Fund als Beleg darunter.
