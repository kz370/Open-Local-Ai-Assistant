// Typed wrappers around Tauri commands and events.

import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppErrorPayload,
  AppInfo,
  AudioDevice,
  CapabilityReport,
  CatalogEntry,
  ChatEvent,
  ConnectionStatus,
  Conversation,
  ImportCandidate,
  InstalledModel,
  ListenMode,
  McpServerConfig,
  Message,
  ModelInfo,
  ModelSelection,
  Permission,
  PrivacyStatus,
  SearchHit,
  ServerStatus,
  Settings,
  VoiceInfo,
} from "./types";

export function isAppError(e: unknown): e is AppErrorPayload {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

export function toAppError(e: unknown): AppErrorPayload {
  if (isAppError(e)) return e;
  return { code: "other", detail: e instanceof Error ? e.message : String(e) };
}

export const ipc = {
  // settings & app
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  shortcutErrors: () => invoke<string[]>("shortcut_errors"),
  appInfo: () => invoke<AppInfo>("app_info"),
  openFolder: (which: "logs" | "models" | "data") => invoke<void>("open_folder", { which }),
  readLogTail: (lines?: number) => invoke<string>("read_log_tail", { lines }),
  scanCapabilities: () => invoke<CapabilityReport>("scan_capabilities"),
  privacyStatus: () => invoke<PrivacyStatus>("privacy_status"),
  appReady: () => invoke<void>("app_ready"),
  completeFirstRun: () => invoke<void>("complete_first_run"),
  quit: () => invoke<void>("quit_app"),

  // LM Studio
  lmstudioTest: (url?: string) => invoke<ConnectionStatus>("lmstudio_test", { url }),
  lmstudioModels: (refresh: boolean) => invoke<ModelInfo[]>("lmstudio_models", { refresh }),
  lmstudioAutoSelection: () => invoke<ModelSelection | null>("lmstudio_auto_selection"),

  // window
  setCompact: (compact: boolean) => invoke<void>("window_set_compact", { compact }),
  hideWindow: () => invoke<void>("window_hide"),
  minimizeWindow: () => invoke<void>("window_minimize"),
  showMain: () => invoke<void>("window_show_main"),
  bubbleOpenChat: () => invoke<void>("bubble_open_chat"),
  openSettings: (section?: string) => invoke<void>("open_settings_window", { section }),

  // chat
  chatSend: (
    input: { turnId: string; conversationId: string | null; text: string; spokenLanguage: string | null; voice: boolean },
    onEvent: (e: ChatEvent) => void,
  ) => {
    const channel = new Channel<ChatEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("chat_send", { input, onEvent: channel });
  },
  chatStop: (turnId: string) => invoke<void>("chat_stop", { turnId }),
  chatConfirmTool: (callId: string, approved: boolean) => invoke<boolean>("chat_confirm_tool", { callId, approved }),

  // conversations
  convList: (limit = 200) => invoke<Conversation[]>("conv_list", { limit }),
  convSearch: (query: string) => invoke<SearchHit[]>("conv_search", { query }),
  convGet: (id: string) => invoke<[Conversation, Message[]]>("conv_get", { id }),
  convRename: (id: string, title: string) => invoke<void>("conv_rename", { id, title }),
  convDelete: (id: string) => invoke<void>("conv_delete", { id }),
  convSetLast: (id: string | null) => invoke<void>("conv_set_last", { id }),
  convExport: (ids: string[], format: "json" | "markdown" | "txt", path: string) => invoke<void>("conv_export", { ids, format, path }),
  convImport: (path: string) => invoke<string[]>("conv_import", { path }),

  // voice
  audioDevices: () => invoke<{ inputs: AudioDevice[]; outputs: AudioDevice[] }>("audio_devices"),
  voiceStart: (mode: ListenMode) => invoke<void>("voice_start", { mode }),
  voiceStop: (discard: boolean) => invoke<ListenMode | null>("voice_stop", { discard }),
  voiceStatus: () => invoke<ListenMode | null>("voice_status"),
  ttsVoices: () => invoke<VoiceInfo[]>("tts_voices"),
  ttsSpeak: (text: string, language?: string | null) => invoke<void>("tts_speak", { text, language }),
  ttsTest: (language: string) => invoke<void>("tts_test", { language }),
  ttsStop: () => invoke<void>("tts_stop"),
  ttsReplayLast: () => invoke<boolean>("tts_replay_last"),

  // models
  modelsCatalog: () => invoke<CatalogEntry[]>("models_catalog"),
  modelsInstalled: () => invoke<InstalledModel[]>("models_installed"),
  modelsDownload: (ids: string[]) => invoke<void>("models_download", { ids }),
  modelsCancel: (id: string) => invoke<void>("models_cancel", { id }),
  modelsDelete: (id: string) => invoke<void>("models_delete", { id }),

  // MCP
  mcpList: () => invoke<ServerStatus[]>("mcp_list"),
  mcpSave: (config: McpServerConfig) => invoke<McpServerConfig>("mcp_save", { config }),
  mcpDelete: (id: string) => invoke<void>("mcp_delete", { id }),
  mcpSetEnabled: (id: string, enabled: boolean) => invoke<void>("mcp_set_enabled", { id, enabled }),
  mcpReconnect: (id: string) => invoke<void>("mcp_reconnect", { id }),
  mcpSetPermission: (serverId: string, tool: string, permission: Permission) =>
    invoke<Permission>("mcp_set_permission", { serverId, tool, permission }),
  mcpImportPreview: () => invoke<ImportCandidate[]>("mcp_import_preview"),
  mcpImport: (names: string[]) => invoke<number>("mcp_import", { names }),
};

export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}

export function newId(): string {
  return crypto.randomUUID();
}
