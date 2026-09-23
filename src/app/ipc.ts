// Typed wrappers around Tauri commands and events.

import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppErrorPayload,
  AppInfo,
  AttachResult,
  Attachment,
  AudioDevice,
  CapabilityReport,
  CatalogEntry,
  DictationEntry,
  ChatEvent,
  ConnectionStatus,
  GpuStatus,
  SilmaStatus,
  Conversation,
  ImportCandidate,
  PublicSearxInstance,
  SearchTestResult,
  IncompatibleModel,
  InstalledModel,
  ListenMode,
  McpServerConfig,
  MemoryItem,
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
  shortcutsCapture: (capturing: boolean) => invoke<void>("shortcuts_capture", { capturing }),
  appInfo: () => invoke<AppInfo>("app_info"),
  openFolder: (which: "logs" | "models" | "data") => invoke<void>("open_folder", { which }),
  readLogTail: (lines?: number) => invoke<string>("read_log_tail", { lines }),
  scanCapabilities: () => invoke<CapabilityReport>("scan_capabilities"),
  privacyStatus: () => invoke<PrivacyStatus>("privacy_status"),
  appReady: () => invoke<void>("app_ready"),
  completeFirstRun: () => invoke<void>("complete_first_run"),
  quit: () => invoke<void>("quit_app"),
  settingsExport: (path: string, includeKeys: boolean) => invoke<void>("settings_export", { path, includeKeys }),
  settingsImport: (path: string) => invoke<Settings>("settings_import", { path }),

  // LM Studio
  lmstudioTest: (url?: string, apiKey?: string, provider?: string) => invoke<ConnectionStatus>("lmstudio_test", { url, apiKey, provider }),
  lmstudioModels: (refresh: boolean) => invoke<ModelInfo[]>("lmstudio_models", { refresh }),
  lmstudioAutoSelection: () => invoke<ModelSelection | null>("lmstudio_auto_selection"),

  // window
  setCompact: (compact: boolean) => invoke<void>("window_set_compact", { compact }),
  hideWindow: () => invoke<void>("window_hide"),
  minimizeWindow: () => invoke<void>("window_minimize"),
  toggleMaximize: () => invoke<boolean>("window_toggle_maximize"),
  showMain: () => invoke<void>("window_show_main"),
  bubbleOpenChat: () => invoke<void>("bubble_open_chat"),
  openSettings: (section?: string) => invoke<void>("open_settings_window", { section }),

  // chat
  chatSend: (
    input: { turnId: string; conversationId: string | null; text: string; spokenLanguage: string | null; voice: boolean; attachmentIds: string[] },
    onEvent: (e: ChatEvent) => void,
  ) => {
    const channel = new Channel<ChatEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("chat_send", { input, onEvent: channel });
  },
  chatStop: (turnId: string) => invoke<void>("chat_stop", { turnId }),
  chatConfirmTool: (callId: string, approved: boolean) => invoke<boolean>("chat_confirm_tool", { callId, approved }),

  // attachments
  attachFiles: (paths: string[]) => invoke<AttachResult>("attach_files", { paths }),
  attachBytes: (name: string, mime: string | null, data: string) => invoke<Attachment>("attach_bytes", { name, mime, data }),
  attachText: (name: string, text: string) => invoke<Attachment>("attach_text", { name, text }),
  attachRemove: (id: string) => invoke<void>("attach_remove", { id }),
  attachmentDataUrl: (id: string, mime: string) => invoke<string>("attachment_data_url", { id, mime }),
  attachmentText: (id: string) => invoke<string>("attachment_text", { id }),

  // conversations
  convList: (limit = 200) => invoke<Conversation[]>("conv_list", { limit }),
  convSearch: (query: string) => invoke<SearchHit[]>("conv_search", { query }),
  convGet: (id: string) => invoke<[Conversation, Message[]]>("conv_get", { id }),
  convRename: (id: string, title: string) => invoke<void>("conv_rename", { id, title }),
  convDelete: (id: string) => invoke<void>("conv_delete", { id }),
  convClearAll: () => invoke<number>("conv_clear_all"),
  convSetLast: (id: string | null) => invoke<void>("conv_set_last", { id }),
  convExport: (ids: string[], format: "json" | "markdown" | "txt", path: string) => invoke<void>("conv_export", { ids, format, path }),
  convImport: (path: string) => invoke<string[]>("conv_import", { path }),

  // voice
  audioDevices: () => invoke<{ inputs: AudioDevice[]; outputs: AudioDevice[] }>("audio_devices"),
  voiceStart: (mode: ListenMode) => invoke<void>("voice_start", { mode }),
  voiceStop: (discard: boolean) => invoke<ListenMode | null>("voice_stop", { discard }),
  dictationCancel: () => invoke<void>("dictation_cancel"),
  dictationInsertNow: () => invoke<void>("dictation_insert_now"),
  dictationConfirm: (text: string) => invoke<void>("dictation_confirm", { text }),
  dictationRetry: () => invoke<void>("dictation_retry"),
  dictationHistory: () => invoke<DictationEntry[]>("dictation_history"),
  dictationHistoryDelete: (id: string) => invoke<void>("dictation_history_delete", { id }),
  dictationHistoryClear: () => invoke<void>("dictation_history_clear"),
  dictationResetOverlayPosition: () => invoke<void>("dictation_reset_overlay_position"),
  voiceStatus: () => invoke<ListenMode | null>("voice_status"),
  voiceSetMuted: (muted: boolean) => invoke<boolean>("voice_set_muted", { muted }),
  voiceMuted: () => invoke<boolean>("voice_muted"),
  ttsVoices: () => invoke<VoiceInfo[]>("tts_voices"),
  ttsSpeak: (text: string, language?: string | null, tag?: string) => invoke<void>("tts_speak", { text, language, tag }),
  ttsSetPaused: (paused: boolean) => invoke<{ speaking: boolean; paused: boolean }>("tts_set_paused", { paused }),
  ttsState: () => invoke<{ speaking: boolean; paused: boolean }>("tts_state"),
  ttsTest: (language: string) => invoke<void>("tts_test", { language }),
  ttsStop: () => invoke<void>("tts_stop"),
  ttsReplayLast: () => invoke<boolean>("tts_replay_last"),

  // models
  modelsCatalog: () => invoke<CatalogEntry[]>("models_catalog"),
  modelsInstalled: () => invoke<InstalledModel[]>("models_installed"),
  modelsIncompatible: () => invoke<IncompatibleModel[]>("models_incompatible"),
  modelsDownload: (ids: string[]) => invoke<void>("models_download", { ids }),
  modelsCancel: (id: string) => invoke<void>("models_cancel", { id }),
  modelsDelete: (id: string) => invoke<void>("models_delete", { id }),

  // GPU pack (CUDA libraries for the local speech models)
  gpuStatus: () => invoke<GpuStatus>("gpu_status"),
  gpuSetEnabled: (enabled: boolean) => invoke<void>("gpu_set_enabled", { enabled }),
  gpuInstall: () => invoke<void>("gpu_install"),
  gpuCancel: () => invoke<void>("gpu_cancel"),
  gpuRemove: () => invoke<void>("gpu_remove"),

  // SILMA natural Arabic voice (PyTorch helper managed by the app)
  silmaStatus: () => invoke<SilmaStatus>("silma_status"),
  silmaInstall: () => invoke<void>("silma_install"),
  silmaCancel: () => invoke<void>("silma_cancel"),
  silmaTest: () => invoke<void>("silma_test"),

  // models in memory
  memoryStatus: () => invoke<MemoryItem[]>("memory_status"),
  memoryLoad: (key: string) => invoke<void>("memory_load", { key }),
  memoryUnload: (key: string) => invoke<void>("memory_unload", { key }),
  silmaRemove: () => invoke<void>("silma_remove"),

  // MCP
  mcpList: () => invoke<ServerStatus[]>("mcp_list"),
  mcpSave: (config: McpServerConfig) => invoke<McpServerConfig>("mcp_save", { config }),
  mcpDelete: (id: string) => invoke<void>("mcp_delete", { id }),
  mcpSetEnabled: (id: string, enabled: boolean) => invoke<void>("mcp_set_enabled", { id, enabled }),
  mcpReconnect: (id: string) => invoke<void>("mcp_reconnect", { id }),
  mcpSetPermission: (serverId: string, tool: string, permission: Permission) =>
    invoke<Permission>("mcp_set_permission", { serverId, tool, permission }),
  /** `null` resets every tool of the server to its default. */
  mcpSetAllPermissions: (serverId: string, permission: Permission | null) =>
    invoke<void>("mcp_set_all_permissions", { serverId, permission }),
  mcpSafeMode: () => invoke<boolean>("mcp_safe_mode"),
  /** Turning safe mode off shows a Windows Hello prompt. */
  mcpSetSafeMode: (enabled: boolean) => invoke<void>("mcp_set_safe_mode", { enabled }),
  mcpImportPreview: () => invoke<ImportCandidate[]>("mcp_import_preview"),
  mcpImport: (names: string[]) => invoke<number>("mcp_import", { names }),
  searchPublicInstances: (refresh: boolean) => invoke<PublicSearxInstance[]>("search_public_instances", { refresh }),
  searchTest: () => invoke<SearchTestResult>("search_test"),
};

export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}

export function newId(): string {
  return crypto.randomUUID();
}
