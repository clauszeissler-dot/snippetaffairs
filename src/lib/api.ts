import { invoke } from "@tauri-apps/api/core";

export interface EspansoInfo {
  installed: boolean;
  version: string | null;
  config_path: string | null;
  match_dir: string | null;
  packages_path: string | null;
}

export interface SnippetView {
  index: number;
  trigger: string;
  replace: string;
  label: string | null;
  advanced: boolean;
  kind: string;
}

export interface FileGroup {
  path: string;
  name: string;
  snippets: SnippetView[];
}

export interface CmdResult {
  success: boolean;
  output: string;
}

export interface ConflictSite {
  source: string;
  file_path: string;
  index: number;
}

export interface TriggerConflict {
  trigger: string;
  sites: ConflictSite[];
}

export interface BackupInfo {
  kind: "bak" | "orig";
  path: string;
  size: number;
  /** Unix-Sekunden, 0 wenn unbekannt. */
  modified: number;
  snippet_count: number;
}

export const api = {
  info: () => invoke<EspansoInfo>("get_espanso_info"),
  listSnippets: () => invoke<FileGroup[]>("list_snippets"),
  // ACHTUNG Argument-Namen: `#[tauri::command]` erwartet sie in camelCase
  // (`filePath`), obwohl die Rust-Signatur `file_path` heißt. snake_case wird
  // zur Laufzeit mit "missing required key" abgelehnt — nicht beim Build.
  // Abgesichert durch src-tauri/src/ipc_tests.rs.
  //
  // expectedTrigger = was die UI an dieser Position anzeigt. Das Backend bricht
  // ab, wenn die Datei zwischenzeitlich extern geändert wurde (Staleness-Guard).
  saveSnippet: (args: {
    filePath: string;
    index: number | null;
    trigger: string;
    replace: string;
    label: string | null;
    expectedTrigger: string | null;
  }) => invoke<void>("save_snippet", args),
  deleteSnippet: (filePath: string, index: number, expectedTrigger: string) =>
    invoke<void>("delete_snippet", { filePath, index, expectedTrigger }),
  createMatchFile: (name: string) => invoke<string>("create_match_file", { name }),
  renameMatchFile: (filePath: string, newName: string) =>
    invoke<string>("rename_match_file", { filePath, newName }),
  deleteMatchFile: (filePath: string) => invoke<void>("delete_match_file", { filePath }),

  triggerConflicts: () => invoke<TriggerConflict[]>("trigger_conflicts"),
  listBackups: (filePath: string) => invoke<BackupInfo[]>("list_backups", { filePath }),
  restoreBackup: (filePath: string, kind: "bak" | "orig") =>
    invoke<void>("restore_backup", { filePath, kind }),

  serviceStatus: () => invoke<CmdResult>("service_status"),
  serviceStart: () => invoke<CmdResult>("service_start"),
  serviceStop: () => invoke<CmdResult>("service_stop"),
  serviceRestart: () => invoke<CmdResult>("service_restart"),

  autostartEnabled: () => invoke<boolean>("autostart_enabled"),
  autostartEnable: () => invoke<CmdResult>("autostart_enable"),
  autostartDisable: () => invoke<CmdResult>("autostart_disable"),

  /** Expandiert das Snippet im gerade aktiven Fenster. */
  matchExec: (trigger: string) => invoke<CmdResult>("match_exec", { trigger }),

  engineLog: () => invoke<CmdResult>("engine_log"),
  fixSecureInput: () => invoke<CmdResult>("fix_secure_input"),
  openAccessibilitySettings: () => invoke<void>("open_accessibility_settings"),
  packageList: () => invoke<CmdResult>("package_list"),
  packageInstall: (name: string) => invoke<CmdResult>("package_install", { name }),
  packageUninstall: (name: string) => invoke<CmdResult>("package_uninstall", { name }),
  packageUpdate: (name: string) => invoke<CmdResult>("package_update", { name }),
};
