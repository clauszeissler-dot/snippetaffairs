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
  saveSnippet: (args: {
    file_path: string;
    index: number | null;
    trigger: string;
    replace: string;
    label: string | null;
  }) => invoke<void>("save_snippet", args),
  deleteSnippet: (file_path: string, index: number) =>
    invoke<void>("delete_snippet", { file_path, index }),
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
