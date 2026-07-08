mod espanso;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            espanso::get_espanso_info,
            espanso::list_snippets,
            espanso::save_snippet,
            espanso::delete_snippet,
            espanso::create_match_file,
            espanso::service_status,
            espanso::service_start,
            espanso::service_stop,
            espanso::service_restart,
            espanso::package_list,
            espanso::package_install,
            espanso::package_uninstall,
            espanso::package_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
