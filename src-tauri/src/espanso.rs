//! Backend-Anbindung an espanso: Binary auflösen, CLI aufrufen,
//! Match-YAML-Dateien lesen und (atomar + Backup) schreiben.
//!
//! Grundsatz: espanso ist rein dateibasiert und lädt geänderte Dateien
//! automatisch neu (auto_restart). Diese GUI schreibt also nur die YAMLs
//! und ruft die espanso-CLI für Service-/Paketsteuerung.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

// ---------------------------------------------------------------------------
// Fehlercodes (errorcodebase-Standard, siehe AGENTS.md)
// Format "ECB:<code>|<detail>" — das Frontend löst den Code gegen die
// vendorte Registry auf (src/lib/errors.ts). Codes NIE erfinden.
// ---------------------------------------------------------------------------

const ECB_NOT_FOUND: &str = "AI-2011-FIND"; // Ressource fehlt (Binary, Config)
const ECB_INPUT: &str = "AI-1955-INPUT"; // ungültige Eingabe
const ECB_STALE: &str = "AI-2017-CTX"; // Ansicht veraltet → neu laden
const ECB_CORE: &str = "AI-1956-CORE"; // interner Fehler / IO / Fallback

fn ecb(code: &str, detail: impl std::fmt::Display) -> String {
    format!("ECB:{code}|{detail}")
}

const MSG_NO_CONFIG: &str =
    "Die Konfiguration der Text-Expander-Engine wurde nicht gefunden. Ist espanso installiert und einmal gestartet worden?";

/// Fehler, wenn die angezeigte Liste nicht mehr zur Datei auf der Platte passt
/// (Datei extern geändert, Snippet verschoben/gelöscht). Kein Blind-Schreiben.
fn stale_err() -> String {
    ecb(
        ECB_STALE,
        "Die Snippet-Liste ist nicht mehr aktuell — die Datei wurde zwischenzeitlich geändert. Bitte neu laden und noch einmal versuchen.",
    )
}

// ---------------------------------------------------------------------------
// Binary-Auflösung
// ---------------------------------------------------------------------------

/// Findet das espanso-Binary robust — wichtig, weil eine aus dem Finder
/// gestartete .app nicht zwingend /opt/homebrew/bin im PATH hat.
fn espanso_bin() -> Option<PathBuf> {
    // 1) Bekannte feste Pfade (macOS/Linux/Homebrew)
    let candidates = [
        "/opt/homebrew/bin/espanso",
        "/usr/local/bin/espanso",
        "/usr/bin/espanso",
        "/Applications/Espanso.app/Contents/MacOS/espanso",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    // 2) which/where über die Shell (fängt Windows + individuelle Pfade)
    #[cfg(windows)]
    let (finder, arg) = ("where", "espanso");
    #[cfg(not(windows))]
    let (finder, arg) = ("which", "espanso");
    let mut finder_cmd = Command::new(finder);
    finder_cmd.arg(arg);
    no_window(&mut finder_cmd);
    if let Ok(out) = finder_cmd.output() {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Some(line) = s.lines().next() {
                    let t = line.trim();
                    if !t.is_empty() {
                        return Some(PathBuf::from(t));
                    }
                }
            }
        }
    }
    None
}

/// Unterdrückt unter Windows das Konsolenfenster, das GUI-Apps sonst bei
/// jedem Prozess-Spawn aufblitzen lassen. No-op auf anderen Plattformen.
fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Baut das Command für das espanso-Binary. Windows-Sonderfall: `.cmd`/`.bat`-
/// Shims lassen sich nicht direkt spawnen (CreateProcess) → über `cmd /C`.
fn espanso_command(bin: &Path) -> Command {
    #[cfg(windows)]
    {
        let is_batch = bin
            .extension()
            .map(|e| {
                let e = e.to_string_lossy().to_ascii_lowercase();
                e == "cmd" || e == "bat"
            })
            .unwrap_or(false);
        if is_batch {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(bin);
            return c;
        }
    }
    Command::new(bin)
}

/// Führt einen espanso-Subcommand aus und liefert (stdout+stderr, success).
fn run_espanso(args: &[&str]) -> Result<CmdResult, String> {
    let bin = espanso_bin().ok_or_else(|| {
        ecb(
            ECB_NOT_FOUND,
            "Die Text-Expander-Engine (espanso) wurde nicht gefunden. macOS: `brew install --cask espanso` — sonst espanso.org/install.",
        )
    })?;
    let mut cmd = espanso_command(&bin);
    cmd.args(args);
    no_window(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| ecb(ECB_CORE, format!("Engine-Aufruf fehlgeschlagen: {e}")))?;
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    Ok(CmdResult {
        success: out.status.success(),
        output: text.trim().to_string(),
    })
}

#[derive(Serialize)]
pub struct CmdResult {
    pub success: bool,
    pub output: String,
}

// ---------------------------------------------------------------------------
// Datenmodell
// ---------------------------------------------------------------------------

/// Wandelt YAML-Skalare tolerant in Strings (fängt die klassischen Footguns
/// `trigger: no` → bool oder `replace: 42` → int ab, statt die ganze Datei
/// als unlesbar abzulehnen). Nicht-Skalare ergeben None.
fn value_to_string(v: Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn de_opt_scalar<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Value>::deserialize(d)?.and_then(value_to_string))
}

fn de_opt_scalar_vec<'de, D>(d: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Vec<Value>>::deserialize(d)?
        .map(|v| v.into_iter().filter_map(value_to_string).collect()))
}

/// Ein einzelnes espanso-Match. Bekannte Felder sind benannt, alles Übrige
/// (vars, form, word, propagate_case, regex, image_path …) wird in `extra`
/// aufgefangen — damit Round-Trip-Writes erweiterte Matches NICHT zerstören.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct EspMatch {
    #[serde(default, deserialize_with = "de_opt_scalar", skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    #[serde(default, deserialize_with = "de_opt_scalar_vec", skip_serializing_if = "Option::is_none")]
    pub triggers: Option<Vec<String>>,
    #[serde(default, deserialize_with = "de_opt_scalar", skip_serializing_if = "Option::is_none")]
    pub replace: Option<String>,
    #[serde(default, deserialize_with = "de_opt_scalar", skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Ganze Match-Datei. `matches` sind die Snippets, alles Übrige (global_vars,
/// imports, filter_*) bleibt in `extra` erhalten.
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct MatchDoc {
    #[serde(default)]
    pub matches: Vec<EspMatch>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// ---- Ansichts-Typen fürs Frontend -----------------------------------------

#[derive(Serialize)]
pub struct EspansoInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub config_path: Option<String>,
    pub match_dir: Option<String>,
    pub packages_path: Option<String>,
}

#[derive(Serialize)]
pub struct SnippetView {
    pub index: usize,
    pub trigger: String,
    pub replace: String,
    pub label: Option<String>,
    /// true, wenn das Match vars/form/regex etc. nutzt → v1-Editor read-only,
    /// um erweiterte Matches nicht versehentlich zu zerstören.
    pub advanced: bool,
    pub kind: String,
}

#[derive(Serialize)]
pub struct FileGroup {
    pub path: String,
    pub name: String,
    pub snippets: Vec<SnippetView>,
}

// ---------------------------------------------------------------------------
// Pfade
// ---------------------------------------------------------------------------

/// Ermittelt den espanso-Config-Ordner primär über `espanso path config`,
/// mit per-OS-Fallback.
fn config_dir() -> Option<PathBuf> {
    if let Ok(res) = run_espanso(&["path", "config"]) {
        if res.success {
            let p = PathBuf::from(res.output.trim());
            if p.exists() {
                return Some(p);
            }
        }
    }
    // Fallback: per-OS-Default
    let home = dirs_home()?;
    #[cfg(target_os = "macos")]
    let p = home.join("Library/Application Support/espanso");
    #[cfg(target_os = "windows")]
    let p = {
        let appdata = std::env::var("APPDATA").ok()?;
        PathBuf::from(appdata).join("espanso")
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let p = home.join(".config/espanso");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .map(PathBuf::from)
}

fn match_dir() -> Option<PathBuf> {
    config_dir().map(|c| c.join("match"))
}

// ---------------------------------------------------------------------------
// YAML-IO (atomar + Backup)
// ---------------------------------------------------------------------------

fn read_doc(path: &Path) -> Result<MatchDoc, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| ecb(ECB_CORE, format!("Konnte {} nicht lesen: {e}", path.display())))?;
    if raw.trim().is_empty() {
        return Ok(MatchDoc::default());
    }
    serde_yaml::from_str::<MatchDoc>(&raw)
        .map_err(|e| ecb(ECB_CORE, format!("Ungültiges YAML in {}: {e}", path.display())))
}

/// Schreibt ein MatchDoc atomar (temp + rename) und legt vorher Backups an.
///
/// Hinweis Kommentare: serde_yaml kann YAML-Kommentare nicht erhalten — ein
/// Rewrite verliert sie. Darum wird vor der ALLERERSTEN GUI-Änderung einer
/// Datei einmalig ein unangetastetes `.yml.orig` abgelegt (bewahrt Original
/// samt Kommentaren dauerhaft); `.yml.bak` hält zusätzlich den jeweils
/// letzten Stand vor dem aktuellen Schreibvorgang.
fn write_doc(path: &Path, doc: &MatchDoc) -> Result<(), String> {
    // 1) YAML serialisieren und VOR dem Schreiben erneut parsen (Validierung)
    let yaml = serde_yaml::to_string(doc)
        .map_err(|e| ecb(ECB_CORE, format!("YAML-Serialisierung fehlgeschlagen: {e}")))?;
    serde_yaml::from_str::<MatchDoc>(&yaml).map_err(|e| {
        ecb(ECB_CORE, format!("Interne Validierung fehlgeschlagen, Schreibvorgang abgebrochen: {e}"))
    })?;

    // 2) Backups der bestehenden Datei (falls vorhanden)
    if path.exists() {
        let orig = path.with_extension("yml.orig");
        if !orig.exists() {
            // Best effort: Original mit Kommentaren dauerhaft sichern.
            let _ = std::fs::copy(path, &orig);
        }
        let bak = path.with_extension("yml.bak");
        std::fs::copy(path, &bak)
            .map_err(|e| ecb(ECB_CORE, format!("Backup fehlgeschlagen ({}): {e}", bak.display())))?;
    }

    // 3) Atomar schreiben: temp im selben Verzeichnis, dann rename
    let dir = path
        .parent()
        .ok_or_else(|| ecb(ECB_INPUT, "Ungültiger Pfad (kein Verzeichnis)"))?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("match")
    ));
    std::fs::write(&tmp, yaml.as_bytes())
        .map_err(|e| ecb(ECB_CORE, format!("Schreiben der Temp-Datei fehlgeschlagen: {e}")))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| ecb(ECB_CORE, format!("Atomares Umbenennen fehlgeschlagen: {e}")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Snippet-Ableitung für die Anzeige
// ---------------------------------------------------------------------------

/// Anzeige-/Vergleichs-Trigger eines Matches (auch für den Staleness-Guard
/// in save/delete — muss deterministisch zur SnippetView passen).
fn effective_trigger(m: &EspMatch) -> String {
    if let Some(t) = &m.trigger {
        t.clone()
    } else if let Some(ts) = &m.triggers {
        ts.join(", ")
    } else {
        "(kein Trigger)".to_string()
    }
}

/// Ein Match, das der v1-Editor nur lesen darf. Wird sowohl für die Anzeige
/// als auch als Schreibschutz im Backend genutzt (Frontend blockt zusätzlich).
fn is_advanced(m: &EspMatch) -> bool {
    m.triggers.is_some()
        || m.extra.contains_key("form")
        || m.extra.contains_key("form_fields")
        || m.extra.contains_key("vars")
        || m.extra.contains_key("image_path")
        || m.extra.contains_key("regex")
}

fn snippet_view(index: usize, m: &EspMatch) -> SnippetView {
    let trigger = effective_trigger(m);

    let has_form = m.extra.contains_key("form") || m.extra.contains_key("form_fields");
    let has_vars = m.extra.contains_key("vars");
    let has_image = m.extra.contains_key("image_path");
    let has_regex = m.extra.contains_key("regex");

    let kind = if has_form {
        "form"
    } else if has_vars {
        "vars"
    } else if has_image {
        "image"
    } else if has_regex {
        "regex"
    } else {
        "text"
    }
    .to_string();

    // "advanced" = im v1-Editor nur lesbar, um Datenverlust zu vermeiden.
    let advanced = is_advanced(m);

    let replace = match &m.replace {
        Some(r) => r.clone(),
        None => {
            if has_form {
                "(Formular)".to_string()
            } else if has_image {
                "(Bild)".to_string()
            } else {
                "(erweitertes Match)".to_string()
            }
        }
    };

    SnippetView {
        index,
        trigger,
        replace,
        label: m.label.clone(),
        advanced,
        kind,
    }
}

// ---------------------------------------------------------------------------
// Tauri-Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_espanso_info() -> EspansoInfo {
    let version = run_espanso(&["--version"]).ok().and_then(|r| {
        // Hinweis: espanso 2.3.0 gibt die Version aus, beendet sich dabei aber mit
        // Exit-Code 1 → success bewusst ignorieren, stattdessen Ausgabe parsen.
        let line = r.output.lines().next().unwrap_or("").trim();
        let token = line.split_whitespace().last().unwrap_or(line).trim().to_string();
        if token.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            Some(token)
        } else {
            None
        }
    });
    let cfg = config_dir();
    let md = match_dir();
    let pkgs = cfg.as_ref().map(|c| c.join("match").join("packages"));
    EspansoInfo {
        installed: espanso_bin().is_some(),
        version,
        config_path: cfg.as_ref().map(|p| p.display().to_string()),
        match_dir: md.as_ref().map(|p| p.display().to_string()),
        packages_path: pkgs.as_ref().map(|p| p.display().to_string()),
    }
}

/// Liest alle Top-Level-Match-Dateien (match/*.yml), OHNE den packages/-Unterordner
/// (Hub-Pakete sind read-only Fremdinhalt).
#[tauri::command]
pub fn list_snippets() -> Result<Vec<FileGroup>, String> {
    let dir = match_dir().ok_or_else(|| ecb(ECB_NOT_FOUND, MSG_NO_CONFIG))?;
    let mut groups = Vec::new();
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| ecb(ECB_CORE, format!("Snippet-Ordner nicht lesbar: {e}")))?;
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .map(|x| x == "yml" || x == "yaml")
                    .unwrap_or(false)
        })
        .collect();
    files.sort();
    for path in files {
        let doc = read_doc(&path)?;
        let snippets = doc
            .matches
            .iter()
            .enumerate()
            .map(|(i, m)| snippet_view(i, m))
            .collect();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("match")
            .to_string();
        groups.push(FileGroup {
            path: path.display().to_string(),
            name,
            snippets,
        });
    }
    Ok(groups)
}

/// Legt ein Snippet an (index = null) oder aktualisiert ein bestehendes.
/// Erweiterte Matches (vars/form/…) werden über `extra` erhalten.
///
/// `expected_trigger` ist der Trigger, den das Frontend an dieser Position
/// ANZEIGT. Weicht er vom Trigger auf der Platte ab, wurde die Datei extern
/// geändert und der Index zeigt auf ein fremdes Snippet → Abbruch statt
/// Überschreiben.
#[tauri::command]
pub fn save_snippet(
    file_path: String,
    index: Option<usize>,
    trigger: String,
    replace: String,
    label: Option<String>,
    expected_trigger: Option<String>,
) -> Result<(), String> {
    let path = PathBuf::from(&file_path);
    let trigger = trigger.trim().to_string();
    if trigger.is_empty() {
        return Err(ecb(ECB_INPUT, "Der Trigger darf nicht leer sein."));
    }
    let mut doc = if path.exists() {
        read_doc(&path)?
    } else {
        MatchDoc::default()
    };

    match index {
        Some(i) => {
            let m = doc.matches.get_mut(i).ok_or_else(stale_err)?;
            if let Some(expected) = expected_trigger.as_deref() {
                if effective_trigger(m) != expected {
                    return Err(stale_err());
                }
            }
            if is_advanced(m) {
                return Err(ecb(
                    ECB_INPUT,
                    "Dieses Snippet nutzt ein erweitertes Match und ist hier schreibgeschützt. Bitte direkt in der YAML-Datei bearbeiten.",
                ));
            }
            // Nur einfache Text-Matches editieren; erweiterte bleiben unangetastet.
            m.trigger = Some(trigger);
            m.triggers = None;
            m.replace = Some(replace);
            m.label = if label.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                None
            } else {
                label.map(|s| s.trim().to_string())
            };
        }
        None => {
            doc.matches.push(EspMatch {
                trigger: Some(trigger),
                triggers: None,
                replace: Some(replace),
                label: label.and_then(|s| {
                    let t = s.trim().to_string();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                }),
                extra: BTreeMap::new(),
            });
        }
    }
    write_doc(&path, &doc)
}

/// Löscht ein Snippet. `expected_trigger` sichert wie bei `save_snippet` ab,
/// dass der Index noch auf das Snippet zeigt, das der Nutzer gesehen hat.
#[tauri::command]
pub fn delete_snippet(
    file_path: String,
    index: usize,
    expected_trigger: Option<String>,
) -> Result<(), String> {
    let path = PathBuf::from(&file_path);
    let mut doc = read_doc(&path)?;
    let m = doc.matches.get(index).ok_or_else(stale_err)?;
    if let Some(expected) = expected_trigger.as_deref() {
        if effective_trigger(m) != expected {
            return Err(stale_err());
        }
    }
    doc.matches.remove(index);
    write_doc(&path, &doc)
}

/// Erzeugt eine neue leere Match-Datei match/<name>.yml.
#[tauri::command]
pub fn create_match_file(name: String) -> Result<String, String> {
    let dir = match_dir().ok_or_else(|| ecb(ECB_NOT_FOUND, MSG_NO_CONFIG))?;
    let safe: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if safe.is_empty() {
        return Err(ecb(ECB_INPUT, "Der Dateiname ist ungültig."));
    }
    let path = dir.join(format!("{safe}.yml"));
    if path.exists() {
        return Err(ecb(
            ECB_INPUT,
            format!("Die Datei {safe}.yml existiert bereits."),
        ));
    }
    let doc = MatchDoc::default();
    write_doc(&path, &doc)?;
    Ok(path.display().to_string())
}

// ---- Service -----------------------------------------------------------------

#[tauri::command]
pub fn service_status() -> CmdResult {
    run_espanso(&["status"]).unwrap_or(CmdResult {
        success: false,
        output: "Engine nicht gefunden".to_string(),
    })
}

#[tauri::command]
pub fn service_start() -> Result<CmdResult, String> {
    run_espanso(&["start"])
}

#[tauri::command]
pub fn service_stop() -> Result<CmdResult, String> {
    run_espanso(&["stop"])
}

#[tauri::command]
pub fn service_restart() -> Result<CmdResult, String> {
    run_espanso(&["restart"])
}

// ---- Pakete ------------------------------------------------------------------

#[tauri::command]
pub fn package_list() -> Result<CmdResult, String> {
    run_espanso(&["package", "list"])
}

#[tauri::command]
pub fn package_install(name: String) -> Result<CmdResult, String> {
    run_espanso(&["install", &name])
}

#[tauri::command]
pub fn package_uninstall(name: String) -> Result<CmdResult, String> {
    run_espanso(&["uninstall", &name])
}

#[tauri::command]
pub fn package_update(name: String) -> Result<CmdResult, String> {
    run_espanso(&["package", "update", &name])
}

// ---------------------------------------------------------------------------
// Tests — sichern die wichtigste Garantie: kein Datenverlust beim Schreiben.
// (Laufen ohne installiertes espanso; nutzen nur Datei-IO im temp-Verzeichnis.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("snippetaffairs_test_{name}.yml"))
    }

    const WITH_VARS: &str = r#"matches:
  - trigger: ":hi"
    replace: "Hallo"
    label: "Gruß"
  - trigger: ":date"
    replace: "{{mydate}}"
    vars:
      - name: mydate
        type: date
        params:
          format: "%d.%m.%Y"
"#;

    #[test]
    fn round_trip_preserves_vars() {
        // Ein Match mit vars darf beim Serialisieren NICHT verloren gehen.
        let doc: MatchDoc = serde_yaml::from_str(WITH_VARS).unwrap();
        assert_eq!(doc.matches.len(), 2);
        let yaml = serde_yaml::to_string(&doc).unwrap();
        assert!(yaml.contains("vars"), "vars muss erhalten bleiben");
        assert!(yaml.contains("mydate"));
        let back: MatchDoc = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.matches.len(), 2);
        assert!(back.matches[1].extra.contains_key("vars"));
    }

    #[test]
    fn snippet_view_flags_advanced() {
        let doc: MatchDoc = serde_yaml::from_str(WITH_VARS).unwrap();
        let simple = snippet_view(0, &doc.matches[0]);
        let with_vars = snippet_view(1, &doc.matches[1]);
        assert!(!simple.advanced, "einfaches Match ist editierbar");
        assert_eq!(simple.trigger, ":hi");
        assert!(with_vars.advanced, "vars-Match ist advanced/read-only");
        assert_eq!(with_vars.kind, "vars");
    }

    #[test]
    fn write_creates_backup_and_keeps_existing() {
        let path = tmp_path("backup");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("yml.bak"));
        let _ = std::fs::remove_file(path.with_extension("yml.orig"));

        // Ausgangsdatei mit vars-Match schreiben
        std::fs::write(&path, WITH_VARS).unwrap();
        let mut doc = read_doc(&path).unwrap();

        // Neues einfaches Snippet hinzufügen
        doc.matches.push(EspMatch {
            trigger: Some(":new".into()),
            replace: Some("Neu".into()),
            ..Default::default()
        });
        write_doc(&path, &doc).unwrap();

        // Backup muss existieren und den Originalinhalt haben
        let bak = path.with_extension("yml.bak");
        assert!(bak.exists(), "Backup .yml.bak muss angelegt sein");
        assert!(std::fs::read_to_string(&bak).unwrap().contains(":date"));

        // Neu geschriebene Datei: altes vars-Match UND neues Snippet vorhanden
        let reread = read_doc(&path).unwrap();
        assert_eq!(reread.matches.len(), 3);
        assert!(reread.matches.iter().any(|m| m.extra.contains_key("vars")));
        assert!(reread.matches.iter().any(|m| m.trigger.as_deref() == Some(":new")));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&bak);
    }

    #[test]
    fn empty_trigger_rejected_at_command() {
        // save_snippet lehnt leere Trigger ab (kein Schreiben ins Nichts).
        let path = tmp_path("empty");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, "matches: []\n").unwrap();
        let res = save_snippet(
            path.display().to_string(),
            None,
            "   ".into(),
            "x".into(),
            None,
            None,
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().starts_with("ECB:AI-1955-INPUT|"));
        let _ = std::fs::remove_file(&path);
    }

    /// Jeder an das Frontend gereichte Fehler muss maschinenlesbar sein,
    /// sonst zeigt der Resolver den Fallback statt der echten Ursache.
    #[test]
    fn errors_carry_ecb_prefix() {
        let path = tmp_path("ecb_format");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, "matches: [\n").unwrap(); // kaputtes YAML
        let err = read_doc(&path).unwrap_err();
        assert!(err.starts_with("ECB:AI-1956-CORE|"), "war: {err}");
        // Code-Teil und Detail müssen sauber trennbar bleiben.
        let (code, detail) = err.strip_prefix("ECB:").unwrap().split_once('|').unwrap();
        assert_eq!(code, "AI-1956-CORE");
        assert!(!detail.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn stale_index_is_rejected_before_write() {
        // Die Datei wurde extern geändert: Index 0 zeigt jetzt auf ein anderes
        // Snippet als das, was der Nutzer im Editor geöffnet hat.
        let path = tmp_path("stale");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, WITH_VARS).unwrap();

        let res = save_snippet(
            path.display().to_string(),
            Some(0),
            ":neu".into(),
            "Neu".into(),
            None,
            Some(":wasanderes".into()), // erwartet ":hi"
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().starts_with("ECB:AI-2017-CTX|"));

        let del = delete_snippet(path.display().to_string(), 0, Some(":wasanderes".into()));
        assert!(del.is_err());
        assert!(del.unwrap_err().starts_with("ECB:AI-2017-CTX|"));

        // Nichts darf geschrieben worden sein.
        let doc = read_doc(&path).unwrap();
        assert_eq!(doc.matches.len(), 2);
        assert_eq!(doc.matches[0].trigger.as_deref(), Some(":hi"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("yml.bak"));
        let _ = std::fs::remove_file(path.with_extension("yml.orig"));
    }

    #[test]
    fn matching_expected_trigger_allows_write() {
        let path = tmp_path("guard_ok");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, WITH_VARS).unwrap();

        save_snippet(
            path.display().to_string(),
            Some(0),
            ":hi".into(),
            "Servus".into(),
            None,
            Some(":hi".into()),
        )
        .unwrap();

        let doc = read_doc(&path).unwrap();
        assert_eq!(doc.matches[0].replace.as_deref(), Some("Servus"));
        assert!(doc.matches[1].extra.contains_key("vars"), "vars intakt");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("yml.bak"));
        let _ = std::fs::remove_file(path.with_extension("yml.orig"));
    }

    #[test]
    fn advanced_match_is_write_protected() {
        // Auch wenn das Frontend den Schutz umginge: das Backend schreibt
        // kein vars/form-Match platt.
        let path = tmp_path("advanced");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, WITH_VARS).unwrap();

        let res = save_snippet(
            path.display().to_string(),
            Some(1),
            ":date".into(),
            "kaputt".into(),
            None,
            Some(":date".into()),
        );
        assert!(res.is_err(), "vars-Match darf nicht überschrieben werden");

        let doc = read_doc(&path).unwrap();
        assert!(doc.matches[1].extra.contains_key("vars"));
        assert_eq!(doc.matches[1].replace.as_deref(), Some("{{mydate}}"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("yml.bak"));
        let _ = std::fs::remove_file(path.with_extension("yml.orig"));
    }

    #[test]
    fn tolerates_non_string_scalars() {
        // YAML-Footgun: unquotierte Skalare landen als bool/int im Dokument.
        // (serde_yaml folgt YAML 1.2: `no` bleibt String, `true` wird bool.)
        // Solche Dateien müssen lesbar bleiben, statt komplett abgelehnt zu werden.
        let yaml = "matches:\n  - trigger: no\n    replace: 42\n  - trigger: true\n    replace: 3.5\n";
        let doc: MatchDoc = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(doc.matches[0].trigger.as_deref(), Some("no"));
        assert_eq!(doc.matches[0].replace.as_deref(), Some("42"));
        assert_eq!(doc.matches[1].trigger.as_deref(), Some("true"));
        assert_eq!(doc.matches[1].replace.as_deref(), Some("3.5"));
    }

    #[test]
    fn first_write_preserves_original_with_comments() {
        let path = tmp_path("orig");
        let orig = path.with_extension("yml.orig");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&orig);

        let with_comment = "# wichtige Notiz\nmatches:\n  - trigger: \":hi\"\n    replace: \"Hallo\"\n";
        std::fs::write(&path, with_comment).unwrap();

        let doc = read_doc(&path).unwrap();
        write_doc(&path, &doc).unwrap();
        assert!(orig.exists(), ".yml.orig muss beim ersten Write entstehen");
        assert!(std::fs::read_to_string(&orig).unwrap().contains("# wichtige Notiz"));

        // Zweiter Write darf das Original NICHT mit der kommentarlosen Fassung überschreiben.
        write_doc(&path, &doc).unwrap();
        assert!(std::fs::read_to_string(&orig).unwrap().contains("# wichtige Notiz"));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&orig);
        let _ = std::fs::remove_file(path.with_extension("yml.bak"));
    }
}
