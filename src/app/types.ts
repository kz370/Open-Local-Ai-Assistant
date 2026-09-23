// Types mirroring the Rust IPC payloads (camelCase serde).

export type LangCode = "en" | "ar" | "de";
export type LangSetting = "auto" | LangCode;

export type ProviderId = "lmstudio" | "openrouter" | "gemini" | "huggingface";

export interface ProviderProfile {
  serverUrl: string;
  apiKey: string | null;
  model: string | null;
  modelMode: "auto" | "manual" | "";
}

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
    preloadModels: boolean;
    /** Per-model startup opt-out keyed by MemoryItem.autoloadKey; missing means on. */
    autoloadModels: Record<string, boolean>;
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
    /** Custom starter prompts for the empty chat screen; empty uses the built-in defaults. */
    suggestedPrompts: string[];
  };
  ai: {
    provider: ProviderId;
    serverUrl: string;
    apiKey: string | null;
    /** URL / key / model remembered for providers that are not currently selected. */
    providerProfiles: Record<string, ProviderProfile>;
    modelMode: "auto" | "manual";
    model: string | null;
    temperature: number;
    contextLength: number | null;
    maxTokens: number | null;
    systemPrompt: string;
    streaming: boolean;
    requestTimeoutSecs: number;
    showReasoning: boolean;
    showStats: boolean;
    modelAliases: Record<string, string>;
    /** Model ids left out of the chat window's model picker. */
    hiddenModels: string[];
    /** Hosted providers: list only models that cost nothing. */
    freeModelsOnly: boolean;
    /** Pasted text longer than this becomes a text attachment. 0 disables it. */
    pasteAsFileChars: number;
  };
  language: {
    responseLanguage: string;
    entries: LanguageEntry[];
    arabicTashkeelEnabled: boolean;
    arabicTashkeelInstruction: string;
  };
  stt: {
    model: string;
    language: string;
    microphone: string | null;
    micOnly: boolean;
    isolateSystemAudio: boolean;
    hardware: "auto" | "cpu";
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
    speed: number;
    volume: number;
    outputDevice: string | null;
    preferredGender: "any" | "female" | "male";
    /** Per-voice hardware override ("auto" | "cpu"), keyed by voice/model id. */
    voiceHardware: Record<string, string>;
    /** Speak only once the whole reply is written. */
    speakAfterReply: boolean;
  };
  dictation: {
    enabled: boolean;
    shortcut: string;
    mode: "hold" | "toggle";
    correctionEnabled: boolean;
    correctionModel: string | null;
    insertMethod: "type" | "paste";
    addTrailingSpace: boolean;
    overlayX: number | null;
    overlayY: number | null;
  };
  silma: {
    hardware: "auto" | "cpu";
  };
  search: {
    enabled: boolean;
    maxResults: number;
    searxngUrl: string;
    searxngEnabled: boolean;
    searxngSource: "local" | "public";
    searxngPublicUrl: string;
    primary: "searxng" | "duckduckgo";
  };
  lastConversationId: string | null;
  version: number;
}

export interface LanguageEntry {
  code: string;
  displayName: string;
  direction: "ltr" | "rtl";
  sttLanguage: string;
  ttsVoice: string;
  builtIn: boolean;
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
  /** Hosted providers only: the model costs nothing to call. */
  free: boolean;
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

export type AttachmentKind = "image" | "text" | "binary";

export interface Attachment {
  id: string;
  name: string;
  mime: string;
  kind: AttachmentKind;
  sizeBytes: number;
  textChars: number;
  truncated: boolean;
  note: string | null;
  createdAt: string;
}

export interface AttachResult {
  attachments: Attachment[];
  /** One message per file that could not be attached. */
  failures: string[];
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
  attachments?: Attachment[] | null;
  stats?: MessageStats | null;
  createdAt: string;
}

/** Generation stats of an assistant reply. */
export interface MessageStats {
  completionTokens: number;
  tokensPerSecond?: number | null;
  firstTokenMs?: number | null;
  contextUsed?: number | null;
  contextLength?: number | null;
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
  | { type: "level"; mode: ListenMode; value: number; bands: number[] }
  | { type: "transcript"; mode: ListenMode; text: string; language: string | null; audioMs: number; elapsedMs: number }
  | { type: "error"; mode: ListenMode; code: string; detail: string };

export type TtsEvent =
  | { type: "speaking"; tag: string }
  | { type: "sentence"; tag: string; text: string; durationMs: number }
  | { type: "paused" }
  | { type: "resumed" }
  | { type: "idle" }
  | { type: "voiceUnavailable"; language: string }
  | { type: "error"; detail: string };

export interface GpuStatus {
  supported: boolean;
  gpuName: string | null;
  installed: boolean;
  sizeBytes: number;
  downloadBytes: number;
  active: boolean;
  restartRequired: boolean;
}

export interface MemoryItem {
  /** "stt" | "voice:en" | "voice:ar" | "voice:de" | "silma" | "llm:<model id>" */
  key: string;
  kind: "stt" | "voice" | "silma" | "llm";
  /** "stt" | language code | "silma" | "chat" | "other" */
  role: string;
  model: string;
  state: "loaded" | "loading" | "idle" | "missing" | "failed";
  detail: string | null;
  /** Key of the model's "load at startup" switch; empty when it has none. */
  autoloadKey: string;
  autoload: boolean;
}

export interface SilmaStatus {
  installed: boolean;
  state: "off" | "starting" | "ready" | "failed";
  device: string | null;
  error: string | null;
  sizeBytes: number;
  downloadBytes: number;
  gpu: boolean;
}

export interface SilmaProgress {
  stage: "runtime" | "packages" | "weights" | "prepare" | "done" | "error" | "cancelled";
  downloadedBytes: number;
  totalBytes: number;
  detail: string | null;
  error: string | null;
}

export interface GpuProgress {
  state: "downloading" | "extracting" | "done" | "error" | "cancelled";
  downloadedBytes: number;
  totalBytes: number;
  error: string | null;
}

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

export interface PublicSearxInstance {
  url: string;
  searchSuccess: number;
  searchTime: number | null;
  version: string | null;
}

export interface SearchTestResult {
  engine: string;
  count: number;
  firstTitle: string | null;
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
  llmIsCloud: boolean;
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
