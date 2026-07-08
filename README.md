<p align="center">
  <img src="assets/banner.png" alt="SnippetAffAIrs — KI AffAIrs" width="100%">
</p>

<h1 align="center">SnippetAffAIrs</h1>

<p align="center">
  <b>Deine Textbausteine per Kürzel — überall, blitzschnell, in schönem Design.</b><br>
  Eine Desktop-App, die aus wenigen getippten Zeichen ganze Textblöcke macht.
</p>

<p align="center">
  <a href="https://github.com/clauszeissler-dot/snippetaffairs/releases/latest"><img src="https://img.shields.io/badge/Download-neueste%20Version-FC8337?style=for-the-badge" alt="Download"></a>
  &nbsp;
  <img src="https://img.shields.io/badge/Windows%20·%20macOS%20·%20Linux-00E5FF?style=for-the-badge" alt="Plattformen">
</p>

<p align="center">
  🇩🇪 Deutsch (diese Seite) &nbsp;·&nbsp; <a href="README.en.md">🇬🇧 English version</a>
</p>

---

## Was ist das? (in einem Satz)

Du tippst z. B. `:mail` und es erscheint sofort deine komplette Signatur. Oder `:tel` → deine
Telefonnummer. Oder `:datum` → das heutige Datum. **In jedem Programm** — E-Mail, Word, Browser,
Chat. SnippetAffAIrs ist die schöne, einfache Oberfläche dafür, mit der du diese Kürzel anlegst
und verwaltest.

<p align="center">
  <img src="assets/screenshot.png" alt="SnippetAffAIrs Screenshot" width="90%">
</p>

## Was kann die App?

- ⌨️ **Snippets verwalten** — Kürzel + Ersetzungstext anlegen, bearbeiten, löschen, durchsuchen.
- 🗂️ **Ordnung** — Snippets in mehreren Dateien gruppieren (z. B. „Arbeit", „Privat").
- 🟢 **Ein/Aus mit einem Klick** — Status sehen, starten/stoppen/neu starten.
- 🧩 **Paket-Hub** — fertige Snippet-Sammlungen aus der espanso-Community (Emojis, Symbole,
  Textbausteine) mit einem Klick installieren.
- 🛡️ **Sicher** — vor jeder Änderung wird automatisch ein Backup deiner Datei angelegt.

---

## ⬇️ Download & Installieren (ganz einfach)

> **Voraussetzung:** SnippetAffAIrs ist die Oberfläche für die kostenlose Engine **espanso**.
> Die installierst du einmalig mit — das ist ein einzelner Befehl bzw. Klick, siehe unten.

### 1. SnippetAffAIrs herunterladen — Button für dein System

<p align="center">
  <a href="https://github.com/clauszeissler-dot/snippetaffairs/releases/latest/download/SnippetAffAIrs-macOS-AppleSilicon.dmg"><img src="https://img.shields.io/badge/macOS-Apple_Silicon_·_.dmg-FC8337?style=for-the-badge&logo=apple&logoColor=white" alt="Download für macOS"></a>
  &nbsp;
  <a href="https://github.com/clauszeissler-dot/snippetaffairs/releases/latest"><img src="https://img.shields.io/badge/Windows-.exe-2D2D2D?style=for-the-badge&logo=windows11&logoColor=00E5FF" alt="Download für Windows"></a>
  &nbsp;
  <a href="https://github.com/clauszeissler-dot/snippetaffairs/releases/latest"><img src="https://img.shields.io/badge/Linux-.AppImage_·_.deb-2D2D2D?style=for-the-badge&logo=linux&logoColor=00E5FF" alt="Download für Linux"></a>
</p>

> ℹ️ **macOS (Apple Silicon)** ist als fertige Datei da — der Button lädt sie direkt.
> **Windows**, **Linux** und **macOS (Intel)** werden automatisch gebaut und erscheinen auf der
> [Release-Seite](https://github.com/clauszeissler-dot/snippetaffairs/releases/latest); diese
> beiden Buttons führen dorthin (Build via GitHub Actions, siehe Abschnitt „Für Entwickler").

**Welche Datei ist für mich?**

| System | Datei | Installation |
|--------|-------|--------------|
| 🍎 macOS (Apple Silicon, M1–M4) | `SnippetAffAIrs_…_aarch64.dmg` | Öffnen → App ins „Programme"-Fenster ziehen |
| 🍎 macOS (Intel) | `SnippetAffAIrs_…_x64.dmg` | dito |
| 🪟 Windows | `SnippetAffAIrs_…_x64-setup.exe` | Doppelklick → „Installieren" |
| 🐧 Linux | `.AppImage` (ausführbar machen) **oder** `.deb` | AppImage starten bzw. `sudo dpkg -i …` |

<details>
<summary>🍎 <b>macOS meldet „nicht verifiziert / kann nicht geöffnet werden"?</b> (einmalig)</summary>

Das ist normal bei kostenlosen Apps ohne teures Apple-Zertifikat. So öffnest du sie trotzdem:
**Rechtsklick auf die App → „Öffnen" → im Dialog nochmal „Öffnen".** Danach startet sie immer normal.
</details>

### 2. espanso installieren (die Engine, einmalig)

<details>
<summary>🪟 <b>Windows</b></summary>

Lade den espanso-Installer von **[espanso.org/install](https://espanso.org/install/)** (Windows) und
führe ihn aus. Alternativ per Winget: `winget install espanso`.
</details>

<details>
<summary>🍎 <b>macOS</b></summary>

Einmal [Homebrew](https://brew.sh) installiert? Dann im Terminal:
```bash
brew install --cask espanso
```
</details>

<details>
<summary>🐧 <b>Linux</b></summary>

Siehe **[espanso.org/install](https://espanso.org/install/)** (AppImage/Snap/Paketquellen).
</details>

### 3. Loslegen
SnippetAffAIrs öffnen → oben auf **Start** → fertig. 🎉

---

## 🍎 Der eine wichtige Schritt auf dem Mac

Damit espanso Text in andere Programme schreiben darf, braucht es einmalig deine Erlaubnis
(das ist Apples Sicherheitsmechanismus, kein Fehler):

1. **Systemeinstellungen** → **Datenschutz & Sicherheit** → **Bedienungshilfen**
2. **Espanso** in der Liste **aktivieren** (Haken setzen).
3. In SnippetAffAIrs oben auf **Start** klicken → die Anzeige wird **grün**.

Danach tippe irgendwo `:espanso` — es wird automatisch zu „Hi there!". Läuft. ✅

---

## ⌨️ So nutzt du es

1. **＋ Neues Snippet** klicken.
2. **Trigger** = dein Kürzel (z. B. `:mail`). Tipp: mit einem `:` am Anfang lösen Kürzel nicht
   aus Versehen mitten in Wörtern aus.
3. **Ersetzung** = der Text, der eingefügt werden soll.
4. **Speichern** — sofort einsatzbereit, in jedem Programm.

**Paket-Hub:** links auf **🧩 Paket-Hub** → durchsuchen → **Installieren**. Fertige Sammlungen
(z. B. alle Emojis per `:emoji`) landen direkt bei dir.

---

## ℹ️ Was ist espanso?

[espanso](https://espanso.org) ist ein kostenloser, quelloffener Text-Expander (GPL-3.0), der
komplett über einfache Dateien funktioniert — aber bewusst **keine** grafische Oberfläche
mitbringt. **SnippetAffAIrs ist genau diese Oberfläche**: eine eigenständige App, die espanso
komfortabel und schön bedienbar macht. Alle Snippets bleiben normale espanso-Dateien — du bist
nie eingesperrt.

---

## 🛠️ Für Entwickler

SnippetAffAIrs ist eine **[Tauri](https://tauri.app)**-App (Rust-Backend + React/TypeScript-Frontend).

```bash
# Voraussetzungen: Node/Bun, Rust (rustup), espanso installiert
git clone https://github.com/clauszeissler-dot/snippetaffairs.git
cd snippetaffairs
bun install
bun run tauri dev      # Entwicklung
bun run tauri build    # Installer für dein OS bauen
```

Architektur in Kürze:
- `src-tauri/src/espanso.rs` — liest/schreibt espansos YAML-Match-Dateien (atomar + Backup),
  steuert Service & Pakete über die espanso-CLI.
- `src/` — React-Oberfläche im KI-AffAIrs-Design (Sidebar, Editor, Paket-Hub).

Installer für alle Plattformen werden automatisch per GitHub Actions gebaut (siehe
`.github/workflows/release.yml`) — bei jedem veröffentlichten Tag entsteht ein Release mit
`.exe`, `.dmg` und Linux-Paketen.

---

## 📄 Lizenz

**[CC BY 4.0](LICENSE)** (Namensnennung 4.0 International) — frei nutzbar, weitergebbar und
veränderbar, auch kommerziell, **solange du den Ersteller nennst**:
„KI AffAIrs (Claus Zeißler)" · [affairs-consulting.de](https://www.affairs-consulting.de).

espanso selbst steht unter GPL-3.0 und wird nur als eigenständiges Programm aufgerufen
(nicht eingebettet).

---

<p align="center">
  Ein Werkzeug von <b>KI AffAIrs</b> · <a href="https://www.affairs-consulting.de">affairs-consulting.de</a><br>
  <sub>Pragmatische KI-Werkzeuge für den Mittelstand.</sub>
</p>
