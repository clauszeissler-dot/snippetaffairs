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
  serviceStatus: () => invoke<CmdResult>("service_status"),
  serviceStart: () => invoke<CmdResult>("service_start"),
  serviceStop: () => invoke<CmdResult>("service_stop"),
  serviceRestart: () => invoke<CmdResult>("service_restart"),
  packageList: () => invoke<CmdResult>("package_list"),
  packageInstall: (name: string) => invoke<CmdResult>("package_install", { name }),
  packageUninstall: (name: string) => invoke<CmdResult>("package_uninstall", { name }),
  packageUpdate: (name: string) => invoke<CmdResult>("package_update", { name }),
};
