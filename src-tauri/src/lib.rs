mod espanso;

#[cfg(test)]
mod ipc_tests;

/// `generate_context!` darf pro Binary nur EINMAL expandiert werden (es bettet
/// u. a. das Info.plist-Symbol ein). Deshalb hier gebündelt — die IPC-Tests
/// holen sich denselben Context inklusive aufgelöster Capabilities.
pub fn context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

/// Hängt Plugins und Commands an einen Builder. Die IPC-Tests reichen hier
/// `mock_builder()` hinein (der den bekannten invoke_key setzt) und prüfen
/// damit exakt die ausgelieferte Command-Registrierung.
pub fn configure<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            espanso::get_espanso_info,
            espanso::list_snippets,
            espanso::save_snippet,
            espanso::delete_snippet,
            espanso::create_match_file,
            espanso::rename_match_file,
            espanso::delete_match_file,
            espanso::trigger_conflicts,
            espanso::list_backups,
            espanso::restore_backup,
            espanso::service_status,
            espanso::service_start,
            espanso::service_stop,
            espanso::service_restart,
            espanso::autostart_enabled,
            espanso::autostart_enable,
            espanso::autostart_disable,
            espanso::match_exec,
            espanso::engine_log,
            espanso::fix_secure_input,
            espanso::open_accessibility_settings,
            espanso::package_list,
            espanso::package_install,
            espanso::package_uninstall,
            espanso::package_update,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure(tauri::Builder::default())
        .run(context())
        .expect("error while running tauri application");
}
