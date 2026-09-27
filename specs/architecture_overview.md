# Architecture Overview

**Project:** Open Local Assistant (`local-ai-assistant` v1.0.0)
**License:** GPL-3.0-only
**Repository:** <https://github.com/kz370/Open-Local-Ai-Assistant>
**Companion diagram:** [`diagrams/architecture.mmd`](./diagrams/architecture.mmd) (layered view)

This document describes how the application is put together: which technologies
are used, what the boundaries between modules are, how data and control move at
runtime, and which decisions were deliberate. It complements
[`full_documentation.md`](./full_documentation.md) (per-file analysis) and the
other diagrams in [`diagrams/`](./diagrams/).

---

## Table of Contents

1. [Project type](#1-project-type)
2. [Technologies used](#2-technologies-used)
3. [Architecture pattern](#3-architecture-pattern)
4. [Module boundaries](#4-module-boundaries)
5. [Infrastructure components](#5-infrastructure-components)
6. [Service relationships](#6-service-relationships)
7. [High-level behaviour](#7-high-level-behaviour)
8. [Key architectural decisions and rationale](#8-key-architectural-decisions-and-rationale)
9. [Threading model](#9-threading-model)
10. [Known architectural drift](#10-known-architectural-drift)

---

## 1. Project type

A **local-first desktop application** for Windows built with **Tauri 2**. It is a
two-process system:

- a **Rust backend** (`src-tauri/`) that owns every side effect — HTTP calls, the
  microphone, the speaker, the filesystem, SQLite, global hotkeys, the tray;
- a **web frontend** (`src/`, React 19 + TypeScript) that is a *view* over that
  backend. It renders, collects input, and calls commands. It performs no
  privileged work of its own.

Five windows are served by one frontend bundle, selected by **hash routing**:

| Window | Tauri label | Route | Role |
| --- | --- | --- | --- |
| Chat | `main` | `#/` | Primary conversation surface, undecorated and transparent. |
| Settings | `settings` | `#/settings` | Dashboard, sections, model manager, MCP & tools. |
| Dictation overlay | `overlay` | `#/overlay` | Live word card shown while dictating into another app. |
| Launcher bubble | `bubble` | `#/bubble` | Small floating circle that opens the chat window. |
| Setup wizard | *(renders into `main`)* | `#/` when `firstRunComplete` is false | First-run hardware scan and consent. |

The application is **Windows-first**: global shortcuts, the window-activation and
text-insertion paths, the tray, and the GPU pack relaunch all use Win32 APIs. The
code is written to also build on macOS and Linux, but those are untested.

There is **no account system, no telemetry and no operator-side server**. The
only data that leaves the machine is the user's AI messages going to the AI
provider they selected, web searches (only when the assistant uses the search
tool), and model downloads (only when the user clicks *Download*).

---

## 2. Technologies used

### Runtime and build

| Layer | Technology | Version | Role |
| --- | --- | --- | --- |
| Shell | Tauri | 2 | Windowing, IPC (`invoke` / `listen` / `emit`), tray, bundling (NSIS/MSI), CSP, capabilities. |
| Backend | Rust | edition 2021, stable (MSVC on Windows) | All privileged behaviour. |
| Frontend | React / React DOM | 19.1.x | Component tree for all five surfaces. |
| Language (frontend) | TypeScript | ~6.0 | Strict typing of every IPC payload. |
| Bundler | Vite | ^8.0 | Dev server (`127.0.0.1:1420`) and production build to `dist/`. |
| Node | Node.js | 20+ | Build-time only. |
| WebView | WebView2 (Edge) | system | Hosts the frontend; `withGlobalTauri: false`, `freezePrototype: true`. |

### Backend libraries

| Concern | Crate | Version | Role |
| --- | --- | --- | --- |
| Async runtime | `tokio` | 1.53 (`features = ["full"]`) | Tauri async runtime; timers, `select!`, `spawn_blocking`. |
| Cancellation | `tokio-util` | 0.7 | `CancellationToken` for turns, tool calls, downloads. |
| Serialization | `serde`, `serde_json` | 1, 1 | `camelCase` wire format; every payload crosses IPC this way. |
| Errors | `thiserror` | 2.0 | `AppError` enum with stable `code()` strings. |
| HTTP | `reqwest` | 0.13 | LM Studio + hosted providers, web search, model/GPU downloads. |
| Streaming | `futures-util` | 0.3 | SSE byte streams (`bytes_stream`). |
| Database | `rusqlite` | 0.40 (`bundled`) | SQLite incl. FTS5: conversations, messages, settings KV, MCP config. |
| MCP | `rmcp` | 3.4 | Model Context Protocol client (stdio child process + streamable HTTP). |
| Speech runtime | `sherpa-onnx`, `sherpa-onnx-sys` | 1.13.8 (`shared`) | STT, VAD, TTS, linear resampling; the raw C API for Whisper language switching. |
| Audio I/O | `cpal` | 0.18 | Microphone capture and speaker playback. |
| Text input | `enigo` | 0.6 | Typing/pasting into the focused application (dictation). |
| Clipboard | `arboard` | 3.6 | Copy paths in the chat UI. |
| Language ID | `whatlang` | 0.18 | English/Arabic/German detection behind `language::detect`. |
| Hashing | `sha2`, `hex` | 0.11, 0.4 | SHA-256 verification of every downloaded model and GPU archive. |
| Encryption | `chacha20poly1305` | 0.10 | Encrypted settings backup export/import. |
| Archives | `tar`, `zip`, `bzip2` | 0.4, 2, 0.6 | `.tar.bz2` model and GPU-pack extraction. |
| Time | `chrono` | 0.45 | Local clock (turn notes), RFC 3339 stamps, log timestamps. |
| IDs | `uuid` | 1.26 | Conversation, message, attachment and call identifiers. |
| Hardware | `sysinfo` | 0.39 | CPU/RAM/GPU detection behind `HardwareInfo`. |
| Logging | `tracing`, `tracing-subscriber`, `tracing-appender` | 0.1, 0.3, 0.2 | Non-blocking file appender, daily rotation, 7 files kept. |
| Autostart | `auto-launch` | 0.5 | "Start with Windows" registry entry. |
| Regex / URL | `regex`, `url` | 1.13, 2.5 | Source extraction from tool output; URL normalisation. |
| Tauri plugins | `single-instance`, `global-shortcut`, `dialog`, `opener` | 2.4.4, 2.3.2, 2.7.3, 2 | One instance, hotkeys, file dialogs, "open folder". |
| Async traits | `async-trait` | 0.1 | `AiService`, `ToolProvider` object safety. |

### Frontend libraries

| Concern | Package | Version | Role |
| --- | --- | --- | --- |
| State | `zustand` | ^5.0 | `chatStore`, `settingsStore`, `voiceStore` — the three client-side state machines. |
| Tauri bridge | `@tauri-apps/api` | ^2 | `invoke`, `Channel`, `listen`. |
| Markdown | `react-markdown`, `remark-gfm` | ^10.1, ^4.0 | Assistant replies (code fences, tables, lists). |
| Icons | `lucide-react` | ^1.47 | Icon set. |
| Test | `vitest`, `@testing-library/react`, `jsdom` | ^5.0, ^16.3, ^30.1 | Store, streaming and component tests. |

### Model runtimes (external, user-installed)

| Component | Why it appears here |
| --- | --- |
| LM Studio (`http://localhost:1234/v1`) | Default chat backend. OpenAI-compatible chat plus native `/api/v1/models` discovery and load. |
| OpenRouter / Google Gemini / Hugging Face | Optional hosted providers, selected by the user; the same OpenAI-compatible client. |
| sherpa-onnx speech models | Whisper/Parakeet/SenseVoice/Moonshine (STT), Silero (VAD), Supertonic 3/Kokoro/Piper/Kitten (TTS). |
| SearXNG / DuckDuckGo | Built-in web search (no key, no extra runtime). |
| Hugging Face / GitHub releases / NVIDIA redist | Model and GPU-pack download origins, all SHA-256 pinned. |

---

## 3. Architecture pattern

### 3.1 Tauri 2 two-process model

```
┌──────────────────────────── Rust process ────────────────────────────┐
│  commands/*  ──invoke──▶  services/*  ──▶  database, settings, state  │
│         ▲                        │                                     │
│         │ emit (global)         │ spawn_blocking                     │
└─────────┼────────────────────────┼─────────────────────────────────────┘
          │                        │
   ═══════╪════════ IPC boundary (Channel + event bus) ══════════════════
          │                        │
┌─────────┼─────────────────────────────────────────────────────────────┐
│  WebView (React 19): components, pages, app/* stores                 │
└───────────────────────────────────────────────────────────────────────┘
```

The frontend is **untrusted at the architectural level**: it is a WebView running
under a restrictive CSP
(`default-src 'self'; connect-src ipc: http://ipc.localhost; script-src 'self'`),
and every side effect requires a round trip. The backend never trusts a payload
coming in — `Settings::sanitize()` clamps every field, `SendInput` is validated,
attachments are size-capped, and `McpServerConfig::validate()` rejects bad
transports.

### 3.2 Layered backend

Four layers, each depending only on the one below it:

1. **Presentation / IPC** — `src-tauri/src/commands/*`. Thin `#[tauri::command]`
   functions that validate input and delegate. No business logic, no state of
   their own beyond what the services return.
2. **Application / orchestration** — `src-tauri/src/services/chat/*`. The chat
   turn: history, prompt assembly, tool rounds, permission handshakes,
   persistence, speech hand-off.
3. **Domain services** — `src-tauri/src/services/{ai,stt,tts,audio,models,gpu,mcp,search,language,attachments,hardware}` plus
   `dictation.rs` and `capabilities/`. Each is self-contained and reachable
   through a trait or a narrow public surface.
4. **Infrastructure** — `database/`, `settings/`, `state.rs`, `errors.rs`,
   `logging.rs`, `desktop/`.

Cross-layer reach-up is what `state.rs` and the `capabilities/` module exist to
prevent: they hold the `Arc` singletons and hand each layer only the handles it
needs (`AppState::capabilities()` builds a `LocalCapabilityManager` that carries
exactly six services, not the whole state).

### 3.3 Hash-routed React SPA

There is no router library. `src/main.tsx` reads `location.hash` once and picks
a screen: `#/settings`, `#/overlay`, `#/bubble`, otherwise chat. The same bundle
is loaded in every window. This keeps the WebView count low (a WebView per
window is expensive) and makes each surface a pure function of a string.

Boot is explicit and defensive: settings load → `settings://changed`
subscription → voice subscription (chat route only) → render. A 3-second stuck
detector shows a visible message instead of a blank window, and a load failure
renders a retry screen rather than throwing into a blank page.

### 3.4 Channel-based streaming vs. global events

The app uses **two distinct IPC transports** and picks per use case:

| Transport | Used for | Why |
| --- | --- | --- |
| **`Channel<T>`** (a `tauri::ipc::Channel` passed *into* `invoke`) | `chat_send` (`Channel<ChatEvent>`), `chat_explain` (`Channel<ExplainEvent>`) | Per-invocation, ordered, and **garbage-collected with the call**. A turn that is stopped or whose window closes simply drops; there is no "who is listening to this turn?" problem. Events also carry a `turn_id`, and the store ignores any whose `turn_id` is not the current one. |
| **Global events** (`app.emit` / `listen`) | `voice://event`, `tts://event`, `dictation://state`, `dictation://history`, `settings://changed`, `models://changed`, `models://download`, `mcp://changed`, `memory://changed`, `voice://error` | Broadcast semantics: these describe **machine state** ("the mic is at level 0.4", "settings changed", "model download 40%"), and more than one window may care. |

The distinction is deliberate: **request-scoped result streams use Channels,
state broadcasts use global events.** `Channel::send` failures are ignored
(`let _ = on_event.send(ev)`) because a dropped turn is not an error.

The frontend batches stream application: `chatStore.bufferStream()` accumulates
deltas and flushes at most every `STREAM_FLUSH_MS = 50` ms
(`src/app/chatStore.ts:117`).

---

## 4. Module boundaries

### 4.1 Rust modules

| Module | Layer | Responsibility |
| --- | --- | --- |
| `main.rs` | entry | Thin binary that calls `local_ai_assistant_lib::run()`. |
| `lib.rs` | orchestration / composition root | Builds `AppState`, wires the tool and speech sinks, routes dictation events, runs Tauri plugins and the command handler table. |
| `state.rs` | infrastructure | `AppPaths` and `AppState` — the process-wide `Arc` singletons plus mutable flags. |
| `errors.rs` | infrastructure | `AppError` (16 variants), `code()`, the `{code, detail}` wire format, `From` conversions, `from_lmstudio_http`. |
| `logging.rs` | infrastructure | `tracing` file appender, daily rotation, 7 retained files; the guard is leaked for the process lifetime. |
| `commands/mod.rs` | presentation | `CmdResult<T>` alias; declares the six command modules. |
| `commands/app.rs` | presentation | Settings get/save (with all change side effects), window control, capability scan, privacy status, shortcuts, log tail, app info, export/import. |
| `commands/chat.rs` | presentation | `chat_send` / `chat_explain` (Channel commands), `chat_stop`, `chat_confirm_tool`, conversation CRUD, search, export/import. |
| `commands/voice.rs` | presentation | Audio devices, listening sessions, dictation control and history, TTS control, model catalogue/install/delete, GPU pack commands. |
| `commands/mcp.rs` | presentation | MCP server CRUD, per-tool permissions, safe mode, LM Studio import, SearXNG instance list and search test. |
| `commands/memory.rs` | presentation | "Models in memory" (`MemoryItem`), load/unload, startup preload, STT reload. |
| `commands/attachments.rs` | presentation | File/bytes/text ingestion, removal, data URL, extracted text. |
| `services/mod.rs` | domain | Declares the ten service submodules plus `dictation`. |
| `services/ai/mod.rs` | domain | `ModelInfo`, `ConnectionStatus`, `ChatMessage`/`MessageContent`/`ContentPart`, `ChatRequest`, `StreamChunk`, `ChatCompletion`, and the `AiService` trait. |
| `services/ai/lmstudio.rs` | domain | The one `AiService` implementation: chat streaming, native v1/v0 discovery with OpenAI fallback, hosted-provider parsing, load/unload. |
| `services/ai/sse.rs` | domain | `SseDecoder` (incremental, CRLF- and split-UTF-8 safe) and `ToolCallAccumulator` for streamed tool-call fragments. |
| `services/ai/model_selector.rs` | domain | `ModelSelection` and the scoring function used for automatic model choice. |
| `services/chat/mod.rs` | application | Re-exports `ChatEngine`, `ChatEvent`, `SendInput`, `ModelResolver`. |
| `services/chat/orchestrator.rs` | application | `ChatEngine::send` — the whole turn: conversation, language, model resolution, prompt, history, tool rounds with permissions, persistence, events, cancellation. Also `explain()` and `build_history()`. |
| `services/chat/prompt.rs` | application | `build_system_prompt` (byte-stable) and `turn_note` / `freshness_hint` (per-turn, never stored). |
| `services/chat/resolver.rs` | application | `ModelResolver` — 20 s discovery cache, auto vs. manual selection, lazy load. |
| `services/chat/tools.rs` | application | `ToolSpec`, `ToolOutput`, `Source`, `ToolCategory`, `Permission`, the `ToolProvider` and `SpeechSink` traits, `CombinedTools`, `NoTools`, `NoSpeech`. |
| `services/chat/think.rs` | application | `ThinkFilter` — splits `<think>` reasoning from visible content. |
| `services/chat/freshness.rs` | application | Heuristic that decides whether a message asks for current information. |
| `services/chat/attach.rs` | application | Renders stored attachments into message content parts (images only on the newest user message). |
| `services/chat/explain.rs` | application | `ExplainEvent` and the one-shot request that explains a selection out of band. |
| `services/stt/mod.rs` | domain | `SttService`: model resolution, recogniser caching, `clean_transcript`, `MIN_AUDIO_MS`. |
| `services/stt/engine.rs` | domain | sherpa-onnx recogniser wrapper, including the in-place Whisper language switch via the raw C API. |
| `services/stt/session.rs` | domain | `VoiceSessions` and `run_session`: push-to-talk, hands-free VAD endpointing, dictation, mic test; `VoiceEvent` (5 variants); playback gating. |
| `services/tts/mod.rs` | domain | `TtsService` as a `SpeechSink`: sentence queueing, per-sentence language detection, voice selection, engine cache, `TtsEvent` (7 variants). |
| `services/tts/sentence_buffer.rs` | domain | Token stream → complete sentences; skips code fences, protects abbreviations/decimals/URLs, soft-cuts run-ons. |
| `services/tts/speech_text.rs` | domain | Cleans text for speech and gates the `<laugh>`/`<sigh>`/`<breath>` sound tags. |
| `services/tts/voices.rs` | domain | `VoiceInfo`, voice enumeration from installed models, `select_voice` by language and gender. |
| `services/audio/mod.rs` | domain | `STT_SAMPLE_RATE`, `level`, `spectrum` (24 bands), `HighPassFilter` (100 Hz). |
| `services/audio/capture.rs` | domain | `Capture` on the `mic-capture` thread: downmix → resample → high-pass → level/spectrum + sample events. |
| `services/audio/playback.rs` | domain | `Player` on the `audio-playback` thread: clip queue, volume, pause, current-clip id. |
| `services/audio/devices.rs` | domain | Input/output enumeration and device lookup. |
| `services/models/mod.rs` | domain | `ModelStore`: catalogue-backed and scanned models, `Recommendation`, `IncompatibleModel`, staging and `.complete` handling. |
| `services/models/catalog.rs` | domain | The pinned catalogue: `ModelKind`, `Engine`, `RemoteFile` (URL + SHA-256 + dest + size), `CatalogModel`. |
| `services/models/download.rs` | domain | `install()` — staged download, in-loop hashing, verification, extraction, atomic rename, `DownloadProgress`. |
| `services/gpu/mod.rs` | domain | Optional CUDA pack: status, download, install, and the self-relaunch that makes Windows load its DLLs. |
| `services/hardware/mod.rs` | domain | `HardwareInfo` / `GpuInfo` detection, `best_vram`, `has_nvidia`, `inference_threads`. |
| `services/mcp/mod.rs` | domain | `McpManager`: connect/disconnect lifecycle, `available_tools`, `call_tool`, `statuses`, `internet_available`, safe mode. |
| `services/mcp/config.rs` | domain | `McpServerConfig` persistence + `validate()`, LM Studio `mcp.json` discovery and `ImportCandidate`. |
| `services/mcp/permissions.rs` | domain | `classify` (name/description/hints → category), `default_permission`, `clamp_permission`. |
| `services/mcp/sources.rs` | domain | Extracts `Source` records (URL + title) that really appear in tool output. |
| `services/search/mod.rs` | domain | `WebSearch` as a `ToolProvider`: SearXNG (local or public) with DuckDuckGo fallback, 10-minute result cache. |
| `services/language/mod.rs` | domain | Re-exports `Lang` and `detect`. |
| `services/language/detect.rs` | domain | `Lang` (closed `En`/`Ar`/`De` enum), script-based detection, `strip_noise`, `Detection { lang, confidence }`. |
| `services/attachments/mod.rs` | domain | `Attachment`, `AttachmentKind`, `AttachmentStore` (stage, `claim`, `gc`, size caps). |
| `services/attachments/base64.rs` | domain | Base64 helpers for image data URLs. |
| `services/attachments/office.rs` | domain | Plain-text extraction from office/PDF documents. |
| `services/dictation.rs` | domain | Grammar cleanup via a small model, text finalisation, `LiveTyper` (type + retract), clipboard insertion, `ReviewPending`. |
| `capabilities/mod.rs` | application | `LocalCapabilityManager` and the `CapabilityReport` shape used by the setup wizard. |
| `database/mod.rs` | infrastructure | `Db` = `Mutex<Connection>`, schema init, migrations, KV helpers, attachment GC support. |
| `database/conversations.rs` | infrastructure | `Conversation`, `Message`, `SearchHit`; CRUD plus FTS5 search. |
| `database/dictation.rs` | infrastructure | `DictationEntry` history (last 100). |
| `database/export.rs` | infrastructure | Conversation export to JSON/Markdown/Text and import. |
| `database/migrations.rs` | infrastructure | Ordered schema migrations. |
| `settings/mod.rs` | infrastructure | The whole `Settings` tree, `sanitize()`, `SettingsStore` (`RwLock<Settings>` + DB), versioned migrations. |
| `settings/backup.rs` | infrastructure | Encrypted export/import, key stripping, merge. |
| `desktop/mod.rs` | infrastructure | Declares the six desktop modules. |
| `desktop/window.rs` | infrastructure | Window creation, preset/custom placement, show/hide/minimise, animation, dictation target focus, overlay/bubble/settings windows. |
| `desktop/tray.rs` | infrastructure | Tray icon and menu. |
| `desktop/shortcuts.rs` | infrastructure | Global shortcut registration, action dispatch, the Windows modifier watcher, Esc-cancels-dictation. |
| `desktop/autostart.rs` | infrastructure | `auto-launch` sync. |
| `desktop/icon.rs` | infrastructure | Tray/taskbar icon generation tinted to the accent. |
| `desktop/verify.rs` | infrastructure | Startup self-check used by diagnostics. |
| `bin/gpu_probe.rs` | tooling | Standalone probe of the ONNX Runtime providers. |
| `bin/mcp_fixture_server.rs` | tooling | Minimal stdio MCP server used by the test suite. |

### 4.2 Frontend modules

| Module | Layer | Responsibility |
| --- | --- | --- |
| `src/main.tsx` | entry | Hash route, boot sequence, stuck/failure screens, `ErrorBoundary`. |
| `src/app/ipc.ts` | IPC | Every typed wrapper over `invoke`, plus `Channel` construction, `on()` for global events, `toAppError`, `newId`. |
| `src/app/types.ts` | IPC | TypeScript mirror of every Rust wire type. |
| `src/app/chatStore.ts` | state | Conversation state machine: optimistic user message, 50 ms stream batching, tool activity, confirmation, queueing, error handling. |
| `src/app/settingsStore.ts` | state | Settings state machine with `structuredClone` optimistic update and rollback; `applyAppearance` writes the CSS custom properties. |
| `src/app/voiceStore.ts` | state | Listening/TTS state machine: session numbering, level/bands, partial text, auto-submit, spoken-sentence timing for word highlighting. |
| `src/app/strings.ts` | presentation | UI string catalogue. |
| `src/app/soundTags.ts` | presentation | Sound-cue tag handling for display. |
| `src/app/knownLanguages.ts` | presentation | Language list for pickers. |
| `src/app/providers.ts` | presentation | Provider display names and URLs. |
| `src/app/attach.ts` | presentation | Attachment helpers (icons, sizes, labels). |
| `src/app/vision.ts` | presentation | Image-preview helpers. |
| `src/app/settingsHighlight.ts` | presentation | Jump-to-setting highlight for deep links from chat. |
| `src/pages/Chat/ChatApp.tsx` | presentation | Chat screen composition, call view toggle, history panel, scroll behaviour. |
| `src/pages/Settings/SettingsApp.tsx` | presentation | Settings dashboard shell, section nav, search index. |
| `src/pages/Settings/searchIndex.ts` | presentation | Settings search index. |
| `src/pages/Settings/sections/*.tsx` | presentation | `Basic`, `Ai`, `Voice`, `Dictation`, `WebSearch`, `Languages`, `Mcp`, `Memory`, `System`, `About`. |
| `src/pages/Overlay/Overlay.tsx` | presentation | Dictation overlay: live text, insert / retry / cancel actions, position memory. |
| `src/pages/Bubble/Bubble.tsx` | presentation | Floating launcher circle. |
| `src/pages/Setup/SetupWizard.tsx` | presentation | First-run capability scan, provider choice, model recommendations, consent gates. |
| `src/components/chat/*` | presentation | `Composer`, `MessageBubble`, `Markdown`, `ModelPicker`, `SelectionMenu`, `Sources`, `ToolActivity`, `ToolConfirmDialog`, `Attachments`. |
| `src/components/voice/*` | presentation | `CallView`, `LevelMeter`, `VoiceBars`, `SpokenText` (word-by-word highlight). |
| `src/components/settings/*` | presentation | `GpuCard`, `LanguagePicker`, `ModelManager`, `ShortcutInput`, `layout`. |
| `src/components/common/*` | presentation | `BrandMark`, `ErrorBoundary`, `controls` (buttons, toggles, fields). |
| `src/components/history/HistoryPanel.tsx` | presentation | Conversation list, rename/delete, export. |
| `src/styles/*.css` | presentation | Design tokens, base, chat, settings. |
| `src/test/*` | verification | Vitest suites: streaming store, tool UI, shortcuts, strings, history, selection, voice sessions. |

### 4.3 The rules that enforce the boundaries

1. **Commands hold no state.** Every `#[tauri::command]` is a thin adapter. If a
   command needs a decision, it calls a service; there is no business rule in
   `commands/`.
2. **Services never import `tauri::command` or the frontend types.** Domain
   modules talk to each other through Rust traits (`AiService`, `ToolProvider`,
   `SpeechSink`) and plain structs, which is what makes `ChatEngine` testable
   with `FakeAi` / `FakeTools` / `NoSpeech`.
3. **One direction only.** Domain modules do not import `commands`, and
   infrastructure (`database`, `settings`, `errors`, `logging`) does not import
   services. `state.rs` holds the handles; it is the only place that knows the
   whole set.
4. **Capabilities are layered, not per window.** A single
   `src-tauri/capabilities/default.json` grants the same permission set to
   `main`, `settings`, `overlay` and `bubble`. The backend enforces what that
   set cannot express.
5. **Traits at every I/O edge.** `AiService`, `ToolProvider`, `SpeechSink` and
   `ModelResolver` mean the orchestrator has no `reqwest`, no `rmcp` and no
   sherpa-onnx dependency, and the tests substitute fakes.
6. **The frontend may not hold domain logic.** Validation, clamping and policy
   live in Rust; `Settings::sanitize()` runs on **every** load and **every**
   save, so a hand-crafted IPC payload is sanitised exactly like a UI one.
7. **Errors are values, not strings.** `AppError` crosses the boundary as
   `{ code, detail }` with a stable `code`, and the UI maps codes to localised
   messages.
8. **Channel for turn-scoped streams, event for state.** Documented above; the
   rule is that a stream owned by one `invoke` call never uses a global event.

---

## 5. Infrastructure components

| Component | Kind | Location / implementation | Notes |
| --- | --- | --- | --- |
| Application data directory | Path | `%APPDATA%\com.localassistant.app` (`app_data_dir()`) | Created at startup; the root of everything below. |
| SQLite database | File | `<data>\assistant.db` | `conversations`, `messages` (+ FTS5), `settings` KV, `mcp_servers`, `mcp_tool_permissions`, `dictation_history`, `attachments`. |
| `Db` | In-process | `rusqlite::Connection` behind `std::sync::Mutex` | Serialised access; poison-tolerant. |
| Settings store | In-process + DB | `RwLock<Settings>` + `app_settings` KV row | Single JSON document; `sanitize()` on load and on save; schema version + migrations. |
| Settings cache | In-process | `SettingsStore.current` | `get()` clones the whole tree; cheap enough at this size. |
| Model store | Directory | `<data>\models\<model-id>\` | Catalogue models plus scanned custom folders. |
| Download staging | Directory | `<data>\models\.staging-<model-id>\` | Removed on success and on failure. |
| Completion marker | File | `<model-dir>\.complete` | JSON `{ id, installedAt }`; its presence marks a model usable. |
| Attachment store | Directory | `<data>\attachments\` | Staged composer files, claimed once per turn, garbage-collected against referenced ids. |
| GPU pack | Directory | `<data>\gpu\cuda-1.13.8\` | sherpa-onnx CUDA + ONNX Runtime CUDA + `cudart`/`cublas`/`cudnn`. |
| GPU launcher copy | File | `<data>\gpu\launcher\` | The executable is copied here and relaunched so Windows resolves the CUDA DLLs first. |
| GPU switch | File | `<data>\gpu\` | Read before the database opens, so it cannot live in settings. |
| Log directory | Directory | `app_log_dir()` — `AppPaths::logs_dir` falls back to `<data>\logs`, and the initialiser to the temp directory | `tracing-appender`, daily rotation, 7 files kept. |
| Tray / shortcuts | OS | Win32 via Tauri | Global hotkeys registered at startup and re-registered when settings change. |
| Keyring / secrets | In settings | `settings.ai.apiKey` in the local database | Never written to logs; stripped from exports unless explicitly included, and exports are ChaCha20-Poly1305 encrypted. |
| Command registry | Static | `tauri::generate_handler!` in `lib.rs` | 94 commands; `src/app/ipc.ts` is the one-to-one client. |

---

## 6. Service relationships

Edge list, `A | uses | B`:

```text
lib.rs (composition root)        | builds     | AppState
lib.rs                           | composes   | CombinedTools([McpManager, WebSearch])
lib.rs                           | wires      | ChatEngine(ai=LmStudioService, tools, speech=TtsService)
lib.rs                           | routes     | VoiceEvent -> voice://event | dictation://state | overlay
AppState                         | owns       | Db
AppState                         | owns       | SettingsStore
AppState                         | owns       | LmStudioService
AppState                         | owns       | ModelResolver
AppState                         | owns       | ChatEngine
AppState                         | owns       | AttachmentStore
AppState                         | owns       | McpManager
AppState                         | owns       | WebSearch
AppState                         | owns       | ModelStore
AppState                         | owns       | SttService
AppState                         | owns       | TtsService
AppState                         | owns       | VoiceSessions
AppState                         | owns       | HardwareInfo
AppState                         | owns       | downloads: Mutex<HashMap<String, CancellationToken>>
AppState                         | owns       | model_loading / dictation_busy / dictation_cancel / dictation_target
AppState                         | owns       | dictation_live_typer / dictation_review / shortcut_errors
AppState.capabilities()          | exposes    | LocalCapabilityManager

commands/app.rs                  | uses       | SettingsStore, LmStudioService, ModelResolver, ModelStore, SttService, TtsService, McpManager
commands/app.rs                  | drives     | desktop::{window, shortcuts, tray, autostart, icon}
commands/app.rs                  | reads      | logging (log tail)
commands/chat.rs                 | uses       | ChatEngine, Db, AttachmentStore, SettingsStore
commands/voice.rs                | uses       | VoiceSessions, SttService, TtsService, ModelStore, services::gpu
commands/mcp.rs                  | uses       | McpManager, Db, WebSearch
commands/memory.rs               | uses       | SttService, TtsService, ModelResolver, LmStudioService, services::gpu
commands/attachments.rs          | uses       | AttachmentStore

ChatEngine                       | implements | AiService (holds Arc<dyn AiService>)
ChatEngine                       | implements | ToolProvider (holds Arc<dyn ToolProvider>)
ChatEngine                       | implements | SpeechSink (holds Arc<dyn SpeechSink>)
LmStudioService                  | implements | AiService
McpManager                       | implements | ToolProvider
WebSearch                        | implements | ToolProvider
CombinedTools                    | implements | ToolProvider
NoTools / NoSpeech               | implements | ToolProvider / SpeechSink (test doubles)
TtsService                       | implements | SpeechSink

ChatEngine.send                  | uses       | ModelResolver.resolve
ChatEngine.send                  | uses       | prompt::build_system_prompt
ChatEngine.send                  | uses       | prompt::turn_note + freshness::needs_fresh_info
ChatEngine.send                  | uses       | orchestrator::build_history
ChatEngine.send                  | uses       | language::detect
ChatEngine.send                  | uses       | think::ThinkFilter
ChatEngine.send                  | uses       | Db (conversation + message persistence)
ChatEngine.send                  | uses       | AttachmentStore.claim
ChatEngine.send                  | uses       | ToolProvider.available_tools / call_tool
ChatEngine.explain               | uses       | explain::request
ChatEngine.confirm_tool          | uses       | oneshot::Sender<bool> registry

LmStudioService.chat             | uses       | reqwest bytes_stream
LmStudioService.chat             | uses       | sse::SseDecoder
LmStudioService.chat             | uses       | sse::ToolCallAccumulator
LmStudioService.discover         | uses       | /api/v1/models -> /api/v0/models -> {base}/models
LmStudioService.discover         | uses       | tokio_util::CancellationToken (chat only)
ModelResolver                    | uses       | AiService.list_models
ModelResolver                    | uses       | model_selector::select_model
ModelResolver                    | uses       | hardware::HardwareInfo
ModelResolver                    | uses       | AiService.load_model

McpManager                       | uses       | rmcp (stdio + streamable HTTP)
McpManager                       | uses       | Db (server configs, tool permissions)
McpManager                       | uses       | permissions::{classify, default_permission, clamp_permission}
McpManager                       | uses       | sources::extract
WebSearch                        | uses       | SearXNG (local or public) then DuckDuckGo HTML
WebSearch                        | uses       | SearchSettings
WebSearch                        | uses       | SettingsStore

VoiceSessions                    | uses       | SttService
VoiceSessions                    | uses       | audio::Capture
VoiceSessions                    | uses       | sherpa_onnx::VoiceActivityDetector (CPU)
VoiceSessions                    | uses       | language::detect
VoiceSessions                    | uses       | TtsService.is_speaking (playback gating probe)
Capture                          | uses       | cpal input stream
Capture                          | uses       | audio::{level, spectrum, HighPassFilter, LinearResampler}

TtsService                       | uses       | SentenceBuffer
TtsService                       | uses       | speech_text::{clean_for_speech, sound_tags}
TtsService                       | uses       | language::detect (threshold 0.5)
TtsService                       | uses       | voices::select_voice
TtsService                       | uses       | ModelStore (installed voice models)
TtsService                       | uses       | sherpa_onnx::OfflineTts
TtsService                       | uses       | services::gpu::provider_for / mark_healthy
TtsService                       | uses       | audio::Player

SttService                       | uses       | ModelStore
SttService                       | uses       | sherpa_onnx recognisers
SttService                       | uses       | services::gpu::provider_for

ModelStore                       | uses       | catalog::CATALOG / catalog::find
ModelStore                       | uses       | models::download::install
models::download::install        | uses       | sha2::Sha256 + hex
models::download::install        | uses       | tokio_util::CancellationToken
models::download::install        | uses       | extract_tar_bz2 + safe_join
services::gpu                    | uses       | models::download::extract_tar_bz2
services::gpu                    | uses       | tokio_util::CancellationToken

dictation::correct_text          | uses       | LmStudioService (small model)
dictation::insert_text           | uses       | enigo / arboard
settings::backup                 | uses       | chacha20poly1305

database::*                      | uses       | rusqlite (+ FTS5)
logging                          | uses       | tracing + tracing-appender
desktop::shortcuts               | uses       | Win32 GetAsyncKeyState (modifier watcher, Esc)
desktop::window                  | uses       | tauri WebviewWindow + monitor work areas
```

Frontend edges:

```text
main.tsx                         | uses       | settingsStore.load / subscribe, voiceStore.subscribe
ChatApp                          | uses       | chatStore, settingsStore, voiceStore, HistoryPanel
ChatApp                          | renders    | Composer, MessageBubble, ModelPicker, CallView
Composer                         | calls      | chatStore.send / stop
MessageBubble                    | reads      | voiceStore.speakingTag / paused
CallView                         | reads      | chatStore.messages, voiceStore.levels / spoken
SpokenText                       | reads      | voiceStore.spoken (word highlight timing)
SettingsApp                      | uses       | settingsStore.update, ModelManager
ModelManager                     | calls      | ipc.modelsCatalog / modelsInstalled / modelsDownload / modelsCancel
chatStore                        | calls      | ipc.chatSend (Channel), ipc.chatStop, ipc.chatConfirmTool
chatStore                        | calls      | ipc.convGet / convSetLast / attachRemove
settingsStore                    | calls      | ipc.getSettings / saveSettings
voiceStore                       | calls      | ipc.voiceStart / voiceStop / ttsStop / ttsSetPaused
voiceStore                       | calls      | chatStore.send (auto-submit, voice barge-in)
all stores                       | listens    | settings://changed, voice://event, tts://event
```

---

## 7. High-level behaviour

### 7.1 Startup sequence

1. **`main` → `run()`.** `services::gpu::activate_early()` runs *before* Tauri and
   before the single-instance guard. If the GPU pack is installed, enabled and not
   already active, the executable is copied into the pack folder and relaunched
   from there with `LOCAL_ASSISTANT_GPU_ACTIVE` set; this call returns and the
   original process exits. Otherwise it returns `false` and startup continues.
2. **Logging is initialised** first thing in `setup`, into `app_log_dir()` with a
   non-blocking appender (daily rotation, 7 files kept). The guard is
   `Box::leak`ed so the writer outlives everything.
3. **`init_state`** resolves `AppPaths` (`%APPDATA%\com.localassistant.app`),
   creates the directory, opens `assistant.db` (schema + migrations) and loads
   `SettingsStore` (which sanitises and migrates).
4. **Hardware is detected** (`sysinfo`): CPU name, physical/logical cores, total
   and available RAM, GPU list. The result is logged once.
5. **Services are constructed in dependency order**: `LmStudioService`
   (base URL, provider, API key) → `ModelResolver` → `ModelStore` (which installs
   the bundled Silero VAD model and registers the user's extra model folders) →
   `McpManager` → `TtsService` (spawns `tts-synth` and `tts-progress`) →
   `SttService` → `VoiceSessions` (with a speaking probe into TTS) →
   `WebSearch` → `CombinedTools([McpManager, WebSearch])` → `AttachmentStore`
   (garbage-collecting unreferenced files) → `ChatEngine`.
6. **Plugins register**: single-instance (a second launch focuses the existing
   window), global shortcuts, dialog, opener.
7. **Desktop integration**: autostart synced to the setting, tray icon created,
   global shortcuts registered, the main window restored to its preset position,
   the dictation overlay and the settings window are pre-created hidden (creating
   them on demand flakes on some machines), tray/taskbar icons tinted to the
   saved accent.
8. **Background work is spawned** and does not block the first paint:
   `preload_models` (speech + voices + chat model, honouring the per-model
   autoload switches) and `McpManager::connect_enabled` (one task per enabled
   server).
9. **`LA_OPEN=chat|settings[:section]`** (debug helper) may open a window.
10. **The webview loads** `index.html#/`. `main.tsx` reads the hash, calls
    `useSettings.load()`, subscribes to `settings://changed`, and (on the chat
    route) subscribes to `voice://event` / `tts://event` / `voice://error`.
11. **`app_ready`** is invoked. If `firstRunComplete` is false the setup wizard
    renders instead of the chat window. Closing the last window does not exit
    (`ExitRequested` is prevented) — the app lives in the tray.

### 7.2 A chat turn (text)

1. The composer calls `chatStore.send(text)`. The store creates a `turnId`,
   appends an optimistic user message and a pending assistant message, and calls
   `ipc.chatSend` with a `Channel<ChatEvent>`.
2. `commands::chat::chat_send` turns the channel into an `Emit` closure and calls
   `ChatEngine::send`.
3. The engine claims staged attachments, loads or creates the conversation,
   detects the language (spoken hint → text detection ≥ 0.4 confidence →
   conversation language; a forced response language wins), and persists the user
   message exactly as typed.
4. `ModelResolver::resolve` picks the model (manual or scored automatic) and
   loads it if needed. `ChatEvent::Started` is emitted with the persisted user
   message, the model id and the response language.
5. Tools are collected. The system prompt is built (byte-stable) and the per-turn
   note (local time, detected language, freshness hint) is appended **to the
   latest user message only** — the stored message is untouched.
6. History is replayed: `build_history` drops incomplete tool exchanges, trims
   to ~60 % of the context window, and re-renders attachments (images only on the
   newest user message).
7. The request goes to `LmStudioService::chat` over
   `POST {base}/chat/completions` with `stream: true` and
   `stream_options.include_usage`. `SseDecoder` turns bytes into events,
   `ToolCallAccumulator` collects tool-call fragments, and each content or
   reasoning delta is pushed to `ChatStore` (through a 50 ms batch) and, when
   speaking, to the `SpeechSink`.
8. If the model asked for tools, the engine persists an `assistant_tool_calls`
   message, executes each call (permission check → optional confirmation →
   90 s timeout), appends `tool` messages, and loops — at most 6 rounds, after
   which tools are withheld and an explicit "tool limit reached" instruction is
   appended.
9. On completion the assistant message (content, reasoning, deduplicated
   sources, tool activity, stats) is persisted and `ChatEvent::Done` is emitted.
   Any queued messages start immediately; a failure hands the queue back to the
   composer.
10. Cancellation (`Esc`, stop button, new voice utterance) cancels the token,
    the HTTP body, the tool call and the speech sink; the partial reply is
    persisted and `ChatEvent::Error` with `code: "cancelled"` is emitted.

### 7.3 A voice turn (hands-free)

1. The call view calls `voiceStore.toggleHandsFree()` → `ipc.voiceStart("handsFree")`.
2. `VoiceSessions::start` resolves the language hint, stops and joins any
   previous session, opens the microphone and spawns the `voice-session` thread.
3. `Capture` (on `mic-capture`) downmixes to mono, resamples to 16 kHz, applies a
   100 Hz high-pass filter and emits `CaptureEvent::Level` (RMS + 24-band
   spectrum, ~20×/s) and `CaptureEvent::Samples`.
4. The session thread warms the recogniser on a scoped thread while it keeps the
   meter moving, then runs either the streaming path (live partials, endpoint on
   the recogniser) or the VAD path (512-sample windows into Silero, always CPU;
   each finished utterance is decoded and emitted immediately in hands-free mode).
5. While the assistant is speaking — and for 700 ms after it stops — input is
   gated, so the assistant cannot hear itself. A muted microphone gates
   indefinitely, and a muted mic's silence does not count toward the idle timeout.
6. `VoiceEvent::Transcript` reaches the UI. `voiceStore` auto-submits it as a
   voice turn (or fills the composer when auto-submit is off) and clears the
   partial caption. The mic gate is reopened by the backend itself, through the
   `SpeakingProbe` into `TtsService::is_speaking`.
7. The turn is a normal chat turn with `voice: true`, so the reply is always
   spoken.

### 7.4 A spoken reply

1. Every visible delta is pushed to `TtsService` (`SpeechSink::push_text`).
2. `SentenceBuffer` releases a sentence as soon as it is complete: code fences
   are skipped, decimals/abbreviations/URLs do not terminate, the first sentence
   is allowed to be short (4 chars) so speech starts quickly, run-ons are
   soft-cut at 280 characters.
3. Each sentence's language is detected; a detection below 0.5 confidence falls
   back to the previous sentence's language. `voices::select_voice` then picks
   the voice from the language entry's `ttsVoice` and the preferred gender.
4. The sentence becomes a `Job` on the `tts-synth` thread, which drops jobs from
   an older generation (a stopped turn) or a cancelled tag, synthesises, and
   enqueues a `Clip` on the `audio-playback` thread.
5. The `tts-progress` thread watches the current clip id and emits
   `TtsEvent::Sentence { tag, text, durationMs }` when playback actually starts,
   plus `TtsEvent::Idle` when the queue runs dry.
6. `voiceStore` records `startedAt = performance.now()`; `SpokenText` walks the
   sentence word by word and highlights the spoken word, shifting the clock
   forward across pauses.

### 7.5 A model download (consented)

1. The settings UI shows the catalogue; the user ticks models and confirms.
2. `ipc.modelsDownload(ids)` reserves a `CancellationToken` per model **before**
   returning, so a model cannot be queued twice, and emits `models://changed`
   immediately.
3. A background task installs the models **sequentially**. For each file:
   stream the bytes into `<models>\.staging-<id>\download-<n>.part` while
   **hashing in the same loop**, emit progress at most every 250 ms, then verify
   SHA-256 against the pinned digest (mismatch → error), then either extract the
   `.tar.bz2` (top folder stripped, `..`/absolute paths rejected) or rename the
   part file into place.
4. On success: remove any previous directory, atomically rename staging →
   `<models>\<id>`, write `.complete`, emit `DownloadProgress { state: "done" }`,
   and reload STT if a model was already loaded (the automatic pick may change).
5. On failure or cancellation: delete the staging directory, emit
   `error` / `cancelled`, and remove the token. Nothing is installed unless every
   file verified.

### 7.6 A settings save

1. `settingsStore.update(mutate)` deep-copies the current tree with
   `structuredClone`, applies the mutation, sets it optimistically and calls
   `applyAppearance` so the UI reacts instantly.
2. `ipc.saveSettings(draft)` → `SettingsStore::set` → `sanitize()` (clamp every
   enum, number and language reference) → write the `app_settings` KV row →
   replace the in-memory `RwLock` value.
3. The command diffs `before`/`saved` and applies the side effects: provider,
   base URL, API key, timeout → `LmStudioService` + resolver invalidation; model
   folders → `models://changed`; speech settings → background `reload_stt`;
   `voiceHardware` → unload all voice engines; device/volume → `apply_settings`;
   preload toggle → start preloading; shortcuts → re-register; autostart →
   `auto-launch`; always-on-top / position → window; accent → icons.
4. `settings://changed` is broadcast with the sanitised settings; **every** open
   window adopts them.
5. If the save failed, the store rolls back to the previous object and re-applies
   the old appearance.

---

## 8. Key architectural decisions and rationale

### 8.1 Byte-stable system prompt, per-turn context on the latest message

`build_system_prompt` must be byte-identical from one turn to the next. LM Studio
reuses its prompt cache only for an unchanged prefix; anything that changes near
the top (the clock ticking over, a different detected language) invalidates the
cache and forces a full re-read of the conversation before the first token — with
a long chat, or with a speculative-decoding draft model, that is many seconds of
silence.

Therefore:

- the **date** (not the time) is in the system prompt, and the Arabic-quality
  block is included whenever Arabic *could* be answered, not only when it is;
- the **local time**, the **detected response language** and the
  **freshness hint** go into `turn_note`, appended to the **latest user message
  only**, in the in-memory copy. The stored message stays exactly as typed.

`services/chat/orchestrator.rs::append_to_last_user` is the single place this
happens, and a unit test asserts the system prompt is unchanged across a
seven-minute gap with a different message language.

### 8.2 Preset-locked window position

The chat window is a floating panel. `windowPosition` is one of
`bottom-right` / `bottom-left` / `center` / `custom`. A **preset is locked**: the
position is recomputed from the monitor work area on every show, resize, compact
toggle and maximise-restore, with a 16 px margin and a clamp so the window is
always fully visible. Only `custom` persists a dragged `x`/`y`, and only when that
point is still on some monitor. A user's intent ("always bottom-right") therefore
survives a resolution change, a DPI change and a maximise.

### 8.3 Per-sentence speech synthesis

A reply is spoken **sentence by sentence, while it is still being generated**.
Sentences are released the moment they are complete (minimum 16 characters, 4 for
the first, to start quickly), each is detected and spoken in the matching voice,
so a multilingual answer can switch voice mid-reply. Waiting for the whole reply
would add seconds of dead air, and speaking in one pass would force one voice for
the whole answer.

The escape hatch is `tts.speakAfterReply`: sentences are collected and only
queued when the turn finishes, so the chat model and the voice model take turns
on the GPU instead of competing.

### 8.4 Voice-activity detection always on the CPU

The Silero VAD is ~2 MB and runs on every 512-sample window (about every 32 ms).
On CUDA, each call would be almost entirely launch and synchronisation overhead
that **competes with the chat model** for the same GPU. So the provider is pinned
to `"cpu"` in `create_vad` regardless of the GPU pack. The heavy models (STT, TTS)
do use the CUDA provider when the pack is active.

### 8.5 The GPU pack relaunches the app

Windows resolves an imported DLL from the folder the executable lives in, *before*
`main` runs and before anything on `PATH`. The CUDA build of ONNX Runtime and
sherpa-onnx therefore cannot be loaded into a running process, and putting them on
`PATH` would change nothing.

So `services::gpu::activate_early` copies the executable into
`<data>\gpu\launcher\`, next to the pack's libraries, and relaunches it with
`LOCAL_ASSISTANT_GPU_ACTIVE` set. The same binary then starts with the GPU build
resolved, and the marker stops it from relaunching again. The alternative — a
custom loader or a separate launcher binary — adds a build artefact and a
diagnostics surface for no benefit.

### 8.6 Layer-gated tool permissions

Tools are classified (`search`, `fetch`, `read`, `write`, `execute`, `other`) from
the MCP name, description and the `readOnlyHint`/`destructiveHint` fields. Policy
is applied in layers, each one narrowing the previous:

1. `default_permission(category)` — search/fetch/read default to `Allow`, write
   and execute to `Ask`, unknown to `Ask`.
2. The stored per-tool choice (explicit user decision).
3. **Safe mode** — on by default at every launch, never persisted; it clamps
   `write`/`execute`/`other` back to `Ask` regardless of the stored choice.
   Turning it off requires a Windows Hello prompt.
4. The orchestrator's own gate: `Permission::Deny` is never offered, `Ask`
   suspends the turn on a `oneshot` with a 180 s timeout, and denial returns a
   model-facing refusal rather than a silent failure.

A user who later re-enables safe mode gets their previous choices back, because
stored permissions are never overwritten by the clamp.

### 8.7 No per-window capability split

`src-tauri/capabilities/default.json` is a single capability that lists
`main`, `settings`, `overlay` and `bubble` together and grants the same nine
permissions to all of them. A per-window split was rejected: it multiplies the
configuration surface for no isolation benefit, because all four windows load the
**same bundle** and therefore the same code. Any restriction that actually
matters is enforced in Rust (settings sanitisation, permission clamping, dictation
targeting), where it cannot be bypassed by anything running in the WebView.

### 8.8 `structuredClone` optimistic settings with rollback

`settingsStore.update` clones the current settings, mutates the copy, publishes
it immediately, and re-applies the appearance. Controls therefore respond at once
even though a save is an IPC round trip plus a SQLite write.

The failure path restores the **pre-update object reference** (not a re-fetch) and
re-applies the old appearance. Capturing the snapshot before mutating is what
makes the rollback exact; a `getSettings()` round trip could return a state that
another window has since changed.

### 8.9 Batched 50 ms stream flush

Stream deltas arrive per token. Applying each one re-parses the whole Markdown
message and re-lays out the message list, which pegs the WebView — and, on the
same machine, the GPU the model is running on. `chatStore` therefore buffers
content and reasoning text and flushes at most every `STREAM_FLUSH_MS = 50` ms,
and **any** non-delta event forces an immediate flush first, so `toolStarted`,
`done` and `error` can never appear before the text that preceded them. 50 ms is
below the perceptual threshold for text appearing to be instant, and caps
re-renders at ~20/s.

### 8.10 Trait seams instead of direct dependencies

`AiService`, `ToolProvider` and `SpeechSink` exist so the orchestrator depends on
capabilities, not implementations. That is what lets the tool-round, permission,
language and history tests run against `FakeAi` / `FakeTools` / `NoSpeech` with no
network and no audio device, and it is why the AI layer can grow a second backend
without touching `ChatEngine`.

---

## 9. Threading model

### 9.1 Named OS threads

Five threads are named explicitly so they are identifiable in a stack dump or
debugger:

| Thread | Created in | Responsibility | Blocking model |
| --- | --- | --- | --- |
| `mic-capture` | `services/audio/capture.rs` | Owns the cpal input stream. Downmix → resample → 100 Hz high-pass → level/spectrum (20 Hz) and sample events over an `mpsc` channel. | Real-time-ish; the stream callback is the only place audio is touched. The thread then sleeps in 20 ms steps until stopped. |
| `voice-session` | `services/stt/session.rs` | One per listening session. Drains capture events, drives the VAD/streaming recogniser, emits `VoiceEvent`. | `recv_timeout(100 ms)`; also spawns a `std::thread::scope` worker to warm the recogniser and (outside hands-free) to decode utterances off the capture loop. |
| `tts-synth` | `services/tts/mod.rs` | Consumes `Job`s, loads/synthesises with the cached `OfflineTts`, enqueues clips. | One in-flight job at a time; `min(4)` inference threads per job. |
| `tts-progress` | `services/tts/mod.rs` | Polls the player's current clip id every 40 ms and emits `TtsEvent::Sentence` / `Idle`. | Pure polling; holds only a `Weak<Self>` so it cannot keep the service alive. |
| `audio-playback` | `services/audio/playback.rs` | Owns the cpal output stream and the clip queue. | Command channel; volume/rate/paused/playing are atomics. |

Two further threads are **unnamed** but architecturally significant:

- **The Windows modifier watcher** (`desktop/shortcuts.rs::trigger_modifier_watch`)
  — a single `thread::spawn` that polls `GetAsyncKeyState` every **25 ms**. It
  exists because the global-shortcut plugin cannot register modifier-only
  combinations (e.g. `Ctrl+Alt+Space`) reliably, and because `Esc` must cancel
  dictation even while the overlay cannot take focus. It is **observe-only** — it
  never swallows a key — and the watched combinations are refreshed on every
  `register_all`, with stale actions dropped so a stuck "pressed" state cannot
  leak into a different combination.
- **Scoped helper threads** inside `voice-session` (recogniser warm-up and the
  dictation utterance decoder), joined by `std::thread::scope` so a session can
  never outlive its thread.

### 9.2 Tokio async versus `spawn_blocking`

Tauri's `tauri::async_runtime::spawn` is the async entry point; tokio's
multi-threaded runtime backs it. The split is strict:

**Async (`.await`) — I/O and orchestration:**
`ChatEngine::send` (the whole turn), `LmStudioService::chat` (streaming body),
model discovery and loading, `McpManager` connect/disconnect/`call_tool`,
download streaming, and the hardware-capability scan.

**`tokio::task::spawn_blocking` — CPU-bound or device-bound:**
loading a speech recogniser (`commands/memory.rs` "stt"), preloading a voice
(`"voice:<lang>"`), enumerating audio devices (`capabilities::scan`), typing or
pasting dictation text and retracting live-typed text (`lib.rs`). Each is a call
that can take seconds and would otherwise stall the runtime.

**Named std threads — anything with its own real-time or lifetime requirements:**
everything in §9.1. The `mic-capture` and `audio-playback` threads must exist for
the process lifetime; `voice-session` must be joinable and must be able to finish
after the UI has moved on (`stop()` never blocks — it hands the handle to
`previous` and the *next* `start()` joins it).

### 9.3 Mutex and lock policy

- **Poison-tolerant by policy.** Every `std::sync` lock in the backend is
  acquired as `lock().unwrap_or_else(|p| p.into_inner())`. A panic while a lock
  was held must not permanently brick the app: the guard is recovered and the
  data is used. This appears in `AppState`, `SettingsStore`, `LmStudioService`,
  `ChatEngine`, `TtsService`, `WebSearch`, `VoiceSessions` and `commands/*`.
- **Locks are never held across `.await`.** Every critical section is a short,
  synchronous mutation: a `HashMap` insert, a settings clone, a settings write.
  `McpManager` is the one place where this rule is structural — it uses a
  **`tokio::sync::RwLock<Inner>`** so `statuses()`/`available_tools()` can read
  while a `connect()` writes, and it explicitly `drop`s the write guard before
  invoking the `on_change` callback (which emits a Tauri event).
- **`Db` uses a `std::sync::Mutex<Connection>`, not a pool.** SQLite access is
  short and serialised; `rusqlite::Connection` is not `Sync`, so one connection
  behind one mutex keeps the type simple and the critical sections tiny.
- **Cheap snapshots over long borrows.** `SettingsStore::get()` clones the whole
  tree and hands the clone out; services then read from their own snapshot for the
  duration of a turn. The same pattern appears in `HardwareInfo` (cloned into
  every service that needs it) and in `ModelResolver`'s `Vec<ModelInfo>` cache.
- **Atomics for flags that cross threads without a lock**: `AtomicBool`
  (`dictation_busy`, `dictation_cancel`, `muted`, `GPU_ACTIVE`), `AtomicU64`
  (`tts.generation`, `tts-progress`'s last clip id), `AtomicIsize`
  (`dictation_target` native handle), `AtomicU32` (playback volume/rate).
  `Relaxed` ordering is sufficient because these are coordination flags, not
  shared data.
- **`CancellationToken` replaces "stop" flags.** A turn, a tool call, a model
  download and the GPU pack each own a `CancellationToken`; cancelling is a
  single atomic operation that every `tokio::select!` in the call stack observes.
  `TtsService` uses a monotonic `generation` counter for the same purpose,
  because queued synthesis jobs need to know that they belong to a *previous*
  turn.

---

## 10. Known architectural drift

These are real, currently-shipping divergences between what the documentation and
the code say and what the code does. They are recorded here so a reviewer does not
read them as new regressions.

1. **`VoiceCapability.acceleration` is hardcoded to `"cpu"` while
   `commands/memory.rs` computes the real device.**
   `capabilities/mod.rs::detect_voice` returns `acceleration: "cpu".into()` with
   the comment "This build runs voice models on the CPU." But
   `commands::memory::memory_status` derives the same idea correctly:
   `if crate::services::gpu::is_active() { "GPU" } else { "CPU" }`, and that value
   *is* used, so the "Models in memory" list can say `GPU` while the setup
   wizard's capability report for the same machine says `cpu`. The correct
   expression is `services::gpu::provider()` (or `provider_for("auto")`).

2. **`AGENTS.md` §3 mandates `Diagnostic { problem, cause, fix }`, but the wire
   format is `{ code, detail }`.** `errors.rs` serialises `AppError` as
   `WireError { code, detail }` and `src/app/types.ts` mirrors it as
   `AppErrorPayload { code, detail }`. The project-level instruction has not been
   updated to the shape that actually shipped, and the README's "says what went
   wrong, why, and how to fix it" claim is satisfied by the UI string catalogue
   (which maps `code` to a human message) rather than by the error payload. Either
   the mandate or the payload should change; today they disagree.

3. **`privacy_status` under-reports private LAN addresses.**
   `commands/app.rs::privacy_status` classifies the configured base URL as local
   when the host is `localhost` / `127.0.0.1` / `::1` / `[::1]`, or starts with
   `192.168.` or `10.`, or ends with `.local`. The RFC 1918 range
   **172.16.0.0/12** (`172.16.` – `172.31.`) is missing, so a user pointing the
   provider at a private 172.x host is shown a "cloud / data leaves this machine"
   notice for an address that never leaves their LAN. Link-local
   (`169.254.0.0/16`) and IPv6 unique-local (`fc00::/7`) hosts are unhandled too.

4. **CI installs Inno Setup 6 while `build-installer.bat` looks for 7.**
   `.github/workflows/build.yml` checks for
   `${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe` before falling back to
   `choco install innosetup`, whereas `build-installer.bat` searches only
   `Inno Setup 7\ISCC.exe` in four locations before falling back to `where iscc`.
   The README also says "Inno Setup 6". A CI build on a machine with only
   Inno Setup 6 therefore skips the setup file with a warning instead of failing
   loudly, and the versions in three places disagree.

5. **The `tts` legacy voice fields are on the wire but not in the TypeScript
   type.** `TtsSettings` serialises `voiceEn`, `voiceAr` and `voiceDe`
   (`#[serde(rename = "voiceEn")] legacy_voice_en`, …) so that a pre-v4 settings
   blob still deserialises; the v4 migration copies them into
   `language.entries[].ttsVoice` and they are never read again. But
   `src/app/types.ts` `Settings["tts"]` does not declare them, so the two type
   declarations of the same payload disagree. The fields are inert at runtime —
   `serde`'s container-level `default` fills them in and the migration ignores
   them once `version >= 4` — but the mismatch makes the wire shape
   undiscoverable from the frontend, and the fields are one careless cleanup away
   from being read again by code that no longer has a migration.

6. **`upload-release.bat` runs `git add -A`.** The release script stages the whole
   working tree before committing with the message in `commit-message.txt`. On a
   maintainer machine this can commit unrelated in-progress work, stray build
   output or anything else present in the working tree, and it is the only
   mutation path in the repository that bypasses per-file staging discipline.
   `build-installer.bat upload` and `nupload` are safe; only this script is not.

Additionally worth recording, though not a documentation/code contradiction:

7. **`specs/architecture_overview.md` and `specs/full_documentation.md` count
   settings fields differently** (e.g. `GeneralSettings` has 21 fields, not 22;
   `SttSettings` has 13, not 12; `TtsSettings` has 12, of which 3 are legacy).
   The field lists in this document were taken from the current source and are the
   ones to trust when they disagree with a prose count elsewhere.
