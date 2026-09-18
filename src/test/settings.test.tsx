import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../app/settingsStore";
import { SettingsApp } from "../pages/Settings/SettingsApp";
import { Bubble } from "../pages/Bubble/Bubble";
import { ModelPicker } from "../components/chat/ModelPicker";
import type { Settings } from "../app/types";

const SETTINGS: Settings = {
  general: {
    theme: "system",
    accent: "teal",
    assistantName: "Local Assistant",
    highContrast: false,
    fontScale: 1,
    startWithOs: false,
    startMinimized: false,
    preloadModels: true,
    alwaysOnTop: true,
    windowPosition: "bottom-right",
    window: { x: 0, y: 0, width: 420, height: 640 },
    compact: false,
    globalShortcut: "CommandOrControl+Space",
    pushToTalkShortcut: "CommandOrControl+Shift+Space",
    developerMode: false,
    firstRunComplete: true,
    logConversationContent: false,
    bubbleX: null,
    bubbleY: null,
  },
  ai: {
    serverUrl: "http://localhost:1234/v1",
    apiKey: null,
    modelMode: "auto",
    model: null,
    temperature: 0.7,
    contextLength: null,
    maxTokens: null,
    systemPrompt: "",
    streaming: true,
    requestTimeoutSecs: 300,
    showReasoning: false,
    modelAliases: { "qwen/qwen3.5-9b": "Qwen 9B" },
    pasteAsFileChars: 2000,
  },
  language: { responseLanguage: "auto" },
  stt: { model: "auto", language: "auto", microphone: null, micOnly: true, isolateSystemAudio: false, hardware: "auto", autoSubmit: true, handsFree: false, pushToTalk: true, vadThreshold: 0.5, silenceMs: 800, extraModelDirs: [], callView: true, autoStopSilenceSecs: 8, handsFreeTimeoutSecs: 300 },
  tts: { speakResponses: false, voiceEn: "auto", voiceAr: "auto", voiceDe: "auto", speed: 1, volume: 1, outputDevice: null, preferredGender: "any" },
  dictation: { enabled: true, shortcut: "CommandOrControl+Alt+Space", mode: "hold", correctionEnabled: false, correctionModel: null, insertMethod: "type", addTrailingSpace: true },
  search: { enabled: true, maxResults: 5, searxngUrl: "" },
  lastConversationId: null,
  version: 2,
};

const MODELS = [
  { id: "qwen/qwen3.5-9b", displayName: "Qwen3.5 9B", kind: "llm", sizeBytes: 6_500_000_000, params: "9B", quantization: "Q4_K_M", bitsPerWeight: 4, maxContextLength: 262144, loaded: true, loadedContextLength: 32768, toolUse: true, vision: false, reasoning: true },
  { id: "google/gemma-3-12b", displayName: "Gemma 3 12B", kind: "llm", sizeBytes: 8_000_000_000, params: "12B", quantization: "Q4_K_M", bitsPerWeight: 4, maxContextLength: 131072, loaded: false, loadedContextLength: null, toolUse: false, vision: false, reasoning: false },
];

function mockBackend() {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "get_settings":
        return SETTINGS;
      case "save_settings":
        return SETTINGS;
      case "lmstudio_models":
        return MODELS;
      case "lmstudio_auto_selection":
        return { modelId: "qwen/qwen3.5-9b", needsLoad: false, score: 115, reasons: ["already_loaded", "fits_gpu", "tool_use"] };
      case "lmstudio_test":
        return { connected: true, serverUrl: "http://localhost:1234/v1", modelCount: 2, latencyMs: 8, api: "native-v1" };
      case "audio_devices":
        return { inputs: [{ id: "mic1", name: "Microphone", isDefault: true }], outputs: [{ id: "spk", name: "Speakers", isDefault: true }] };
      case "models_catalog":
        return [{ id: "whisper-small", kind: "stt", engine: "whisper", name: "Whisper small (int8)", languages: ["*"], quality: 3, minRamGb: 4, license: "MIT", downloadBytes: 375_000_000, installed: false, recommended: true, downloading: false }];
      case "models_incompatible":
        return [{ name: "Qwen/Qwen3-TTS-12Hz-0.6B-Base", path: "C:\Users\me\.cache\huggingface\hub\models--Qwen--Qwen3-TTS", reason: "PyTorch weights: this app needs an ONNX export (sherpa-onnx format)" }];
      case "models_installed":
        return [
          { id: "whisper-base", name: "Whisper base (int8)", kind: "stt", engine: "whisper", languages: ["*"], path: "C:\models\whisper-base", source: "catalog", family: "Whisper", streaming: false, gender: "", sizeBytes: 160_000_000 },
          { id: "nemotron-3.5-asr-streaming-0.6b", name: "nemotron-3.5-asr-streaming-0.6b", kind: "stt", engine: "whisper", languages: ["*"], path: "H:\Models\openwhispr\parakeet-models\nemotron-3.5-asr-streaming-0.6b", source: "custom", family: "Streaming transducer", streaming: true, gender: "", sizeBytes: 682_000_000 },
        ];
      case "tts_voices":
        return [{ id: "kokoro-en-v0_19:1", modelId: "kokoro-en-v0_19", name: "Bella (US English, female)", language: "en", speakerId: 1, engine: "kokoro", quality: 6, gender: "female" }];
      case "mcp_list":
        return [];
      case "privacy_status":
        return { llm: "LM Studio", llmServer: "http://localhost:1234/v1", llmIsLocalAddress: true, sttLocal: true, ttsLocal: true, conversationsLocal: true, settingsLocal: true, telemetry: false, internetViaMcp: false, internetServers: [] };
      case "scan_capabilities":
        return {
          lmStudio: { connected: true, serverUrl: "http://localhost:1234/v1", api: "native-v1", modelCount: 2, errorCode: null, errorDetail: null, selection: { modelId: "qwen/qwen3.5-9b", needsLoad: false, score: 115, reasons: [] }, loadedModels: ["qwen/qwen3.5-9b"] },
          hardware: { os: "Windows 11", cpuName: "Xeon", physicalCores: 6, logicalCores: 12, totalRamBytes: 96e9, availableRamBytes: 60e9, gpus: [{ name: "RTX 3060", vendor: "nvidia", vramBytes: 12e9 }] },
          voice: { sttModel: "whisper-small", vadReady: true, ttsEn: "Bella", ttsAr: null, ttsDe: null, installed: [], recommendation: { stt: "whisper-turbo", vad: "silero-vad", ttsEn: "kokoro-en-v0_19", ttsDe: "piper-de_DE-thorsten-high", ttsAr: "piper-ar_JO-kareem-medium" }, acceleration: "cpu" },
          microphones: [{ id: "mic1", name: "Microphone", isDefault: true }],
          audioOutputs: [{ id: "spk", name: "Speakers", isDefault: true }],
          mcp: { enabled: 0, connected: 0, internet: false },
        };
      case "app_info":
        return { version: "0.1.0", paths: { dataDir: "d", modelsDir: "m", logsDir: "l", database: "db" }, hardware: { os: "Windows", cpuName: "Xeon", physicalCores: 6, logicalCores: 12, totalRamBytes: 96e9, availableRamBytes: 60e9, gpus: [] }, os: "windows" };
      case "read_log_tail":
        return "log line";
      case "shortcut_errors":
        return [];
      default:
        return undefined;
    }
  });
}

const SECTIONS = ["general", "ai", "speech", "voice", "language", "dictation", "mcp", "privacy", "appearance", "shortcuts", "diagnostics"];

describe("settings dashboard", () => {
  beforeEach(async () => {
    vi.mocked(invoke).mockReset();
    mockBackend();
    await useSettings.getState().load();
  });

  it.each(SECTIONS)("renders the %s section without crashing", async (section) => {
    location.hash = `#/settings/${section}`;
    const { container } = render(<SettingsApp />);
    // The section heading and the navigation must be present.
    await waitFor(() => expect(container.querySelector(".settings-main h1")).toBeTruthy());
    expect(container.querySelector(".settings-nav")).toBeTruthy();
    expect(container.querySelector(".settings-main")?.textContent?.length ?? 0).toBeGreaterThan(20);
  });

  it("lists where speech models are loaded from, including custom folders", async () => {
    location.hash = "#/settings/speech";
    render(<SettingsApp />);
    expect(await screen.findByText(/H:\Models\openwhispr/)).toBeInTheDocument();
    expect(screen.getByText("Streaming transducer")).toBeInTheDocument();
    expect(screen.getAllByText("Live text").length).toBeGreaterThan(0);
    // Models that cannot run are listed with the reason instead of being hidden.
    expect(screen.getByText("Qwen/Qwen3-TTS-12Hz-0.6B-Base")).toBeInTheDocument();
    expect(screen.getByText(/needs an ONNX export/)).toBeInTheDocument();
  });

  it("shows model short names and lets you set one", async () => {
    location.hash = "#/settings/ai";
    const { container } = render(<SettingsApp />);
    const alias = await screen.findByLabelText("Short name: google/gemma-3-12b");
    expect(alias).toHaveValue("");
    expect(screen.getByLabelText("Short name: qwen/qwen3.5-9b")).toHaveValue("Qwen 9B");
    // The alias replaces the full id in the model list.
    const names = Array.from(container.querySelectorAll(".model-name")).map((n) => n.textContent ?? "");
    expect(names.some((n) => n.startsWith("Qwen 9B"))).toBe(true);
    expect(names.some((n) => n.startsWith("gemma-3-12b"))).toBe(true);
  });
});

describe("model picker", () => {
  beforeEach(async () => {
    vi.mocked(invoke).mockReset();
    mockBackend();
    await useSettings.getState().load();
  });

  it("uses the alias in the composer chip and lists models", async () => {
    const { getByRole } = render(<ModelPicker autoModel="qwen/qwen3.5-9b" />);
    const button = getByRole("button", { name: /Choose model|qwen/i });
    expect(button.textContent).toContain("Qwen 9B");
    button.click();
    await waitFor(() => expect(screen.getByRole("menu")).toBeInTheDocument());
    expect(screen.getByText("gemma-3-12b")).toBeInTheDocument();
  });
});

describe("bubble", () => {
  it("opens the chat when clicked without dragging", async () => {
    vi.mocked(invoke).mockReset();
    mockBackend();
    const { getByRole } = render(<Bubble />);
    const bubble = getByRole("button", { name: /Open Local Assistant/i });
    fireEvent.pointerDown(bubble, { button: 0, screenX: 10, screenY: 10 });
    fireEvent.pointerUp(bubble, { button: 0, screenX: 10, screenY: 10 });
    await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("bubble_open_chat"));
  });
});
