//! Verifiziert die Naht Frontend → Tauri-Command: die Argument-Namen.
//!
//! Hintergrund: `#[tauri::command]` erwartet die Argumente standardmäßig in
//! **camelCase** (`filePath`), während die Rust-Signatur `file_path` heißt.
//! Sendet das Frontend den snake_case-Namen, schlägt der Aufruf mit
//! "missing required key" fehl — sichtbar erst zur Laufzeit, nicht beim Build.
//!
//! Diese Tests rufen die Commands über die echte IPC-Schicht auf, mit exakt
//! dem Payload, den `src/lib/api.ts` schickt.

use serde_json::json;
use tauri::test::{mock_builder, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{WebviewUrl, WebviewWindowBuilder};

/// Prüft NUR, ob die Argument-Namen ankommen — ohne echte Dateien anzufassen.
///
/// Alle Datei-Commands liegen hinter `ensure_within(match_dir())`. Ein Pfad
/// außerhalb davon wird sauber mit einem ECB-Fehler abgelehnt; ein falscher
/// Argument-Name dagegen mit "missing required key". Genau das unterscheiden wir.
fn assert_args_arrive(cmd: &str, body: serde_json::Value) {
    let err = invoke(cmd, body).expect_err("Fremdpfad muss abgelehnt werden");
    let msg = err.as_str().unwrap_or_default();
    assert!(
        !msg.contains("missing required key"),
        "{cmd}: Argument-Name kommt nicht an → {msg}"
    );
    assert!(msg.starts_with("ECB:"), "{cmd}: unerwarteter Fehler → {msg}");
}

fn invoke(cmd: &str, body: serde_json::Value) -> Result<serde_json::Value, serde_json::Value> {
    // Echter Builder + echter Context: dieselbe Command-Registrierung und
    // dieselbe ACL wie in der ausgelieferten App. Das native macOS-Menü wird
    // abgeschaltet — es ließe sich nur auf dem Main-Thread bauen.
    // mock_builder() liefert die MockRuntime, den bekannten invoke_key und
    // schaltet das native macOS-Menü ab (bräuchte den Main-Thread).
    let app = crate::configure(mock_builder())
        .build(crate::context())
        .expect("app");

    let webview = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
        .build()
        .expect("webview");

    tauri::test::get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            // Windows/Android nutzen ein anderes Schema — sonst verwirft die
            // Webview die Nachricht kommentarlos.
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: body.into(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|r| r.deserialize::<serde_json::Value>().unwrap_or(json!(null)))
}

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("snippetaffairs_ipc_{name}.yml"))
}

/// Der Kern: die Keys aus api.ts müssen ankommen. Schlägt dieser Test fehl,
/// ist Snippet-Speichern in der ausgelieferten App kaputt.
///
/// Seit dem Pfad-Schutz (BEFUND 2) liegen `save_snippet`/`delete_snippet` hinter
/// `ensure_within(match_dir())`. Ein Temp-Pfad liegt außerhalb davon (bzw. der
/// Config-Ordner fehlt in der CI) und wird darum mit einem ECB-Fehler abgewiesen,
/// NICHT mit „missing required key". Genau das prüft `assert_args_arrive`: käme
/// der camelCase-Name nicht an, wäre die Meldung „missing required key". Der
/// Effekt (Datei wird wirklich geschrieben/gelöscht) ist von den Unit-Tests von
/// `save_snippet_in`/`delete_snippet_in` in `espanso.rs` abgedeckt.
#[test]
fn save_snippet_accepts_the_keys_the_frontend_sends() {
    assert_args_arrive(
        "save_snippet",
        json!({
            "filePath": tmp("save").display().to_string(),
            "index": null,
            "trigger": ":hi",
            "replace": "Hallo",
            "label": null,
            "expectedTrigger": null,
        }),
    );
}

#[test]
fn delete_snippet_accepts_the_keys_the_frontend_sends() {
    assert_args_arrive(
        "delete_snippet",
        json!({
            "filePath": tmp("delete").display().to_string(),
            "index": 0,
            "expectedTrigger": ":weg",
        }),
    );
}

/// Ein Fremdpfad (außerhalb des match-Ordners) wird auch über die IPC-Naht
/// abgewiesen, bevor irgendetwas geschrieben wird. Der Staleness-Guard selbst
/// liegt jetzt HINTER `ensure_within(match_dir())` und wird auf Funktionsebene
/// geprüft (`stale_index_is_rejected_before_write` in `espanso.rs`), weil im Test
/// kein echtes `match_dir` bespielt werden kann.
#[test]
fn foreign_path_delete_is_rejected_over_ipc() {
    let path = tmp("stale");
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, "matches:\n  - trigger: \":a\"\n    replace: \"x\"\n").unwrap();

    assert_args_arrive(
        "delete_snippet",
        json!({
            "filePath": path.display().to_string(),
            "index": 0,
            "expectedTrigger": ":a",
        }),
    );

    // Datei unverändert — es wurde nichts geschrieben.
    assert!(std::fs::read_to_string(&path).unwrap().contains(":a"));
    let _ = std::fs::remove_file(&path);
}

/// Die Datei-Commands aus v0.2.0. Sie fassen nichts an: der Fremdpfad wird von
/// `ensure_within` abgelehnt, bevor irgendetwas passiert.
#[test]
fn v020_file_commands_accept_the_keys_the_frontend_sends() {
    let outside = "/tmp/snippetaffairs_definitiv_nicht_im_match_ordner.yml";
    assert_args_arrive("rename_match_file", json!({"filePath": outside, "newName": "neu"}));
    assert_args_arrive("delete_match_file", json!({"filePath": outside}));
    assert_args_arrive("list_backups", json!({"filePath": outside}));
    assert_args_arrive("restore_backup", json!({"filePath": outside, "kind": "bak"}));
}

/// snake_case-Keys sind KEIN gültiger Payload — dieser Test hält fest, warum
/// api.ts camelCase senden muss, damit die Konvention nicht zurückrutscht.
#[test]
fn snake_case_keys_are_rejected() {
    let path = tmp("snake");
    let _ = std::fs::remove_file(&path);
    std::fs::write(&path, "matches: []\n").unwrap();

    let res = invoke(
        "save_snippet",
        json!({
            "file_path": path.display().to_string(),
            "index": null,
            "trigger": ":hi",
            "replace": "Hallo",
            "label": null,
            "expected_trigger": null,
        }),
    );
    assert!(res.is_err(), "snake_case darf NICHT stillschweigend akzeptiert werden");
    let _ = std::fs::remove_file(&path);

    // Auch bei den neuen Commands: snake_case scheitert am Argument-Namen,
    // nicht erst an der Pfadprüfung.
    let err = invoke(
        "rename_match_file",
        json!({"file_path": "/tmp/x.yml", "new_name": "y"}),
    )
    .expect_err("snake_case muss scheitern");
    assert!(
        err.as_str().unwrap_or_default().contains("missing required key"),
        "war: {err:?}"
    );
}
