// Types mirroring the Rust IPC payloads (camelCase serde).

export type LangCode = "en" | "ar" | "de";
export type LangSetting = "auto" | LangCode;

export interface WindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Settings {
  general: {
    theme: "system" | "light" | "dark";
    accent: "teal" | "blue" | "green" | "amber" | "rose" | "slate";
    assistantName: string;
    highContrast: boolean;
    fontScale: number;
    startWithOs: boolean;
    startMinimized: boolean;
    alwaysOnTop: boolean;
    windowPosition: "bottom-right" | "bottom-left" | "center" | "custom";
    window: WindowGeometry;
    compact: boolean;
    globalShortcut: string;
    pushToTalkShortcut: string;
    developerMode: boolean;
    firstRunComplete: boolean;
    logConversationContent: boolean;
    bubbleX: number | null;
    bubbleY: number | null;
  };
  ai: {
    serverUrl: string;
    modelMode: "auto" | "manual";
    model: string | null;
    temperature: number;
    contextLength: number | null;
    maxTokens: number | null;
    systemPrompt: string;
    streaming: boolean;
    requestTimeoutSecs: number;
    showReasoning: boolean;
    modelAliases: Record<string, string>;
  };
  language: { responseLanguage: LangSetting };
  stt: {
    model: string;
    language: LangSetting;
    microphone: string | null;
    micOnly: boolean;
    hardware: string;
    autoSubmit: boolean;
    handsFree: boolean;
    pushToTalk: boolean;
    vadThreshold: number;
    silenceMs: number;
    extraModelDirs: string[];
    callView: boolean;
    autoStopSilenceSecs: number;
    handsFreeTimeoutSecs: number;
  };
  tts: {
    speakResponses: boolean;
    voiceEn: string;
    voiceAr: string;
    voiceDe: string;
    speed: number;
    volume: number;
    outputDevice: string | null;
    preferredGender: "any" | "female" | "male";
  };
  dictation: {
    enabled: boolean;
    shortcut: string;
    mode: "hold" | "toggle";
    correctionEnabled: boolean;
    correctionModel: string | null;
    insertMethod: "type" | "paste";
    addTrailingSpace: boolean;
  };
  lastConversationId: string | null;
  version: number;
}

export interface AppErrorPayload {
  code: string;
  detail: string;
}

export interface ModelInfo {
  id: string;
  displayName: string;
  kind: string;
  sizeBytes: number | null;
  params: string | null;
  quantization: string | null;
  bitsPerWeight: number | null;
  maxContextLength: number | null;
  loaded: boolean;
  loadedContextLength: number | null;
  toolUse: boolean;
  vision: boolean;
  reasoning: boolean;
}

export interface ModelSelection {
  modelId: string;
  needsLoad: boolean;
  score: number;
  reasons: string[];
}

export interface ConnectionStatus {
  connected: boolean;
  serverUrl: string;
  modelCount: number;
  latencyMs: number;
  api: string;
}

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  model: string | null;
  language: string | null;
}

export interface Source {
  url: string;
  title?: string | null;
}

export type ToolCategory = "search" | "fetch" | "read" | "write" | "execute" | "other";
export type Permission = "allow" | "ask" | "deny";

export interface ActivityRecord {
  callId: string;
  serverName: string;
  toolName: string;
  category: ToolCategory;
  args: unknown;
  ok: boolean;
  denied: boolean;
  durationMs: number;
  resultPreview: string;
}

export interface Message {
  id: string;
  conversationId: string;
  role: "user" | "assistant" | "tool" | "assistant_tool_calls";
  content: string;
  language: string | null;
  reasoning?: string | null;
  sources?: Source[] | null;
  toolActivity?: ActivityRecord[] | null;
  createdAt: string;
}

export interface SearchHit {
  conversation: Conversation;
  snippet: string;
}

export type ChatEvent =
  | { type: "started"; turnId: string; conversationId: string; userMessage: Message; model: string; language: string; newConversation: boolean }
  | { type: "delta"; turnId: string; text: string }
  | { type: "reasoning"; turnId: string; text: string }
  | { type: "toolStarted"; turnId: string; callId: string; serverName: string; toolName: string; category: ToolCategory; args: unknown }
  | { type: "toolAwaitingConfirmation"; turnId: string; callId: string; serverName: string; toolName: string; category: ToolCategory; args: unknown }
  | { type: "toolFinished"; turnId: string; callId: string; ok: boolean; durationMs: number; resultPreview: string; sources: Source[]; denied: boolean }
  | { type: "done"; turnId: string; message: Message }
  | { type: "error"; turnId: string; code: string; detail: string; partialMessage: Message | null };

export type ListenMode = "pushToTalk" | "handsFree" | "dictation" | "test";

export type VoiceEvent =
  | { type: "state"; mode: ListenMode; state: "listening" | "transcribing" | "idle"; device: string | null; streaming: boolean }
  | { type: "partial"; mode: ListenMode; text: string }
  | { type: "level"; mode: ListenMode; value: number }
  | { type: "transcript"; mode: ListenMode; text: string; language: string | null; audioMs: number; elapsedMs: number }
  | { type: "error"; mode: ListenMode; code: string; detail: string };

export type TtsEvent =
  | { type: "speaking"; tag: string }
  | { type: "paused" }
  | { type: "resumed" }
  | { type: "idle" }
  | { type: "voiceUnavailable"; language: string }
  | { type: "error"; detail: string };

export interface AudioDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface VoiceInfo {
  id: string;
  modelId: string;
  name: string;
  language: string;
  speakerId: number;
  engine: string;
  quality: number;
  gender: string;
}

export interface CatalogEntry {
  id: string;
  kind: "stt" | "vad" | "tts";
  engine: string;
  name: string;
  languages: string[];
  quality: number;
  minRamGb: number;
  license: string;
  downloadBytes: number;
  installed: boolean;
  recommended: boolean;
  downloading: boolean;
}

export interface InstalledModel {
  id: string;
  name: string;
  kind: "stt" | "vad" | "tts";
  engine: string;
  languages: string[];
  path: string;
  source: "catalog" | "custom";
  family: string | null;
  streaming: boolean;
  gender: string;
  sizeBytes: number;
}

export interface IncompatibleModel {
  name: string;
  path: string;
  reason: string;
}

export interface DownloadProgress {
  modelId: string;
  state: "downloading" | "verifying" | "extracting" | "done" | "error" | "cancelled";
  downloadedBytes: number;
  totalBytes: number;
  error: string | null;
}

export interface McpServerConfig {
  id: string;
  name: string;
  description: string;
  transport: "stdio" | "http";
  command: string | null;
  args: string[];
  env: Record<string, string>;
  url: string | null;
  headers: Record<string, string>;
  enabled: boolean;
  source: string;
  createdAt: string;
}

export interface ToolView {
  name: string;
  description: string;
  category: ToolCategory;
  permission: Permission;
  defaultPermission: Permission;
}

export interface ServerStatus {
  config: McpServerConfig;
  state: "disabled" | "connecting" | "connected" | "error" | "offline";
  error: string | null;
  tools: ToolView[];
  internet: boolean;
}

export interface ImportCandidate {
  name: string;
  transport: string;
  command: string | null;
  args: string[];
  url: string | null;
  envKeys: string[];
  alreadyConfigured: boolean;
}

export interface GpuInfo {
  name: string;
  vendor: string;
  vramBytes: number;
}

export interface HardwareInfo {
  os: string;
  cpuName: string;
  physicalCores: number;
  logicalCores: number;
  totalRamBytes: number;
  availableRamBytes: number;
  gpus: GpuInfo[];
}

export interface CapabilityReport {
  lmStudio: {
    connected: boolean;
    serverUrl: string;
    api: string | null;
    modelCount: number;
    errorCode: string | null;
    errorDetail: string | null;
    selection: ModelSelection | null;
    loadedModels: string[];
  };
  hardware: HardwareInfo;
  voice: {
    sttModel: string | null;
    vadReady: boolean;
    ttsEn: string | null;
    ttsAr: string | null;
    ttsDe: string | null;
    installed: InstalledModel[];
    recommendation: { stt: string; vad: string; ttsEn: string; ttsDe: string; ttsAr: string };
    acceleration: string;
  };
  microphones: AudioDevice[];
  audioOutputs: AudioDevice[];
  mcp: { enabled: number; connected: number; internet: boolean };
}

export interface AppInfo {
  version: string;
  paths: { dataDir: string; modelsDir: string; logsDir: string; database: string };
  hardware: HardwareInfo;
  os: string;
}

export interface PrivacyStatus {
  llm: string;
  llmServer: string;
  llmIsLocalAddress: boolean;
  sttLocal: boolean;
  ttsLocal: boolean;
  conversationsLocal: boolean;
  settingsLocal: boolean;
  telemetry: boolean;
  internetViaMcp: boolean;
  internetServers: string[];
}

export interface DictationStateEvent {
  state: "listening" | "transcribing" | "idle" | "correcting" | "inserted" | "empty" | "cancelled" | "error";
  text?: string;
  error?: AppErrorPayload;
  result?: { raw: string; inserted: string; corrected: boolean; correctionError: string | null };
}
