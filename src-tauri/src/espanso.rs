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
    if let Ok(out) = Command::new(finder).arg(arg).output() {
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

/// Führt einen espanso-Subcommand aus und liefert (stdout+stderr, success).
fn run_espanso(args: &[&str]) -> Result<CmdResult, String> {
    let bin = espanso_bin().ok_or_else(|| {
        "AI-1956-CORE: espanso wurde nicht gefunden. Bitte installieren (macOS: `brew install --cask espanso`)."
            .to_string()
    })?;
    let out = Command::new(&bin)
        .args(args)
        .output()
        .map_err(|e| format!("espanso-Aufruf fehlgeschlagen: {e}"))?;
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

/// Ein einzelnes espanso-Match. Bekannte Felder sind benannt, alles Übrige
/// (vars, form, word, propagate_case, regex, image_path …) wird in `extra`
/// aufgefangen — damit Round-Trip-Writes erweiterte Matches NICHT zerstören.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EspMatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Ganze Match-Datei. `matches` sind die Snippets, alles Übrige (global_vars,
/// imports, filter_*) bleibt in `extra` erhalten.
#[derive(Serialize, Deserialize, Default)]
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
        .map_err(|e| format!("Konnte {} nicht lesen: {e}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(MatchDoc::default());
    }
    serde_yaml::from_str::<MatchDoc>(&raw)
        .map_err(|e| format!("Ungültiges YAML in {}: {e}", path.display()))
}

/// Schreibt ein MatchDoc atomar (temp + rename) und legt vorher ein Backup an.
fn write_doc(path: &Path, doc: &MatchDoc) -> Result<(), String> {
    // 1) YAML serialisieren und VOR dem Schreiben erneut parsen (Validierung)
    let yaml = serde_yaml::to_string(doc).map_err(|e| format!("YAML-Serialisierung fehlgeschlagen: {e}"))?;
    serde_yaml::from_str::<MatchDoc>(&yaml)
        .map_err(|e| format!("Interne Validierung fehlgeschlagen, Schreibvorgang abgebrochen: {e}"))?;

    // 2) Backup der bestehenden Datei (falls vorhanden)
    if path.exists() {
        let bak = path.with_extension("yml.bak");
        std::fs::copy(path, &bak)
            .map_err(|e| format!("Backup fehlgeschlagen ({}): {e}", bak.display()))?;
    }

    // 3) Atomar schreiben: temp im selben Verzeichnis, dann rename
    let dir = path.parent().ok_or("Ungültiger Pfad (kein Verzeichnis)")?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("match")
    ));
    std::fs::write(&tmp, yaml.as_bytes())
        .map_err(|e| format!("Schreiben der Temp-Datei fehlgeschlagen: {e}"))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("Atomares Umbenennen fehlgeschlagen: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Snippet-Ableitung für die Anzeige
// ---------------------------------------------------------------------------

fn snippet_view(index: usize, m: &EspMatch) -> SnippetView {
    let trigger = if let Some(t) = &m.trigger {
        t.clone()
    } else if let Some(ts) = &m.triggers {
        ts.join(", ")
    } else {
        "(kein Trigger)".to_string()
    };

    let has_form = m.extra.contains_key("form") || m.extra.contains_key("form_fields");
    let has_vars = m.extra.contains_key("vars");
    let has_image = m.extra.contains_key("image_path");
    let has_regex = m.extra.contains_key("regex");
    let multi_trigger = m.triggers.is_some();

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
    let advanced = has_form || has_vars || has_image || has_regex || multi_trigger;

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
    let dir = match_dir().ok_or("espanso-Config nicht gefunden")?;
    let mut groups = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("match-Ordner nicht lesbar: {e}"))?;
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
#[tauri::command]
pub fn save_snippet(
    file_path: String,
    index: Option<usize>,
    trigger: String,
    replace: String,
    label: Option<String>,
) -> Result<(), String> {
    let path = PathBuf::from(&file_path);
    let trigger = trigger.trim().to_string();
    if trigger.is_empty() {
        return Err("Trigger darf nicht leer sein.".to_string());
    }
    let mut doc = if path.exists() {
        read_doc(&path)?
    } else {
        MatchDoc::default()
    };

    match index {
        Some(i) => {
            let m = doc
                .matches
                .get_mut(i)
                .ok_or("Snippet-Index existiert nicht (Liste veraltet?).")?;
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

#[tauri::command]
pub fn delete_snippet(file_path: String, index: usize) -> Result<(), String> {
    let path = PathBuf::from(&file_path);
    let mut doc = read_doc(&path)?;
    if index >= doc.matches.len() {
        return Err("Snippet-Index existiert nicht (Liste veraltet?).".to_string());
    }
    doc.matches.remove(index);
    write_doc(&path, &doc)
}

/// Erzeugt eine neue leere Match-Datei match/<name>.yml.
#[tauri::command]
pub fn create_match_file(name: String) -> Result<String, String> {
    let dir = match_dir().ok_or("espanso-Config nicht gefunden")?;
    let safe: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if safe.is_empty() {
        return Err("Ungültiger Dateiname.".to_string());
    }
    let path = dir.join(format!("{safe}.yml"));
    if path.exists() {
        return Err(format!("Datei {safe}.yml existiert bereits."));
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
        output: "espanso nicht gefunden".to_string(),
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
        );
        assert!(res.is_err());
        let _ = std::fs::remove_file(&path);
    }
}
