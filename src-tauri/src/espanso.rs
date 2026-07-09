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
const ECB_FLOW: &str = "AI-2016-FLOW"; // Engine-Aktion fehlgeschlagen

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

/// Stellt sicher, dass `path` wirklich unterhalb von `base` liegt. Schützt die
/// destruktiven Datei-Operationen (löschen, umbenennen, wiederherstellen) davor,
/// über `..` oder absolute Pfade aus dem match-Ordner auszubrechen.
///
/// Verglichen wird kanonisiert (Symlinks aufgelöst); für noch nicht existierende
/// Ziele wird das Elternverzeichnis geprüft.
fn ensure_within(path: &Path, base: &Path) -> Result<(), String> {
    let base = base
        .canonicalize()
        .map_err(|e| ecb(ECB_CORE, format!("Basisordner nicht auflösbar: {e}")))?;

    let probe = if path.exists() {
        path.canonicalize()
    } else {
        // Zielpfad existiert noch nicht (rename) → Elternordner prüfen und
        // den Dateinamen wieder anhängen.
        let parent = path
            .parent()
            .ok_or_else(|| ecb(ECB_INPUT, "Ungültiger Pfad (kein Verzeichnis)"))?;
        parent.canonicalize().map(|p| match path.file_name() {
            Some(n) => p.join(n),
            None => p,
        })
    }
    .map_err(|e| ecb(ECB_NOT_FOUND, format!("Pfad nicht auflösbar: {e}")))?;

    if probe.starts_with(&base) {
        Ok(())
    } else {
        Err(ecb(
            ECB_INPUT,
            "Diese Datei liegt außerhalb des Snippet-Ordners.",
        ))
    }
}

/// Säubert einen Dateinamen zu `[a-zA-Z0-9-_]`. Leerer Rest ⇒ Fehler.
fn safe_file_stem(name: &str) -> Result<String, String> {
    let safe: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if safe.is_empty() {
        return Err(ecb(ECB_INPUT, "Der Dateiname ist ungültig."));
    }
    Ok(safe)
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

// ---------------------------------------------------------------------------
// Einfache Variablen ({{date}} / {{clipboard}})
//
// Der Editor kann diese zwei Bausteine einfügen, ohne dass daraus ein
// „erweitertes" (read-only) Match wird. Beim Speichern erzeugt das Backend den
// passenden `vars`-Block aus den Platzhaltern im Ersetzungstext neu.
//
// Alles, was NICHT exakt diesem Schema entspricht (fremde vars, andere Typen,
// zusätzliche Felder), bleibt read-only — sonst zerstört ein Speichern die
// Handarbeit des Nutzers.
// ---------------------------------------------------------------------------

const VAR_DATE: &str = "date";
const VAR_CLIPBOARD: &str = "clipboard";
const DEFAULT_DATE_FORMAT: &str = "%d.%m.%Y";

/// Erkennt genau die zwei von uns erzeugten Variablenformen.
fn is_simple_var(v: &Value) -> bool {
    let Some(map) = v.as_mapping() else {
        return false;
    };
    let get = |k: &str| map.get(Value::String(k.to_string()));
    let name = get("name").and_then(|v| v.as_str()).unwrap_or("");
    let vtype = get("type").and_then(|v| v.as_str()).unwrap_or("");

    match (name, vtype) {
        (VAR_CLIPBOARD, "clipboard") => map.len() == 2,
        (VAR_DATE, "date") => {
            // name + type + params{format}
            if map.len() != 3 {
                return false;
            }
            let Some(params) = get("params").and_then(|v| v.as_mapping().cloned()) else {
                return false;
            };
            params.len() == 1
                && params
                    .get(Value::String("format".into()))
                    .and_then(|v| v.as_str())
                    .is_some()
        }
        _ => false,
    }
}

/// Trifft zu, wenn ALLE vars des Matches von uns stammen.
fn has_only_simple_vars(m: &EspMatch) -> bool {
    match m.extra.get("vars") {
        None => true,
        Some(Value::Sequence(seq)) => seq.iter().all(is_simple_var),
        Some(_) => false,
    }
}

/// Liest das in einem bestehenden Match hinterlegte Datumsformat, damit ein
/// erneutes Speichern eine abweichende Nutzer-Einstellung nicht plattmacht.
fn existing_date_format(m: &EspMatch) -> String {
    if let Some(Value::Sequence(seq)) = m.extra.get("vars") {
        for v in seq {
            let Some(map) = v.as_mapping() else { continue };
            if map.get(Value::String("name".into())).and_then(|v| v.as_str()) == Some(VAR_DATE) {
                if let Some(f) = map
                    .get(Value::String("params".into()))
                    .and_then(|p| p.as_mapping())
                    .and_then(|p| p.get(Value::String("format".into())))
                    .and_then(|v| v.as_str())
                {
                    return f.to_string();
                }
            }
        }
    }
    DEFAULT_DATE_FORMAT.to_string()
}

/// Baut den `vars`-Block aus den Platzhaltern im Ersetzungstext.
/// Kein Platzhalter ⇒ `None` (der vars-Schlüssel wird dann entfernt).
fn build_simple_vars(replace: &str, date_format: &str) -> Option<Value> {
    let mut vars: Vec<Value> = Vec::new();

    if replace.contains("{{date}}") {
        let mut params = serde_yaml::Mapping::new();
        params.insert(
            Value::String("format".into()),
            Value::String(date_format.to_string()),
        );
        let mut v = serde_yaml::Mapping::new();
        v.insert(Value::String("name".into()), Value::String(VAR_DATE.into()));
        v.insert(Value::String("type".into()), Value::String("date".into()));
        v.insert(Value::String("params".into()), Value::Mapping(params));
        vars.push(Value::Mapping(v));
    }
    if replace.contains("{{clipboard}}") {
        let mut v = serde_yaml::Mapping::new();
        v.insert(
            Value::String("name".into()),
            Value::String(VAR_CLIPBOARD.into()),
        );
        v.insert(
            Value::String("type".into()),
            Value::String("clipboard".into()),
        );
        vars.push(Value::Mapping(v));
    }

    if vars.is_empty() {
        None
    } else {
        Some(Value::Sequence(vars))
    }
}

/// Ein Match, das der Editor nur lesen darf. Wird sowohl für die Anzeige
/// als auch als Schreibschutz im Backend genutzt (Frontend blockt zusätzlich).
///
/// Matches mit ausschließlich einfachen vars ({{date}}/{{clipboard}}) sind
/// bewusst NICHT advanced — die kann der Editor gefahrlos neu schreiben.
fn is_advanced(m: &EspMatch) -> bool {
    m.triggers.is_some()
        || m.extra.contains_key("form")
        || m.extra.contains_key("form_fields")
        || m.extra.contains_key("image_path")
        || m.extra.contains_key("regex")
        || (m.extra.contains_key("vars") && !has_only_simple_vars(m))
}

fn snippet_view(index: usize, m: &EspMatch) -> SnippetView {
    let trigger = effective_trigger(m);

    let has_form = m.extra.contains_key("form") || m.extra.contains_key("form_fields");
    let has_vars = m.extra.contains_key("vars");
    let has_image = m.extra.contains_key("image_path");
    let has_regex = m.extra.contains_key("regex");
    // "advanced" = nur lesbar, um Datenverlust zu vermeiden.
    let advanced = is_advanced(m);

    let kind = if has_form {
        "form"
    } else if has_image {
        "image"
    } else if has_regex {
        "regex"
    } else if has_vars && advanced {
        "vars"
    } else if has_vars {
        // Von uns erzeugte {{date}}/{{clipboard}}-Bausteine — editierbar.
        "dynamisch"
    } else {
        "text"
    }
    .to_string();

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
            // Das Datumsformat eines bestehenden Matches wird übernommen, damit
            // eine abweichende Nutzer-Einstellung ein Speichern überlebt.
            let date_format = existing_date_format(m);
            m.trigger = Some(trigger);
            m.triggers = None;
            m.replace = Some(replace.clone());
            m.label = if label.as_deref().map(|s| s.trim().is_empty()).unwrap_or(true) {
                None
            } else {
                label.map(|s| s.trim().to_string())
            };
            match build_simple_vars(&replace, &date_format) {
                Some(vars) => m.extra.insert("vars".into(), vars),
                None => m.extra.remove("vars"),
            };
        }
        None => {
            let mut extra = BTreeMap::new();
            if let Some(vars) = build_simple_vars(&replace, DEFAULT_DATE_FORMAT) {
                extra.insert("vars".to_string(), vars);
            }
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
                extra,
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
    let safe = safe_file_stem(&name)?;
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

/// Benennt eine Match-Datei um. Backups (.bak/.orig) wandern mit, damit die
/// Wiederherstellung nicht ins Leere zeigt.
#[tauri::command]
pub fn rename_match_file(file_path: String, new_name: String) -> Result<String, String> {
    let dir = match_dir().ok_or_else(|| ecb(ECB_NOT_FOUND, MSG_NO_CONFIG))?;
    let path = PathBuf::from(&file_path);
    ensure_within(&path, &dir)?;

    let safe = safe_file_stem(&new_name)?;
    let target = path
        .parent()
        .ok_or_else(|| ecb(ECB_INPUT, "Ungültiger Pfad."))?
        .join(format!("{safe}.yml"));
    ensure_within(&target, &dir)?;

    if target == path {
        return Ok(target.display().to_string());
    }
    if target.exists() {
        return Err(ecb(
            ECB_INPUT,
            format!("Die Datei {safe}.yml existiert bereits."),
        ));
    }
    std::fs::rename(&path, &target)
        .map_err(|e| ecb(ECB_CORE, format!("Umbenennen fehlgeschlagen: {e}")))?;

    for ext in ["yml.bak", "yml.orig"] {
        let from = path.with_extension(ext);
        if from.exists() {
            let _ = std::fs::rename(&from, target.with_extension(ext));
        }
    }
    Ok(target.display().to_string())
}

/// Löscht eine Match-Datei samt ihrer Backups.
#[tauri::command]
pub fn delete_match_file(file_path: String) -> Result<(), String> {
    let dir = match_dir().ok_or_else(|| ecb(ECB_NOT_FOUND, MSG_NO_CONFIG))?;
    let path = PathBuf::from(&file_path);
    ensure_within(&path, &dir)?;
    if !path.exists() {
        return Err(stale_err());
    }
    std::fs::remove_file(&path)
        .map_err(|e| ecb(ECB_CORE, format!("Löschen fehlgeschlagen: {e}")))?;
    for ext in ["yml.bak", "yml.orig"] {
        let _ = std::fs::remove_file(path.with_extension(ext));
    }
    Ok(())
}

// ---- Trigger-Kollisionen -----------------------------------------------------

#[derive(Serialize)]
pub struct ConflictSite {
    /// Anzeigename der Quelle: Dateiname bzw. "Paket: <name>".
    pub source: String,
    pub file_path: String,
    pub index: usize,
}

#[derive(Serialize)]
pub struct TriggerConflict {
    pub trigger: String,
    pub sites: Vec<ConflictSite>,
}

/// Sammelt alle Match-Dateien inkl. `packages/`, denn auch ein Hub-Paket kann
/// einen Trigger belegen, den der Nutzer selbst vergibt.
fn all_match_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for p in entries.flatten().map(|e| e.path()) {
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .map(|x| x == "yml" || x == "yaml")
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Bezeichnung der Quelle für die Anzeige: Paketname, sonst Dateiname.
fn source_label(path: &Path, match_root: &Path) -> String {
    let rel = path.strip_prefix(match_root).unwrap_or(path);
    let parts: Vec<_> = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect();
    if parts.len() >= 2 && parts[0] == "packages" {
        format!("Paket: {}", parts[1])
    } else {
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| rel.display().to_string())
    }
}

/// Findet Trigger, die mehrfach vergeben sind. Solche Doppelungen sind still:
/// espanso expandiert nur eines der Matches, ohne zu warnen.
#[tauri::command]
pub fn trigger_conflicts() -> Result<Vec<TriggerConflict>, String> {
    let dir = match_dir().ok_or_else(|| ecb(ECB_NOT_FOUND, MSG_NO_CONFIG))?;
    Ok(conflicts_in(&dir))
}

fn conflicts_in(dir: &Path) -> Vec<TriggerConflict> {
    let mut by_trigger: BTreeMap<String, Vec<ConflictSite>> = BTreeMap::new();

    for path in all_match_files(dir) {
        // Unlesbare Fremddateien dürfen die Prüfung nicht abbrechen.
        let Ok(doc) = read_doc(&path) else { continue };
        for (index, m) in doc.matches.iter().enumerate() {
            // Ein Match kann mehrere Trigger tragen — jeder zählt einzeln.
            let triggers: Vec<String> = match (&m.trigger, &m.triggers) {
                (Some(t), _) => vec![t.clone()],
                (None, Some(ts)) => ts.clone(),
                _ => continue,
            };
            for t in triggers {
                by_trigger.entry(t).or_default().push(ConflictSite {
                    source: source_label(&path, dir),
                    file_path: path.display().to_string(),
                    index,
                });
            }
        }
    }

    by_trigger
        .into_iter()
        .filter(|(_, sites)| sites.len() > 1)
        .map(|(trigger, sites)| TriggerConflict { trigger, sites })
        .collect()
}

// ---- Backups -----------------------------------------------------------------

#[derive(Serialize)]
pub struct BackupInfo {
    /// "bak" = letzter Stand vor der jüngsten Änderung, "orig" = Original.
    pub kind: String,
    pub path: String,
    pub size: u64,
    /// Unix-Sekunden; 0 wenn nicht ermittelbar.
    pub modified: u64,
    pub snippet_count: usize,
}

fn backup_kinds(path: &Path) -> [(&'static str, PathBuf); 2] {
    [
        ("bak", path.with_extension("yml.bak")),
        ("orig", path.with_extension("yml.orig")),
    ]
}

#[tauri::command]
pub fn list_backups(file_path: String) -> Result<Vec<BackupInfo>, String> {
    let dir = match_dir().ok_or_else(|| ecb(ECB_NOT_FOUND, MSG_NO_CONFIG))?;
    let path = PathBuf::from(&file_path);
    ensure_within(&path, &dir)?;

    let mut out = Vec::new();
    for (kind, bpath) in backup_kinds(&path) {
        if !bpath.exists() {
            continue;
        }
        let meta = std::fs::metadata(&bpath).ok();
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Kaputte Backups gar nicht erst zum Wiederherstellen anbieten.
        let Ok(doc) = read_doc(&bpath) else { continue };
        out.push(BackupInfo {
            kind: kind.to_string(),
            path: bpath.display().to_string(),
            size: meta.map(|m| m.len()).unwrap_or(0),
            modified,
            snippet_count: doc.matches.len(),
        });
    }
    Ok(out)
}

/// Spielt ein Backup zurück. Der aktuelle Stand wandert vorher nach `.yml.bak`,
/// der Rückweg bleibt also offen. Kopiert roh (kein Reserialisieren), damit die
/// Kommentare aus `.yml.orig` erhalten bleiben.
#[tauri::command]
pub fn restore_backup(file_path: String, kind: String) -> Result<(), String> {
    let dir = match_dir().ok_or_else(|| ecb(ECB_NOT_FOUND, MSG_NO_CONFIG))?;
    let path = PathBuf::from(&file_path);
    ensure_within(&path, &dir)?;
    restore_from(&path, &kind)
}

fn restore_from(path: &Path, kind: &str) -> Result<(), String> {
    let (_, source) = backup_kinds(path)
        .into_iter()
        .find(|(k, _)| *k == kind)
        .ok_or_else(|| ecb(ECB_INPUT, "Unbekannte Backup-Art."))?;
    if !source.exists() {
        return Err(ecb(ECB_NOT_FOUND, "Dieses Backup existiert nicht (mehr)."));
    }

    // 1) Inhalt lesen und validieren, BEVOR irgendetwas überschrieben wird.
    let content = std::fs::read_to_string(&source)
        .map_err(|e| ecb(ECB_CORE, format!("Backup nicht lesbar: {e}")))?;
    serde_yaml::from_str::<MatchDoc>(&content).map_err(|e| {
        ecb(
            ECB_CORE,
            format!("Das Backup ist beschädigt, Wiederherstellung abgebrochen: {e}"),
        )
    })?;

    // 2) Aktuellen Stand sichern (überschreibt .yml.bak — auch wenn genau das
    //    gerade die Quelle ist; deshalb liegt der Inhalt oben schon im Speicher).
    if path.exists() {
        std::fs::copy(path, path.with_extension("yml.bak"))
            .map_err(|e| ecb(ECB_CORE, format!("Sicherung fehlgeschlagen: {e}")))?;
    }

    // 3) Atomar zurückschreiben.
    let tmp = path.with_extension("yml.restore.tmp");
    std::fs::write(&tmp, content.as_bytes())
        .map_err(|e| ecb(ECB_CORE, format!("Schreiben fehlgeschlagen: {e}")))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| ecb(ECB_CORE, format!("Atomares Umbenennen fehlgeschlagen: {e}")))?;
    Ok(())
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

/// Autostart beim Systemstart. `service check` meldet den Zustand im Klartext
/// („registered as a service" / „… not registered …"), deshalb wird der Text
/// ausgewertet und nicht der Exit-Code.
#[tauri::command]
pub fn autostart_enabled() -> bool {
    match run_espanso(&["service", "check"]) {
        Ok(r) => {
            let out = r.output.to_lowercase();
            out.contains("registered") && !out.contains("not registered")
        }
        Err(_) => false,
    }
}

#[tauri::command]
pub fn autostart_enable() -> Result<CmdResult, String> {
    run_espanso(&["service", "register"])
}

#[tauri::command]
pub fn autostart_disable() -> Result<CmdResult, String> {
    run_espanso(&["service", "unregister"])
}

// ---- Snippet testen ----------------------------------------------------------

/// Manche espanso-Unterbefehle beenden sich auch im Fehlerfall mit Exit-Code 0
/// und schreiben die Ursache nur nach stdout (z. B. `match exec` →
/// „unable to exec match: Worker process is not running"). Auf `success` allein
/// zu vertrauen, meldete einen Erfolg, den es nicht gab — deshalb wird
/// zusätzlich die Ausgabe ausgewertet.
///
/// Wird von jedem Befehl genutzt, dessen Ergebnis die Oberfläche als Erfolg
/// meldet. Siehe Exit-Code-Tabelle in AGENTS.md §4.
fn cli_failed(r: &CmdResult) -> bool {
    if !r.success {
        return true;
    }
    let out = r.output.to_lowercase();
    out.contains("unable to") || out.contains("error")
}

/// Expandiert ein Snippet sofort — der Text landet im gerade aktiven Fenster.
#[tauri::command]
pub fn match_exec(trigger: String) -> Result<CmdResult, String> {
    let trigger = trigger.trim();
    if trigger.is_empty() {
        return Err(ecb(ECB_INPUT, "Kein Trigger angegeben."));
    }
    let r = run_espanso(&["match", "exec", "--trigger", trigger])?;
    if cli_failed(&r) {
        return Err(ecb(ECB_FLOW, r.output));
    }
    Ok(r)
}

// ---- Diagnose ----------------------------------------------------------------

/// Rohes Log der Engine — die erste Anlaufstelle, wenn nichts expandiert.
#[tauri::command]
pub fn engine_log() -> Result<CmdResult, String> {
    run_espanso(&["log"])
}

/// macOS: „Secure Input" blockiert die Texteingabe global (typisch nach einem
/// Passwort-Dialog). espanso bringt dafür einen eigenen Workaround mit.
#[tauri::command]
pub fn fix_secure_input() -> Result<CmdResult, String> {
    let r = run_espanso(&["workaround", "secure-input"])?;
    if cli_failed(&r) {
        return Err(ecb(ECB_FLOW, r.output));
    }
    Ok(r)
}

/// macOS: Systemeinstellungen → Bedienungshilfen öffnen. Ohne diese Freigabe
/// startet der espanso-Worker nicht („start: timed out").
#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        cmd.arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
        no_window(&mut cmd);
        cmd.spawn()
            .map_err(|e| ecb(ECB_CORE, format!("Systemeinstellungen nicht zu öffnen: {e}")))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Err(ecb(
        ECB_INPUT,
        "Diese Einstellung gibt es nur unter macOS.",
    ))
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

    // ---- Einfache Variablen ------------------------------------------------

    #[test]
    fn simple_vars_are_editable_foreign_vars_are_not() {
        // Von uns erzeugt → editierbar.
        let ours: MatchDoc = serde_yaml::from_str(
            "matches:\n  - trigger: \":d\"\n    replace: \"{{date}}\"\n    vars:\n      - name: date\n        type: date\n        params:\n          format: \"%d.%m.%Y\"\n",
        )
        .unwrap();
        assert!(!is_advanced(&ours.matches[0]), "eigene vars sind editierbar");
        assert_eq!(snippet_view(0, &ours.matches[0]).kind, "dynamisch");

        // Handarbeit des Nutzers (shell/script/andere Namen) → read-only.
        for yaml in [
            "matches:\n  - trigger: \":s\"\n    replace: \"{{out}}\"\n    vars:\n      - name: out\n        type: shell\n        params:\n          cmd: \"date\"\n",
            "matches:\n  - trigger: \":d\"\n    replace: \"{{mydate}}\"\n    vars:\n      - name: mydate\n        type: date\n        params:\n          format: \"%Y\"\n",
            // richtiger Name, aber zusätzliches Feld → nicht unser Schema
            "matches:\n  - trigger: \":c\"\n    replace: \"{{clipboard}}\"\n    vars:\n      - name: clipboard\n        type: clipboard\n        inject: true\n",
        ] {
            let doc: MatchDoc = serde_yaml::from_str(yaml).unwrap();
            assert!(
                is_advanced(&doc.matches[0]),
                "fremde vars müssen read-only bleiben: {yaml}"
            );
        }
    }

    #[test]
    fn saving_builds_and_removes_simple_vars() {
        let path = tmp_path("simplevars");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, "matches: []\n").unwrap();
        let p = path.display().to_string();

        // Anlegen mit beiden Platzhaltern → vars entstehen.
        save_snippet(p.clone(), None, ":x".into(), "{{date}} — {{clipboard}}".into(), None, None)
            .unwrap();
        let doc = read_doc(&path).unwrap();
        let Some(Value::Sequence(vars)) = doc.matches[0].extra.get("vars") else {
            panic!("vars fehlen");
        };
        assert_eq!(vars.len(), 2);
        assert!(!is_advanced(&doc.matches[0]), "bleibt editierbar");

        // Platzhalter entfernen → vars verschwinden wieder (kein Leichenblock).
        save_snippet(p.clone(), Some(0), ":x".into(), "nur Text".into(), None, Some(":x".into()))
            .unwrap();
        let doc = read_doc(&path).unwrap();
        assert!(!doc.matches[0].extra.contains_key("vars"), "vars müssen weg sein");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("yml.bak"));
        let _ = std::fs::remove_file(path.with_extension("yml.orig"));
    }

    #[test]
    fn saving_keeps_custom_date_format() {
        // Der Nutzer hat das Format von Hand auf ISO gestellt — ein Speichern
        // über die GUI darf das nicht auf den Default zurücksetzen.
        let path = tmp_path("dateformat");
        let _ = std::fs::remove_file(&path);
        std::fs::write(
            &path,
            "matches:\n  - trigger: \":d\"\n    replace: \"{{date}}\"\n    vars:\n      - name: date\n        type: date\n        params:\n          format: \"%Y-%m-%d\"\n",
        )
        .unwrap();

        save_snippet(
            path.display().to_string(),
            Some(0),
            ":d".into(),
            "Heute: {{date}}".into(),
            None,
            Some(":d".into()),
        )
        .unwrap();

        let doc = read_doc(&path).unwrap();
        assert_eq!(existing_date_format(&doc.matches[0]), "%Y-%m-%d");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("yml.bak"));
        let _ = std::fs::remove_file(path.with_extension("yml.orig"));
    }

    // ---- Pfad-Schutz --------------------------------------------------------

    #[test]
    fn ensure_within_blocks_traversal() {
        let base = std::env::temp_dir().join("snippetaffairs_base");
        let sub = base.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let inside = base.join("ok.yml");
        std::fs::write(&inside, "matches: []\n").unwrap();

        assert!(ensure_within(&inside, &base).is_ok());
        assert!(ensure_within(&sub.join("neu.yml"), &base).is_ok(), "noch nicht existent, aber drin");

        // Ausbruch per ..
        let outside = base.join("../snippetaffairs_escape.yml");
        std::fs::write(std::env::temp_dir().join("snippetaffairs_escape.yml"), "x").unwrap();
        let err = ensure_within(&outside, &base).unwrap_err();
        assert!(err.starts_with("ECB:AI-1955-INPUT|"), "war: {err}");

        // Absoluter Fremdpfad
        assert!(ensure_within(Path::new("/etc/hosts"), &base).is_err());

        let _ = std::fs::remove_file(&inside);
        let _ = std::fs::remove_file(std::env::temp_dir().join("snippetaffairs_escape.yml"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn safe_file_stem_sanitizes() {
        assert_eq!(safe_file_stem(" meine snippets ").unwrap(), "meine-snippets");
        assert_eq!(safe_file_stem("../../etc/passwd").unwrap(), "------etc-passwd");
        assert!(safe_file_stem("   ").is_err());
    }

    // ---- Trigger-Kollisionen ------------------------------------------------

    #[test]
    fn conflicts_across_files_and_packages() {
        let dir = std::env::temp_dir().join("snippetaffairs_conflicts");
        let pkg = dir.join("packages").join("emoji-pack");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&pkg).unwrap();

        std::fs::write(
            dir.join("base.yml"),
            "matches:\n  - trigger: \":hi\"\n    replace: \"Hallo\"\n  - trigger: \":ok\"\n    replace: \"Okay\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("arbeit.yml"),
            "matches:\n  - trigger: \":hi\"\n    replace: \"Guten Tag\"\n",
        )
        .unwrap();
        // Auch ein Hub-Paket kann einen Trigger belegen.
        std::fs::write(
            pkg.join("package.yml"),
            "matches:\n  - triggers: [\":ok\", \":yes\"]\n    replace: \"👍\"\n",
        )
        .unwrap();

        let conflicts = conflicts_in(&dir);
        let names: Vec<_> = conflicts.iter().map(|c| c.trigger.as_str()).collect();
        assert_eq!(names, vec![":hi", ":ok"], "nur echte Doppelungen");

        let hi = &conflicts[0];
        assert_eq!(hi.sites.len(), 2);
        let sources: Vec<_> = hi.sites.iter().map(|s| s.source.as_str()).collect();
        assert!(sources.contains(&"arbeit") && sources.contains(&"base"));

        // Der Paket-Treffer wird als Paket ausgewiesen, nicht als Dateiname.
        let ok = &conflicts[1];
        assert!(ok.sites.iter().any(|s| s.source == "Paket: emoji-pack"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- Backup-Wiederherstellung -------------------------------------------

    #[test]
    fn restore_swaps_current_and_backup_and_keeps_comments() {
        let path = tmp_path("restore");
        let bak = path.with_extension("yml.bak");
        let orig = path.with_extension("yml.orig");
        for f in [&path, &bak, &orig] {
            let _ = std::fs::remove_file(f);
        }

        let original = "# handgepflegt\nmatches:\n  - trigger: \":alt\"\n    replace: \"Alt\"\n";
        std::fs::write(&path, original).unwrap();

        // Eine GUI-Änderung: legt .orig + .bak an.
        let mut doc = read_doc(&path).unwrap();
        doc.matches.push(EspMatch {
            trigger: Some(":neu".into()),
            replace: Some("Neu".into()),
            ..Default::default()
        });
        write_doc(&path, &doc).unwrap();
        assert!(orig.exists() && bak.exists());

        // .orig zurückspielen → Kommentar ist wieder da, aktueller Stand liegt im .bak.
        restore_from(&path, "orig").unwrap();
        let restored = std::fs::read_to_string(&path).unwrap();
        assert!(restored.contains("# handgepflegt"), "Kommentar muss zurückkommen");
        assert!(!restored.contains(":neu"));
        assert!(
            std::fs::read_to_string(&bak).unwrap().contains(":neu"),
            "der überschriebene Stand muss im .bak liegen (Rückweg offen)"
        );

        // Und wieder vorwärts.
        restore_from(&path, "bak").unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains(":neu"));

        for f in [&path, &bak, &orig] {
            let _ = std::fs::remove_file(f);
        }
    }

    #[test]
    fn restore_refuses_corrupt_backup() {
        let path = tmp_path("corrupt");
        let bak = path.with_extension("yml.bak");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, "matches: []\n").unwrap();
        std::fs::write(&bak, "matches: [\n").unwrap(); // kaputt

        let err = restore_from(&path, "bak").unwrap_err();
        assert!(err.starts_with("ECB:AI-1956-CORE|"), "war: {err}");
        // Die intakte Datei darf nicht angefasst worden sein.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "matches: []\n");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&bak);
    }

    #[test]
    fn cli_failure_is_detected_despite_exit_zero() {
        // Verifiziertes Verhalten von espanso 2.3.0: Exit-Code 0, Fehler nur im Text.
        let failed = CmdResult {
            success: true,
            output: "unable to exec match: Worker process is not running, please start Espanso first.".into(),
        };
        assert!(cli_failed(&failed), "Fehlertext muss als Fehler gelten");

        let ok = CmdResult { success: true, output: String::new() };
        assert!(!cli_failed(&ok));

        let registered = CmdResult { success: true, output: "service registered correctly!".into() };
        assert!(!cli_failed(&registered), "Erfolgsmeldungen dürfen nicht als Fehler gelten");

        let hard = CmdResult { success: false, output: String::new() };
        assert!(cli_failed(&hard));
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
