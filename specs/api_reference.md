# API REFERENCE

**Project:** Open Local Assistant (`local-ai-assistant` v1.0.0)
**Transport:** Tauri 2 IPC
**Scope:** the complete contract between the React frontend (`src/`) and the Rust
backend (`src-tauri/src/`) — 93 commands, 18 global events, 2 streaming channels,
16 error codes.

> Companion documents: [`full_documentation.md`](./full_documentation.md) (per-file
> analysis) and [`developer_guide.md`](./developer_guide.md) (setup, extension, style
> and contribution rules).

---

## Table of Contents

1. [Preface](#1-preface)
2. [Errors reference](#2-errors-reference)
3. [Commands](#3-commands)
   - [3.1 Settings &amp; app (14)](#31-settings--app-14)
   - [3.2 LM Studio (3)](#32-lm-studio-3)
   - [3.3 Window (7)](#33-window-7)
   - [3.4 Chat (4)](#34-chat-4)
   - [3.5 Attachments (6)](#35-attachments-6)
   - [3.6 Conversations (9)](#36-conversations-9)
   - [3.7 Voice (23)](#37-voice-23)
   - [3.8 Models (6)](#38-models-6)
   - [3.9 GPU pack (5)](#39-gpu-pack-5)
   - [3.10 Models in memory (3)](#310-models-in-memory-3)
   - [3.11 MCP &amp; web search (13)](#311-mcp--web-search-13)
4. [Events](#4-events)
5. [Channels](#5-channels)
6. [TypeScript client](#6-typescript-client)
7. [Error-handling conventions](#7-error-handling-conventions)
8. [Security](#8-security)

---

## 1. Preface

### 1.1 Transport

Everything crosses a single boundary. The Rust side of it is
`src-tauri/src/commands/` — the only Tauri-reachable Rust module tree — and the
frontend side of it is `src/app/ipc.ts` — the only frontend file that names a Rust
command. There is no HTTP server, no socket and no shared global object.

Three IPC mechanisms are in use:

| Mechanism | Tauri's name | Used for |
| --- | --- | --- |
| **Request / response** | `invoke(cmd, args)` ⇄ `#[tauri::command]` | All 93 commands. The command resolves with the return value, or rejects with an `AppError` serialised as `{code, detail}`. |
| **Channel (bidirectional push, per invocation)** | `Channel<T>` from `@tauri-apps/api/core` | Exactly two commands: `chat_send` and `chat_explain`. A `Channel` belongs to **one** invocation; it is created by the caller and handed to Rust as an argument, so events cannot leak between turns or windows. |
| **Global event (broadcast)** | `app.emit(name, payload)` ⇄ `listen(name, cb)` | 18 names. Any window may listen; `emit_to(label, …)` targets one window (used for `app://*` and for the overlay's dictation telemetry). |

Tauri serialises with **serde JSON**. Enums that are internally tagged use
`#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]`, so
`ChatEvent::ToolFinished { duration_ms }` becomes
`{ "type": "toolFinished", "durationMs": … }`.

### 1.2 The error shape

A failing command rejects the `invoke` promise with a plain object — **not** an `Error`
instance:

```jsonc
{ "code": "lmstudio_unavailable", "detail": "LM Studio is unavailable: error sending request for url (http://localhost:1234/v1/models)" }
```

`code` is a **stable machine key** used for i18n lookup in
`src/app/strings.en.json`'s `errors.*` family. `detail` is the human-readable
`Display` string of the `AppError` variant; it is logged locally and surfaced only in
Diagnostics / Developer Mode.

`src/app/ipc.ts` provides the two helpers that make this ergonomic:

```ts
export function isAppError(e: unknown): e is AppErrorPayload   // structural: "code" in e && "detail" in e
export function toAppError(e: unknown): AppErrorPayload        // synthesises { code: "other", … } for anything else
```

`toAppError`'s fallback code `"other"` is a real key in `strings.en.json`, so an
unrecognised rejection always renders.

### 1.3 Argument conventions

| Rule | Detail |
| --- | --- |
| **Command names are the Rust `fn` names verbatim** | snake_case: `get_settings`, `window_set_compact`, `open_settings_window`, `quit_app`, `app_ready`, `bubble_open_chat`. Not renamed to camelCase. |
| **Arguments are a single object**, never positional. | `invoke("conv_rename", { id, title })`. |
| **Argument keys are camelCase; the Rust parameters are snake_case.** | `invoke("chat_stop", { turnId })` → `pub fn chat_stop(state: …, turn_id: String)`. The conversion is automatic in Tauri 2. This applies to every parameter, including nested payloads. |
| **Struct payloads carry `#[serde(rename_all = "camelCase")]`** | `Settings`, `AppInfo`, `PrivacyStatus`, `MemoryItem`, `VoiceInfo`, `CatalogEntry`, `McpServerConfig`, `AudioDevices`, `TtsState`, `SendInput`, `CapabilityReport`, `HardwareInfo`, `AppPaths`, `Attachment`, `AttachResult`, `Message`, `Conversation`, `SearchHit`, `InstalledModel`, `IncompatibleModel`, `DictationEntry`, `DownloadProgress`, `GpuProgress`, `GpuStatus`, `ServerStatus`, `ToolView`, `ImportCandidate`, `SearchTestResult`, `PublicInstance`, and every variant field of `ChatEvent` / `ExplainEvent` / `VoiceEvent` / `TtsEvent`. |
| **Optional means `Option<T>` in Rust ⇄ `T \| null` or omitted in TS.** | `readLogTail()` simply omits `lines`; Rust's `Option<usize>` becomes `None` and the default 200 applies. A TS wrapper that always sends `undefined` is equally fine. |
| **Vectors arrive as JSON arrays**; `Option<[f64; 4]>` as a 4-element array. | `dictation_overlay_menu({ menu: [x, y, w, h] })`; `null` hides the menu. |
| **Nullable values are `null`, never `""`.** | `apiKey: string \| null`; `conv_set_last({ id: null })` clears the pointer. |

### 1.4 Streaming: channels vs global events

This distinction matters when you add a new stream.

| | **Channel** (`chat_send`, `chat_explain`) | **Global event** (`voice://event`, `tts://event`, …) |
| --- | --- | --- |
| Lifetime | Exactly one invocation. Closed when the promise settles. | The whole process. |
| Addressing | Only the caller that created it. | Every listening window (or one named window, for `emit_to`). |
| Ordering | Delivered in order relative to that call. | Per-listener ordering is not guaranteed across emitters. |
| Cancellation | `chat_stop(turnId)` / `chat_stop(id)` addresses the stream by the id **inside** the events. | A separate `*_stop` / `*_cancel` command. |
| Use when | The events belong to one request (a chat turn, an explain). | The events are a state machine other windows must observe (voice, TTS, downloads, settings). |

**Only two commands stream over a Channel.** Every other progress or state signal is a
global event. `chat_send` and `chat_explain` are also the only two commands that
**never fail**: the result only signals completion, and errors are delivered as a
terminal `error` event, so a transport failure and a model failure render identically.

---

## 2. Errors reference

All 16 codes from `AppError::code()` in `src-tauri/src/errors.rs`. The wire payload is
`{code, detail}`; `detail` is the variant's `Display` string.

| `code` | Rust variant | Display | Meaning | Typical user-facing remedy |
| --- | --- | --- | --- | --- |
| `lmstudio_unavailable` | `LmStudioUnavailable(String)` | `LM Studio is unavailable: {0}` | The AI endpoint could not be reached at all (connection refused, DNS, bad host, or a bad API key rejected at connect time). | *"Unable to connect to {provider}. If this is LM Studio, make sure its local server is running (Developer tab → Start Server); for a hosted provider, check your API key and network connection."* |
| `lmstudio_error` | `LmStudio(String)` | `LM Studio error: {0}` | The endpoint answered, but with an error — HTTP 4xx/5xx, a malformed response, or a correction model that returned nothing usable. | *"The provider rejected the request. Try again, pick another model, or check the provider's status page."* |
| `no_model` | `NoModel` | `No suitable model is available in LM Studio` | Nothing could be resolved: no model loaded and none fits, or a hosted provider returned an empty catalogue. | *"Download a model in LM Studio, or pick one manually in Settings → Loaded models."* |
| `timeout` | `Timeout(String)` | `Request timed out: {0}` | The request exceeded `ai.requestTimeoutSecs` (default 120 s), or a per-operation timeout elapsed. | *"The assistant took too long to answer. Raise the timeout in Settings → AI provider, or use a smaller model."* |
| `database` | `Database(String)` | `Database error: {0}` | A `rusqlite` failure — corrupted file, locked database, constraint violation. | *"Local storage could not be read or written. Check free disk space; if it persists, remove `%APPDATA%\com.localassistant.app\assistant.db` to start fresh (conversations are lost)."* |
| `stt_unavailable` | `Stt(String)` | `Local speech recognition unavailable: {0}` | No usable speech model, the recogniser could not be built, or the voice-activity detector failed. Also the code used for a panic in the recogniser thread. | *"Install a speech model in Settings → Voice input, or use a model folder that contains a sherpa-onnx model."* |
| `tts_unavailable` | `Tts(String)` | `Local text-to-speech unavailable: {0}` | No output device, no voice installed for a language, or a TTS model folder is missing a required file. | *"No voice is installed for this language. Download the multilingual voice model in Settings → Assistant voice."* |
| `audio` | `Audio(String)` | `Audio device error: {0}` | Microphone or speaker failure: unsupported sample format, device disappeared, capture/playback start timed out. | *"The selected audio device is unavailable. Pick another microphone or speaker in Settings → Voice."* |
| `mcp` | `Mcp(String)` | `MCP error: {0}` | An MCP server failed to start, was disconnected, or a tool call failed at the protocol level. | *"The MCP server could not be started. Check its command, arguments and environment variables."* |
| `permission_denied` | `PermissionDenied(String)` | `Permission denied: {0}` | An operation needs OS consent that was refused or is unavailable. Emitted by `desktop/verify.rs` when Windows Hello is not set up, and when verification fails. | *"Windows Hello is not set up on this device. Set up a PIN in Windows Settings › Accounts › Sign-in options."* |
| `download` | `Download(String)` | `Model download failed: {0}` | A download failed, returned a non-success status, or the **SHA-256 digest did not match** (`checksum mismatch for {url}`). | *"The download could not be completed, or the file did not match its expected checksum. Check your connection and try again."* |
| `not_found` | `NotFound(String)` | `Not found: {0}` | A referenced entity does not exist: an unknown attachment id, LM Studio's `mcp.json` missing. | *"That item no longer exists. Refresh the list."* |
| `invalid` | `Invalid(String)` | `Invalid input: {0}` | A guard rejected the call: empty `ids` for `conv_export`, an unknown memory key, an unknown folder name, speech recognition in use during `memory_unload("stt")`, a deletion of a protected model, no NVIDIA GPU for `gpu_install`, an import file over 200 MiB. | Varies; the `detail` names the specific guard. |
| `io` | `Io(String)` | `I/O error: {0}` | A filesystem operation failed: settings export/import write, log tail read, folder open, conversation export write. | *"The file could not be read or written. Check the path and your permissions."* |
| `cancelled` | `Cancelled` | `Cancelled` | The user backed out. Emitted by the Windows Hello prompt when the user dismisses it, by a cancelled download, and by a chat turn the user stopped. | **No banner is shown.** `chatStore` treats `cancelled` as "the user pressed Stop"; `voiceStore` clears the state silently. |
| `other` | `Other(String)` | `{0}` | The catch-all. Also `tauri::Error`, a `spawn_blocking` join failure, and the fallback synthesised by `toAppError`. | *"Something went wrong. Open Settings → Diagnostics for the technical details."* |

### 2.1 `From` conversions

| Source | Becomes |
| --- | --- |
| `rusqlite::Error` | `Database` |
| `std::io::Error` | `Io` |
| `serde_json::Error` | `Invalid` |
| `tauri::Error` | `Other` |
| `reqwest::Error` (via the free function `from_lmstudio_http`) | `is_timeout()` → `Timeout`; `is_connect() \|\| is_request()` → `LmStudioUnavailable`; otherwise → `LmStudio` |

There is deliberately **no** blanket `From<reqwest::Error>`, so the classification must
be requested explicitly by calling `from_lmstudio_http`.

### 2.2 Note on the wire format

`AGENTS.md` §3 mandates `Diagnostic { problem, cause, fix }`-shaped user-facing
messages, but the IPC wire format is `{code, detail}`. `detail` should still read as a
diagnosis, and the user-visible text comes from the `errors.<code>` key in
`strings.en.json`. This is a documented spec/implementation divergence, not an
oversight.

---

## 3. Commands

All 93 commands, grouped **exactly** as in `src/app/ipc.ts`, which is the authoritative
grouping. Every command appears in `tauri::generate_handler![ … ]` in `lib.rs`, 1:1.

Conventions used in the tables:

- **Command** — the string passed to `invoke`. This is the snake_case Rust `fn` name.
- **Parameters** — the object keys as they appear on the wire (camelCase). `?` marks an
  optional argument; "returns" states the resolved TypeScript type.
- **async** — the Rust function is `async fn`, so it runs on the Tokio runtime and can
  await. **sync** commands run on Tauri's blocking pool, which is why fs-heavy and
  native work is deliberately sync.
- **Stream** — whether the command streams over a `Channel`.
- **Events** — global events the command may emit as a direct result. Indirect events
  (e.g. `settings://changed` from `window.rs`) are noted inline.

---

### 3.1 Settings & app (14)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 1 | `get_settings` | — | `Settings` | sync. Returns the whole persisted document. |
| 2 | `save_settings` | `settings: Settings` | `Settings` | **async.** Sanitises server-side, persists, then fires only the side effects whose values actually changed (provider/base URL/API key → `resolver.invalidate()`; extra model dirs → `models://changed`; STT model/silence/hardware → `reload_stt`; voice hardware → unload all voices; output device/volume → `tts.apply_settings`; preload on → warm-up; any shortcut → re-register; start-with-OS → registry; always-on-top; window position; accent → re-tint the tray/taskbar icon). Returns the **authoritative** sanitised document. Emits `settings://changed`. |
| 3 | `shortcut_errors` | — | `string[]` | sync. Global-shortcut registration failures collected at startup and on every re-register. **Never fatal** — the app starts with a taken hotkey. |
| 4 | `shortcuts_capture` | `capturing: boolean` | `void` | sync. `true` unregisters every global shortcut so the recorder's keypresses are not swallowed; `false` re-registers. |
| 5 | `app_info` | — | `AppInfo` | sync. `{ version, paths: { dataDir, modelsDir, logsDir, database }, hardware, os }`. |
| 6 | `open_folder` | `which: "logs" \| "models" \| "data"` | `void` | sync. Strict allow-list; anything else is `invalid: "unknown folder"`. Creates the directory if missing, then opens it with the platform opener (`explorer` / `open` / `xdg-open`). |
| 7 | `read_log_tail` | `lines?: number` | `string` | sync. Last lines of **today's** log. Default **200**, hard cap **2000**. Only `*.log` files are considered; the newest is chosen by lexicographic sort. Empty string when there is no log yet. |
| 8 | `scan_capabilities` | — | `CapabilityReport` | **async.** One snapshot of LM Studio, hardware, voice, microphones, outputs and MCP. **Never fails** because LM Studio is down — the failure is encoded in `lmStudio.errorCode` / `errorDetail`. Drives the setup wizard. |
| 9 | `privacy_status` | — | `PrivacyStatus` | **async.** `{ llm, llmServer, llmIsLocalAddress, llmIsCloud, sttLocal, ttsLocal, conversationsLocal, settingsLocal, telemetry, internetViaMcp, internetServers }`. `llmIsCloud = provider !== "lmstudio"`. Local-address rule: `localhost` / `127.0.0.1` / `::1` / `[::1]`, a `192.168.` or `10.` prefix, or a `.local` suffix. |
| 10 | `app_ready` | — | `void` | sync. **The hold-release signal.** The main window is created hidden to avoid a white flash; the webview calls this once React has mounted. First run → show the main window; otherwise show the **bubble** unless `startMinimized`. Emits nothing. |
| 11 | `complete_first_run` | — | `void` | sync. Sets `general.firstRunComplete`, registers the global shortcuts, shows the main window. Emits `settings://changed`. |
| 12 | `quit_app` | — | `void` | sync. Delegates to `tray::quit`: stop the chat, discard the voice take, stop TTS, hide **all four** windows, bound `mcp.shutdown()` to 3 s, then `app.exit(0)`. The only clean-exit path (closing the window hides to tray). |
| 13 | `settings_export` | `path: string`, `includeKeys: boolean` | `void` | sync. Writes an **encrypted** (ChaCha20-Poly1305) settings backup. `includeKeys: false` calls `backup::strip_keys` first, so API keys are excluded by default. The path is supplied by the UI's save dialog and is **not** validated further. |
| 14 | `settings_import` | `path: string` | `Settings` | sync. Decrypts a backup and returns `merge_import(current, imported)`. **Does not persist** — the UI must round-trip the result through `save_settings` so the side effects and sanitisation apply. |

---

### 3.2 LM Studio (3)

These names say "lmstudio" for historical reasons; the same code path serves all four
providers (`lmstudio`, `openrouter`, `gemini`, `huggingface`) over the OpenAI-compatible
API.

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 15 | `lmstudio_test` | `url?: string`, `apiKey?: string`, `provider?: string` | `ConnectionStatus` | **async.** `{ connected, serverUrl, modelCount, latencyMs, api }`. If **both** the URL and the key are unchanged it probes the live shared service; otherwise it builds a **throwaway** `LmStudioService` with a hard-coded 10 s timeout, so a candidate configuration never pollutes the shared service or the model cache. `provider` defaults to the saved one. Errors map to `lmstudio_unavailable` / `lmstudio_error` / `timeout`. |
| 16 | `lmstudio_models` | `refresh: boolean` | `ModelInfo[]` | **async.** `true` forces a hard refresh from the provider; `false` serves the cache. `ModelInfo` carries `toolUse`, `vision`, `reasoning` and `free` capability flags. |
| 17 | `lmstudio_auto_selection` | — | `ModelSelection \| null` | **async.** `{ modelId, needsLoad, score, reasons[] }`, or `null` when a loaded model wins outright and no choice had to be made. `reasons` maps to the `settings.ai.reasons.*` string family. |

---

### 3.3 Window (7)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 18 | `window_set_compact` | `compact: boolean` | `void` | sync. Shrinks the chat window to 380×170 and disables geometry persistence. |
| 19 | `window_hide` | — | `void` | sync. `hide_to_tray` — the window closes to the system tray; the process stays alive. |
| 20 | `window_minimize` | — | `void` | sync. `minimize_to_bubble` — emits `app://window-closing {fx, fy}` to `main`, sleeps 190 ms (the CSS shrink), hides, then shows the 84×84 bubble. |
| 21 | `window_toggle_maximize` | — | `boolean` | sync. Returns whether the window is now maximized. |
| 22 | `window_show_main` | — | `void` | sync. `show_main(app, focus_input = true)`. Pre-arms `app://window-shown` **while hidden**, snaps the position, hides the bubble and shows the chat in the same tick. Emits `app://window-shown {origin, animated, fx, fy}` to `main` and, because `focus_input` is true, `app://focus-input` to `main`. |
| 23 | `bubble_open_chat` | — | `void` | sync. Called by the bubble when clicked; identical to `window_show_main`. |
| 24 | `open_settings_window` | `section?: string` | `void` | sync. Focuses the pre-created (never destroyed) settings window and emits `app://navigate "/settings/{section ?? general}"` to it, then applies the accent icon **after** `show()` (Windows only builds the taskbar button on first show). Logs `tracing::info!(section = ?section, "open_settings_window command invoked")`. **Must not be routed onto the main thread** — see [developer_guide §12.6](./developer_guide.md#126-smaller-items-worth-knowing). |

---

### 3.4 Chat (4)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 25 | `chat_send` | `input: SendInput`, `onEvent: Channel<ChatEvent>` | `void` | **async. Streams over a Channel.** `SendInput = { turnId, conversationId: string \| null, text, spokenLanguage: string \| null, voice, attachmentIds, webSearchEnabled?: boolean, mcpEnabled?: Record<string, boolean> }`. `webSearchEnabled` and `mcpEnabled` are session-only overrides; when omitted, settings/DB defaults apply. **`mcpEnabled` is always sent by the chat window** (the map may be empty): the per-server filter runs only when the field is present, so a `null` here means "no filter" and exposes every enabled MCP server. A server absent from the map is not filtered out; the chat window seeds every configured server with `false`, so the model sees no MCP tool until the user switches one on. **Never fails** — the result only signals completion; errors arrive as a terminal `ChatEvent::Error`. Cancel with `chat_stop(turnId)`. |
| 26 | `chat_stop` | `turnId: string` | `void` | sync. Cancels a live turn. Also cancels an `chat_explain` stream by passing the explain id. |
| 27 | `chat_explain` | `id: string`, `selection: string`, `passage: string`, `onEvent: Channel<ExplainEvent>` | `void` | **async. Streams over a Channel.** Explains selected reply text outside the conversation (the pop-up) or as the next chat message. `selection` is clipped to 4 000 chars, `passage` to 12 000. Also **never fails** — errors arrive as `ExplainEvent::Error`. `ExplainEvent` has **no `turnId`**: the caller-supplied `id` identifies the stream. |
| 28 | `chat_confirm_tool` | `callId: string`, `approved: boolean` | `boolean` | sync. Resolves a pending tool confirmation. Returns **whether a pending confirmation was actually accepted**, so the UI can detect a stale dialog. |

---

### 3.5 Attachments (6)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 29 | `attach_files` | `paths: string[]` | `AttachResult` | sync. **Per-file fault-tolerant**: each path is ingested independently, so one bad file never loses the rest. Returns `{ attachments, failures }` — one message per file that could not be attached. It essentially never returns `Err`; **the caller must inspect `failures`**. |
| 30 | `attach_bytes` | `name: string`, `mime?: string \| null`, `data: string` | `Attachment` | sync. For bytes that never existed as a file (a clipboard paste). `data` is base64, optionally still wrapped in a `data:` URL. Decode failure is `invalid`. |
| 31 | `attach_text` | `name: string`, `text: string` | `Attachment` | sync. Attaches a long block of text as a file (the `pasteAsFileChars` threshold). |
| 32 | `attach_remove` | `id: string` | `void` | sync. **Fire-and-forget.** Discards the staged copy; a failed disk cleanup is invisible. Used when a staged image is dropped because the model is blind. |
| 33 | `attachment_data_url` | `id: string`, `mime: string` | `string` | sync. Returns `data:{mime};base64,{…}` so history messages can re-render image previews. The **MIME type comes from the UI**, not from the store. |
| 34 | `attachment_text` | `id: string` | `string` | sync. Extracted text for the "show contents" preview. `not_found` when the attachment has no extractable text. |

---

### 3.6 Conversations (9)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 35 | `conv_list` | `limit?: number`, `offset?: number` | `Conversation[]` | sync. `limit` defaults to **100** in Rust, but the TS wrapper defaults to **200**, so 200 is what actually goes over the wire. Capped at **1000**. `offset` exists in Rust but the typed client never sends it, so it is always 0. |
| 36 | `conv_search` | `query: string` | `SearchHit[]` | sync. FTS5 search. Hard-capped at **100** results, not configurable. An empty query behaves like "list everything" (asserted by `db_smoke`). |
| 37 | `conv_get` | `id: string` | `[Conversation, Message[]]` | sync. **Filters messages to `role == "user" \|\| "assistant"`** — system and tool rows are deliberately hidden from the restored transcript. |
| 38 | `conv_rename` | `id: string`, `title: string` | `void` | sync. |
| 39 | `conv_delete` | `id: string` | `void` | sync. Also clears `settings.lastConversationId` when it pointed at the deleted conversation, so the app cannot start on a deleted row. |
| 40 | `conv_clear_all` | — | `number` | sync. Deletes every conversation, clears the pointer, then **garbage-collects the attachment files** against the remaining `db.attachment_ids()`. Returns the number of conversations removed. |
| 41 | `conv_set_last` | `id: string \| null` | `void` | sync. Persists the last-opened conversation. `null` clears it. |
| 42 | `conv_export` | `ids: string[]`, `format: "json" \| "markdown" \| "txt"`, `path: string` | `void` | sync. An empty `ids` is `invalid: "nothing to export"`. `format` is a **typed Rust enum**, so the wire contract is closed. Writes to the UI-supplied path. |
| 43 | `conv_import` | `path: string` | `string[]` | sync. **Size gate: 200 MiB** (`200 * 1024 * 1024`), rejected with `invalid: "file is too large"`. Returns the ids of the imported conversations. |

---

### 3.7 Voice (23)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 44 | `audio_devices` | — | `{ inputs: AudioDevice[]; outputs: AudioDevice[] }` | **async** (`spawn_blocking`). `AudioDevice = { id, name, isDefault }`. **Loopback inputs are excluded** by a 14-entry name blacklist (`stereo mix`, `what you hear`, `cable output`, `vb-audio`, `voicemeeter`, …) so the PC's own output is never fed back into recognition. Outputs are not filtered. Enumeration failure yields empty arrays, never an error. |
| 45 | `voice_start` | `mode: "pushToTalk" \| "handsFree" \| "dictation" \| "test"` | `number` | sync. **Returns the session number** that `voice://event` `State` events carry — this is how the UI correlates partials to a session and rejects a late `idle` from a previous one. Stops TTS first for every mode except `test`, so the assistant can never hear itself. Stops and **joins** any previous session before reopening the microphone. Emits `State { state: "listening" }` synchronously. |
| 46 | `voice_stop` | `discard: boolean` | `"pushToTalk" \| "handsFree" \| "dictation" \| "test" \| null` | sync. Returns the mode that was active, or `null` if idle. `discard: true` drops the audio (retry / cancel); `false` keeps it ("Insert now"). Never blocks — the thread is joined by the **next** `voice_start`. Emits `State { state: "idle" }` on exit. |
| 47 | `dictation_cancel` | — | `void` | sync. Esc / overlay ✕. Drops the audio, skips the insert, shows "cancelled". **No-op when idle.** Emits `dictation://state { state: "cancelled" }`. |
| 48 | `dictation_insert_now` | `text: string` | `void` | sync. Stops with `discard = false` **only** when the active mode is `dictation`. When review-before-insert is enabled, opens an editable preview immediately with the supplied live partial text while final transcription continues; the final review event replaces it unless the user has edited it. |
| 49 | `dictation_confirm` | `text: string` | `void` | **async.** Inserts the (possibly edited) result the overlay holds in review mode. Takes the pending review; `invalid: "nothing is waiting to be inserted"` if there is none. Shrinks the overlay, restores focus to the dictated window, waits 150 ms, then inserts. Emits `dictation://state { state: "inserted" \| "empty", result }` and `dictation://history`. |
| 50 | `dictation_retry` | — | `void` | **async.** Discards the current take (listening or under review) and re-arms. **Returns early when transcribing or correcting** — nothing to retry yet. |
| 51 | `dictation_set_language` | `language: string` | `void` | sync. Remembers the language chosen in the overlay. `""` means "follow the speech-recognition language". Emits `settings://changed`. |
| 52 | `dictation_history` | — | `DictationEntry[]` | sync. `{ id, createdAt, raw, text, corrected, inserted }`. `inserted: false` records that typing/pasting into the target app failed. |
| 53 | `dictation_history_delete` | `id: string` | `void` | sync. |
| 54 | `dictation_history_clear` | — | `void` | sync. |
| 55 | `dictation_reset_overlay_position` | — | `void` | sync. Reverts the dragged overlay position to the default centred-near-the-bottom placement. |
| 56 | `dictation_overlay_menu` | `menu: [number, number, number, number] \| null` | `void` | sync. Shows the overlay's language menu (logical `x, y, width, height` in window coordinates) outside the card, or hides it again with `null`. The rect is merged into the window region so the menu can hang outside the card without moving the window. |
| 57 | `voice_status` | — | `"pushToTalk" \| "handsFree" \| "dictation" \| "test" \| null` | sync. The active mode, or `null`. |
| 58 | `voice_set_muted` | `muted: boolean` | `boolean` | sync. Mutes the microphone **without ending the session**, so a hands-free call can be held. Returns the resulting state. While muted, silence still counts toward the idle timeout and the level meter reads zero. |
| 59 | `voice_muted` | — | `boolean` | sync. |
| 60 | `tts_voices` | — | `VoiceInfo[]` | sync. `VoiceInfo = { id, modelId, name, language, speakerId, engine, quality, gender }`. `language` is `"*"` for a multilingual voice. `id` is `"<model_id>:<speaker_id>"`. |
| 61 | `tts_speak` | `text: string`, `language?: string \| null`, `tag?: string` | `void` | sync. Long text is **sentence-split inside the service** so speech starts immediately and can be paused or stopped at any point. `tts_unavailable` if there is no output device. An unknown `language` silently becomes the default voice. `tag` is the caller-supplied identity used to group and cancel one utterance; the default is a fresh UUID. Emits `tts://event`. |
| 62 | `tts_set_paused` | `paused: boolean` | `{ speaking: boolean; paused: boolean }` | sync. Returns the resulting state so the UI never has to guess. |
| 63 | `tts_state` | — | `{ speaking: boolean; paused: boolean }` | sync. `speaking` is true even while paused; the store additionally tracks `paused` separately. |
| 64 | `tts_test` | `language: string`, `text?: string` | `void` | sync. Speaks `text` with the voice chosen for `language`, or a **hard-coded sample sentence** when `text` is empty. `invalid: "language"` for an unknown code; `tts_unavailable` when no voice is installed for that language. Uses the fixed tag `"test-voice"`, so repeated tests replace each other. |
| 65 | `tts_stop` | — | `void` | sync. `stop_all` — stops the player, invalidates queued jobs and emits `tts://event { type: "idle" }`. |
| 66 | `tts_replay_last` | — | `boolean` | sync. Replays the last completed turn. Returns whether a replay was possible. |

---

### 3.8 Models (6)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 67 | `models_catalog` | — | `CatalogEntry[]` | sync. The curated catalogue, flattened with runtime state: `id, kind, engine, name, languages, quality, minRamGb, license, downloadBytes, installed, recommended, downloading, deletable`. `recommended` comes from the hardware-agnostic starter set; `deletable = !PROTECTED_MODELS.contains(id)`. The **built-in VAD model is filtered out** — it ships inside the exe and has nothing to download. |
| 68 | `models_installed` | — | `InstalledModel[]` | sync. Every model found in the app's models folder and in every configured extra folder, deduplicated by path. Handles plain folders, grouped folders and the Hugging Face `models--org--name/snapshots/<hash>/` layout. |
| 69 | `models_incompatible` | — | `IncompatibleModel[]` | sync. Folders found in **extra** folders only that have no `.onnx` but do have PyTorch weights (`.safetensors` / `.pt` / `.pth` / `.ckpt` / `pytorch_model.bin`) or `.gguf`. Each carries a **reason** — "GGUF models run in LM Studio, not in the local speech engine" or "PyTorch weights: this app needs an ONNX export (sherpa-onnx format)" — so they are listed, not silently hidden. |
| 70 | `models_download` | `ids: string[]` | `void` | **async. Fire-and-forget.** All-or-nothing on validation: one unknown id aborts the whole batch with `invalid: "unknown model id"`. Then every not-yet-installed model is **reserved with a `CancellationToken` immediately** and `models://changed` is emitted, so the UI greys them out as queued at once and they cannot be selected twice. The actual work is one spawned task installing models **sequentially**. Per model: skip if already installed, treat a missing token as "cancelled before it started", emit `models://download` per tick, release the reservation, and on success **reload STT if a model is loaded** (a new model may now be the automatic pick). Every file is SHA-256 verified. |
| 71 | `models_cancel` | `id: string` | `void` | sync. Cancels a queued or in-flight download via its token. A cancelled download reports `models://download { state: "cancelled" }` and cleans up the staging directory. |
| 72 | `models_delete` | `id: string` | `void` | sync. `invalid: "built-in models cannot be deleted"` for a protected id. If the deleted model is the **loaded STT model** it is unloaded **first** (its files are in use), then deleted, then STT is reloaded so speech keeps working without a wait. Ids containing `/`, `\` or `..` are rejected and the resolved path must stay inside the models directory, so models in **extra folders are never removed by the app**. Emits `models://changed`. |

---

### 3.9 GPU pack (5)

The optional NVIDIA CUDA pack for the local speech models. The libraries are chosen at
process start, which is why most of this takes effect only after a restart.

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 73 | `gpu_status` | — | `GpuStatus` | sync. `{ supported, gpuName, installed, sizeBytes, downloadBytes, active, restartRequired }`. `restartRequired` is true when the pack is installed and enabled but this process has not activated it. |
| 74 | `gpu_set_enabled` | `enabled: boolean` | `void` | sync. Writes the flag. **Takes effect only after a restart**, because `services::gpu::activate_early()` relaunches the process with the pack's DLLs ahead of the bundled CPU ones before `main` runs. Emits `gpu://changed`. |
| 75 | `gpu_install` | — | `void` | **async. Fire-and-forget.** `invalid: "no NVIDIA GPU was detected"` when `hardware.has_nvidia()` is false. **Idempotent** — an existing `"gpu-pack"` reservation returns `Ok(())` immediately. On success the pack is **auto-enabled** ("installing it is what the user asked for"). ~1.65 GiB across four archives (sherpa-onnx CUDA + cuDART + cuBLAS + cuDNN from `developer.download.nvidia.com`, so no CUDA toolkit is needed). Emits `gpu://download` per tick and `gpu://changed` at the end. |
| 76 | `gpu_cancel` | — | `void` | sync. Cancels an in-flight pack download. |
| 77 | `gpu_remove` | — | `void` | sync. Removes the pack **and forces `set_enabled(false)`**, so removal can never leave a dangling enabled flag. Emits `gpu://changed`. |

---

### 3.10 Models in memory (3)

The key namespace is a cross-module contract shared with `AppState::model_loading`:
`"stt"`, `"voice:en" | "voice:ar" | "voice:de"`, `"llm"` (the chat model as a chat would
pick it), `"llm:<model id>"`, and the special `"all"` accepted **only** by `memory_load`.

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 78 | `memory_status` | — | `MemoryItem[]` | **async.** `{ key, kind, role, model, state, detail, autoloadKey, autoload }` with `state` ∈ `loaded \| loading \| idle \| missing \| failed`. `detail` is `"GPU"`/`"CPU"` for ONNX models and `"LM Studio"` for LLM entries. **LLM entries exist only for `provider === "lmstudio"`** — a hosted provider keeps no models on this computer. A resolver failure yields one synthetic `key: "llm", state: "failed"` entry rather than an error. |
| 79 | `memory_load` | `key: string` | `void` | **async.** Loads one model and returns once it is in memory. `"all"` loads everything (ignoring the per-model autoload opt-outs) and **returns immediately** — the work is spawned. Each load marks the key as `loading` and emits `memory://changed` **before** awaiting, and again after, so the UI shows a spinner at once. `"voice:<code>"` caps synthesis at `min(inferenceThreads, 4)`. `"llm:<id>"` calls `load_model` **then** `resolver.invalidate()`. Unknown key → `invalid: "unknown model {key}"`. |
| 80 | `memory_unload` | `key: string` | `void` | **async.** `"stt"` is refused with `invalid: "speech recognition is in use; stop listening first"` whenever a listening mode is active — the app never yanks a model out from under a live session. The bare `"llm"` key is **not** supported for unload. Emits `memory://changed`. |

---

### 3.11 MCP & web search (13)

| # | Command | Parameters | Returns | Notes |
| --- | --- | --- | --- | --- |
| 81 | `mcp_list` | — | `ServerStatus[]` | **async.** `{ config, state, error, tools, internet }` with `state` ∈ `disabled \| connecting \| connected \| error \| offline`. `tools` is `ToolView[]` = `{ name, description, category, permission, defaultPermission }`. A tool is **never returned** when its effective permission is `deny`. |
| 82 | `mcp_save` | `config: McpServerConfig` | `McpServerConfig` | **async.** A server with an **empty `id` is new** and is always stored `enabled: false`, `source: "user"`. For an existing server the incoming `enabled` is **ignored** and the stored value is re-applied, so the UI cannot flip connection state behind the manager's back. If the stored server is enabled a connect is attempted (errors ignored). |
| 83 | `mcp_delete` | `id: string` | `void` | **async.** Disconnects **first**, then deletes the row — no dangling client. |
| 84 | `mcp_set_enabled` | `id: string`, `enabled: boolean` | `void` | **async.** The explicit user action that enables a server. MCP servers are never enabled automatically. |
| 85 | `mcp_reconnect` | `id: string` | `void` | **async. Fire-and-forget.** Always returns `Ok(())`; a connect failure is visible only through `mcp://changed` and the new `state`. |
| 86 | `mcp_set_permission` | `serverId: string`, `tool: string`, `permission: "allow" \| "ask" \| "deny"` | `"allow" \| "ask" \| "deny"` | **async.** Returns the **effective** permission after clamping, so with safe mode on (the default) a request for `allow` on a sensitive tool returns **`ask`**, not `allow`. |
| 87 | `mcp_set_all_permissions` | `serverId: string`, `permission: Permission \| null` | `void` | **async.** `null` resets every tool of the server to its `defaultPermission`. |
| 88 | `mcp_safe_mode` | — | `boolean` | sync. Safe mode **resets to on at every launch**, so it can never persist as off across a restart. |
| 89 | `mcp_set_safe_mode` | `enabled: boolean` | `void` | **async. The one OS-verification gate in the whole API.** Turning safe mode **off** requires Windows Hello via an HWND-parented `UserConsentVerifier` prompt ("Confirm it's you to turn off MCP safe mode"); turning it **on** never does, so the fail-safe direction is cheap. `Cancelled` if the user dismisses the prompt; `PermissionDenied` if Windows Hello is not set up or verification fails — **and on non-Windows it always denies** (fail-closed, never a silent bypass). This is the only command that takes a `WebviewWindow` rather than an `AppHandle`. |
| 90 | `mcp_import_preview` | — | `ImportCandidate[]` | sync. Reads LM Studio's `mcp.json`. A **missing file is not an error** — an empty list is returned. `alreadyConfigured` uses a case-insensitive name match. **`envKeys` contains only the key names, never the values** — a deliberate secret-safety rule. |
| 91 | `mcp_import` | `names: string[]` | `number` | sync. Imports the selected candidates, **always disabled** with `source: "lmstudio"`. Skips candidates not in `names` and those whose lowercase name already exists; saves only those passing `validate()`. Returns the **count actually saved**, so the user can tell that some were rejected. A missing `mcp.json` **is** an error here (`not_found`). |
| 92 | `search_public_instances` | `refresh: boolean` | `PublicSearxInstance[]` | **async.** Public SearXNG instances from **searx.space**: `{ url, searchSuccess, searchTime, version }`. `refresh` forces a reload instead of using the cache. |
| 93 | `search_test` | — | `{ engine: string; count: number; firstTitle: string \| null }` | **async.** Runs **one real, uncached** search with the hard-coded probe query `"open source search engine"`, bypassing the cache. Doubles as a connectivity test and a label check. `engine` names the engine that actually answered, e.g. `"SearXNG (https://searx.be/)"` or `"Built-in search (DuckDuckGo)"`. |

---

### 3.12 Worked invocation examples

All examples are plain JavaScript using the raw Tauri API, as they would appear in a
`console` inside the webview devtools. With `import { invoke, Channel } from
"@tauri-apps/api/core"`.

#### `chatSend` — a streaming turn

```js
import { invoke, Channel } from "@tauri-apps/api/core";

const turnId = crypto.randomUUID();
const onEvent = new Channel();
const done = { ok: false, message: null };

onEvent.onmessage = (ev) => {
  switch (ev.type) {
    case "started":
      console.log("turn", ev.turnId, "model", ev.model, "lang", ev.language,
                  "new", ev.newConversation);
      break;
    case "delta":
      process.stdout.write(ev.text);            // batch in the UI, not here
      break;
    case "reasoning":
      /* model thinking, only if settings.ai.showReasoning */
      break;
    case "toolStarted":
    case "toolAwaitingConfirmation":
      console.log(ev.serverName, ev.toolName, ev.category, ev.args);
      if (ev.type === "toolAwaitingConfirmation") {
        const approved = await askTheUser(ev);  // -> chat_confirm_tool
        await invoke("chat_confirm_tool", { callId: ev.callId, approved });
      }
      break;
    case "toolFinished":
      console.log(ev.ok ? "ok" : "failed", ev.durationMs + "ms",
                  "denied:", ev.denied, "sources:", ev.sources);
      break;
    case "done":
      done = { ok: true, message: ev.message };
      break;
    case "error":
      // NOTE: `code` "cancelled" is the user pressing Stop - render nothing.
      done = { ok: false, code: ev.code, detail: ev.detail,
               partial: ev.partialMessage };
      break;
  }
};

await invoke("chat_send", {
  input: {
    turnId,
    conversationId: null,          // null = start a new conversation
    text: "What is the latest PHP version?",
    spokenLanguage: null,          // set when the text came from speech
    voice: false,                  // true = voice barge-in (may interrupt)
    attachmentIds: [],
  },
  onEvent,                         // the Channel, a sibling of `input`
});
```

Cancel from anywhere with `await invoke("chat_stop", { turnId })`.

#### `chatExplain` — explain selected reply text in a pop-up

```js
const id = crypto.randomUUID();   // identifies the stream; also its cancel key
const onEvent = new Channel();

onEvent.onmessage = (ev) => {
  // ExplainEvent has NO turnId - the caller-supplied id identifies it.
  if (ev.type === "delta") popUpAppend(ev.text);
  if (ev.type === "done") popUpFinish();
  if (ev.type === "error") popUpFail(ev.code, ev.detail);
};

await invoke("chat_explain", {
  id,
  selection: "preload (PHP)",                 // clipped to 4 000 chars
  passage: assistantReplyText,                // context, clipped to 12 000
  onEvent,
});

// To cancel: await invoke("chat_stop", { turnId: id });
```

#### `convExport` — export selected conversations to a file

```js
// `format` is a closed Rust enum - anything else fails to deserialise.
for (const format of ["json", "markdown", "txt"]) {
  try {
    await invoke("conv_export", {
      ids: [convId1, convId2],
      format,
      path: `C:\\Users\\me\\Documents\\export.${format === "markdown" ? "md" : format}`,
    });
    console.log("exported", format);
  } catch (e) {
    // e === { code: "invalid", detail: "Invalid input: nothing to export" }
    console.error("export failed", e.code, e.detail);
  }
}
```

Pair it with `await invoke("dialog", …)` in practice — in the app the path comes from
the save picker (`@tauri-apps/plugin-dialog`'s `save`), not from a hard-coded string.

#### `modelsDownload` — download catalogue models with progress

```js
import { listen } from "@tauri-apps/api/event";

const un = await listen("models://download", (e) => {
  const p = e.payload;   // { modelId, state, downloadedBytes, totalBytes, error }
  const pct = p.totalBytes ? Math.round((p.downloadedBytes / p.totalBytes) * 100) : 0;
  renderProgress(p.modelId, p.state, pct, p.error);
  // state: downloading | verifying | extracting | done | error | cancelled
});

// One unknown id aborts the whole batch: { code: "invalid", detail: "…unknown model id" }
await invoke("models_download", { ids: ["whisper-small", "silero-vad", "supertonic-3-int8"] });

// ... and it returns immediately. The work is a spawned task; watch the event.
await invoke("models_cancel", { id: "whisper-turbo" });   // stops one download
un();
```

#### `mcpSetSafeMode` — the Windows Hello gate

```js
try {
  // Turning OFF requires a Windows Hello prompt; turning ON never does.
  await invoke("mcp_set_safe_mode", { enabled: false });
  console.log("safe mode off - risky tools can now be allowed without asking");
} catch (e) {
  switch (e.code) {
    case "cancelled":
      console.log("user dismissed the prompt - safe mode unchanged (still ON)");
      break;
    case "permission_denied":
      showError("Set up a PIN in Windows Settings > Accounts > Sign-in options.");
      break;
    default:
      showError(e.detail);
  }
}
```

#### `voiceStart` — push-to-talk, with session correlation

```js
import { listen } from "@tauri-apps/api/event";

const un = await listen("voice://event", (e) => {
  const ev = e.payload;
  // Ignore events from an older session: a late `idle` must not end this one.
  if (ev.mode === "dictation" || ev.mode === "test") return;
  if (ev.session < lastSession || ev.session <= endedSession) return;
  endedSession = Math.max(endedSession, ev.session);

  if (ev.type === "level")     setLevelMeter(ev.value, ev.bands);
  if (ev.type === "partial")   setDraft(ev.text);          // live, not final
  if (ev.type === "transcript") { /* final: send or append */ }
  if (ev.type === "error")     showError(ev.code, ev.detail);
});

// Returns the session number the state events will carry.
const session = await invoke("voice_start", { mode: "pushToTalk" });
lastSession = session;

// Later:
const mode = await invoke("voice_stop", { discard: false });   // keep the take
un();
```

#### `ttsSpeak` — speak text, grouped and cancellable by tag

```js
// A tag groups one utterance. Re-using it cancels the previous one;
// omitting it generates a fresh UUID, so two speaks never collide.
const tag = "read-aloud-1";

await invoke("tts_speak", {
  text: "Hello. This is a sentence. And this is another one.",
  language: "en",   // null / omitted -> the default voice
  tag,
});

await listen("tts://event", (e) => {
  const ev = e.payload;
  // speaking | sentence | paused | resumed | idle | voiceUnavailable | error
  if (ev.type === "speaking" && ev.tag === tag) setSpinner(true);
  if (ev.type === "sentence") highlightWord(ev.text, ev.durationMs);
  if (ev.type === "idle")     setSpinner(false);
  if (ev.type === "voiceUnavailable")
    showNotice(`No voice installed for ${ev.language}`);
});

await invoke("tts_set_paused", { paused: true });    // -> { speaking, paused }
await invoke("tts_stop");                            // stops everything
```

---

## 4. Events

18 global events. Payload types are the TypeScript definitions in `src/app/types.ts`.

### 4.1 Full event table

| Event | Payload type | Emitted by | Subscribed by | Payload shape |
| --- | --- | --- | --- | --- |
| `settings://changed` | `Settings` | `commands/app.rs` (`save_settings`, `complete_first_run`), `commands/voice.rs` (`dictation_set_language`), `desktop/window.rs` | `src/app/settingsStore.ts` (`subscribe()`) | The whole sanitised `Settings` document. **Broadcast** so every window re-themes. This is the only cross-window settings sync mechanism. |
| `voice://event` | `VoiceEvent` | `src/lib.rs::on_voice_event` (from `VoiceSessions`) | `src/app/voiceStore.ts`, `src/pages/Bubble/Bubble.tsx`, `src/pages/Overlay/Overlay.tsx`, `src/pages/Settings/sections/Voice.tsx` | Internally tagged union (5 variants). **Broadcast**, except `Level` and `Partial` for `mode: "dictation"`, which are `emit_to(OVERLAY)` only. `State` carries the monotonic `session` number. |
| `voice://error` | `AppErrorPayload` | `src/desktop/shortcuts.rs` (a failed `voice.start(PushToTalk)` from the hotkey) | `src/app/voiceStore.ts` | `{ code, detail }` — the serialised `AppError`. Distinct from `VoiceEvent::Error` because it reports a session that **never started**. |
| `tts://event` | `TtsEvent` | `src/lib.rs` (the `TtsService` event callback) | `src/app/voiceStore.ts`, `src/pages/Bubble/Bubble.tsx` | Internally tagged union (7 variants). `Speaking`/`Sentence` carry the `tag`; `Sentence` also carries `text` (sound tags already stripped) and `durationMs`, which drives per-word highlighting. |
| `dictation://state` | `DictationStateEvent` | `src/lib.rs` (`run_dictation`, `insert_and_report`, `confirm_review`, `on_voice_event`) | `src/pages/Overlay/Overlay.tsx`, `src/pages/Bubble/Bubble.tsx`, `src/pages/Chat/ChatApp.tsx` | `{ state, text?, error?, result? }` with `state` ∈ `listening \| transcribing \| idle \| correcting \| review \| inserted \| empty \| cancelled \| error`. `error` is an `AppErrorPayload`; `result` is `{ raw, inserted, corrected, correctionError }`. **A late `idle` after an Esc-cancel is reported as `cancelled`, never `idle`.** |
| `dictation://history` | `null` (empty) | `src/lib.rs::insert_and_report` | `src/pages/Settings/sections/Dictation.tsx` | **No payload** — a bare `()`. Purely a "reload the list" signal. Only emitted when the write to history succeeded. |
| `models://download` | `DownloadProgress` | `src-tauri/src/commands/voice.rs` (`models_download` progress callback) | `src/components/settings/ModelManager.tsx`, `src/pages/Setup/SetupWizard.tsx` | `{ modelId, state, downloadedBytes, totalBytes, error }`, `state` ∈ `downloading \| verifying \| extracting \| done \| error \| cancelled`. Emitted at most every **250 ms**. The `verifying` state is where SHA-256 comparison happens. |
| `models://changed` | `null` (empty) | `commands/app.rs` (`save_settings`, extra model dirs), `commands/voice.rs` (`models_download`, `models_delete`) | `src/components/settings/ModelManager.tsx`, `src/pages/Settings/sections/Voice.tsx` | **No payload.** Fires when the catalogue/installed set, the recommendation or the download reservations change. |
| `gpu://changed` | `null` (empty) | `commands/voice.rs` (`gpu_set_enabled`, `gpu_install`, `gpu_remove`) | `src/components/settings/GpuCard.tsx`, `src/pages/Settings/sections/Voice.tsx` | **No payload.** Also emitted at the end of `gpu_install`. |
| `gpu://download` | `GpuProgress` | `commands/voice.rs` (`gpu_install` progress callback) | `src/components/settings/GpuCard.tsx` | `{ state, downloadedBytes, totalBytes, error }`, `state` ∈ `downloading \| extracting \| done \| error \| cancelled`. Note there is **no `verifying`** state — the pack archives are not digest-pinned. |
| `memory://changed` | `null` (empty) | `commands/memory.rs` (`tracked()` before and after each load, `memory_unload`) | `src/pages/Settings/sections/Memory.tsx` | **No payload.** Emitted **before** awaiting a load as well as after, so the UI shows a spinner immediately. That page also polls every 3 s as a safety net. |
| `mcp://changed` | `null` (empty) | `src/lib.rs` — the `McpManager` change callback, installed **at construction** so no manager exists without it | `src/pages/Settings/sections/Mcp.tsx` | **No payload.** Fires on any server state or tool-list change, including `connect_enabled()` at startup and `shutdown()`. `privacy_status` is **not** driven by it — it reads the statuses directly. |
| `app://window-shown` | `{ origin?: string; animated?: boolean; fx?: number; fy?: number }` | `src/desktop/window.rs::show_main` → **to `main` only** | `src/pages/Chat/ChatApp.tsx` | The bubble→chat morph handshake. Emitted **while the webview is still hidden** (pre-arm), then a 50 ms sleep, so the first visible frame is already mid-zoom. `fx`/`fy` are the zoom origin in main-window logical pixels. Deduplicated in the UI because a rapid double event replays the zoom. |
| `app://window-closing` | `{ fx?: number; fy?: number }` | `src/desktop/window.rs::minimize_to_bubble` → **to `main` only** | `src/pages/Chat/ChatApp.tsx` | The chat→bubble shrink. The bubble centre; the shell sleeps 190 ms (the CSS duration) before hiding. |
| `app://focus-input` | `null` (empty) | `src/desktop/window.rs::show_main` (`focus_input == true`) → **to `main` only** | `src/pages/Chat/ChatApp.tsx` | **No payload.** "Put the caret in the composer and close the History panel." Deliberately **not** sent by the tray's "Voice Mode" item, which must not steal focus. |
| `app://navigate` | `string` | `src/desktop/window.rs::open_settings` → **to `settings` only** | `src/pages/Settings/SettingsApp.tsx` | `"/settings/{section}"`, e.g. `"/settings/general"` or `"/settings/diagnostics"`. The handler sets `location.hash`, so settings navigation is URL-addressable and the tray's Diagnostics item works. |
| `app://new-conversation` | `null` (empty) | `src/desktop/tray.rs` (the "New Conversation" menu item) → **to `main` only** | `src/pages/Chat/ChatApp.tsx` | **No payload.** Closes the History panel and starts a fresh conversation. |
| `app://toggle-hands-free` | `null` (empty) | `src/desktop/tray.rs` (the "Voice Mode" menu item) → **to `main` only** | `src/pages/Chat/ChatApp.tsx` | **No payload.** Calls `useVoice.getState().toggleHandsFree()`. |

### 4.2 Event usage rules

- **Never assume a listener will be attached when the event fires.** A `listen` returns a
  promise, so subscribe first and only then issue the command that triggers the event.
- **`voiceStore` registers its `voice://event`, `tts://event` and `voice://error`
  listeners exactly once per chat window and never tears them down** (a module-level
  `subscription` promise is the idempotency key). This is deliberate: re-subscribing on a
  React StrictMode remount would duplicate handlers, and an unmount must not leave a
  window without any. `main.tsx` calls `useVoice.subscribe()` once, for the `chat` route
  only.
- **`voice://event` is ignored for `mode: "dictation"` and `mode: "test"`.** The chat
  window must not react to the overlay's or the microphone-test's events; the overlay
  filters for `dictation` and the Voice settings section for `test`.
- **The `null`-payload events really carry nothing.** They are pure "your data changed"
  signals; the correct handler is to re-read the relevant command.

---

## 5. Channels

Exactly two commands stream over a `tauri::ipc::Channel`. Both never fail — the result
only signals completion, and errors arrive as a terminal event.

### 5.1 `chat_send` — `ChatEvent`

Rust: `src-tauri/src/services/chat/orchestrator.rs`, serialised with
`#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]`.
TypeScript: `ChatEvent` in `src/app/types.ts`.

| `type` | Fields | When | Notes |
| --- | --- | --- | --- |
| `started` | `turnId: string`, `conversationId: string`, `userMessage: Message`, `model: string`, `language: string`, `newConversation: boolean` | Once, immediately. | `userMessage` is the **persisted** user message, so the optimistic bubble can be replaced by the real row. `newConversation` tells the UI to open a new conversation rather than append. |
| `delta` | `turnId: string`, `text: string` | Repeatedly while tokens stream. | The UI batches these at most every 50 ms; rendering each one re-parses the whole Markdown message. |
| `reasoning` | `turnId: string`, `text: string` | While a reasoning model streams thinking. | Shown only when `ai.showReasoning` is on. Batched with `delta` through the same buffer. |
| `toolStarted` | `turnId`, `callId`, `serverName`, `toolName`, `category: ToolCategory`, `args: unknown` | When a tool begins executing. | **Idempotent by `callId`**, so the `toolAwaitingConfirmation → toolStarted` transition never duplicates an entry. `category` ∈ `search \| fetch \| read \| write \| execute \| other`. |
| `toolAwaitingConfirmation` | `turnId`, `callId`, `serverName`, `toolName`, `category`, `args` | When the policy requires user consent. | The UI must call `chat_confirm_tool({ callId, approved })`. The dialog is a **write/execute/unclassified** tool, or any sensitive tool with safe mode on. |
| `toolFinished` | `turnId`, `callId`, `ok: boolean`, `durationMs: number`, `resultPreview: string`, `sources: Source[]`, `denied: boolean` | When a tool finishes, successfully or not. | **Sources shown in the UI come only from URLs the tools actually returned**, de-duplicated by `url` in the store. `denied: true` means the user (or the policy) refused. |
| `done` | `turnId: string`, `message: Message` | Once, on success. | The final persisted assistant message. The store keeps the live tool details when the persisted message has none. |
| `error` | `turnId: string`, `code: string`, `detail: string`, `partialMessage: Message \| null` | Once, on failure or cancellation. | `code` is an `AppError::code()`. **`code === "cancelled"` renders no banner** — the user pressed Stop. `lmstudio_unavailable` and `no_model` additionally populate the persistent connection banner. `partialMessage` is what had been streamed, kept so the bubble is not lost. |

**Ordering guarantee:** non-text events call `flushStream()` first, so anything other
than text always lands **after** the text streamed before it.

**Stale-turn guard:** a `delta` with an unknown `turnId` is ignored. This is what makes
barge-in and rapid re-sends safe.

### 5.2 `chat_explain` — `ExplainEvent`

Rust: `src-tauri/src/services/chat/explain.rs`, same serde attributes.
TypeScript: `ExplainEvent` in `src/app/types.ts`.

| `type` | Fields | When |
| --- | --- | --- |
| `delta` | `text: string` | Repeatedly while the explanation streams. |
| `done` | — | Once, on success. |
| `error` | `code: string`, `detail: string` | Once, on failure. |

**There is no `turnId`** — the stream is identified by the caller-supplied `id`
argument, and cancelled with `chat_stop({ turnId: id })`. The prompt is deliberately
minimal: explain the selection in plain words, briefly, in the language of the selected
text, using the surrounding reply only as context, without repeating the selection or
opening with a preamble. The selection is clipped to 4 000 characters and the passage to
12 000.

---

## 6. TypeScript client

`src/app/ipc.ts` is the **only** frontend module that names a Rust command. It exports
the `ipc` object, the two error helpers, an event helper and an id generator. No wrapper
does try/catch — every call rejects and every **caller** decides its policy, which is
what allows the three error tiers in [§7](#7-error-handling-conventions) to coexist.

```ts
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export function isAppError(e: unknown): e is AppErrorPayload
export function toAppError(e: unknown): AppErrorPayload
export const ipc: { /* 93 methods, grouped as in §3 */ }
export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn>
export function newId(): string
```

### 6.1 Method signatures by domain

**Settings & app**

```ts
getSettings(): Promise<Settings>
saveSettings(settings: Settings): Promise<Settings>
shortcutErrors(): Promise<string[]>
shortcutsCapture(capturing: boolean): Promise<void>
appInfo(): Promise<AppInfo>
openFolder(which: "logs" | "models" | "data"): Promise<void>
readLogTail(lines?: number): Promise<string>
scanCapabilities(): Promise<CapabilityReport>
privacyStatus(): Promise<PrivacyStatus>
appReady(): Promise<void>
completeFirstRun(): Promise<void>
quit(): Promise<void>
settingsExport(path: string, includeKeys: boolean): Promise<void>
settingsImport(path: string): Promise<Settings>
```

**LM Studio**

```ts
lmstudioTest(url?: string, apiKey?: string, provider?: string): Promise<ConnectionStatus>
lmstudioModels(refresh: boolean): Promise<ModelInfo[]>
lmstudioAutoSelection(): Promise<ModelSelection | null>
```

**Window**

```ts
setCompact(compact: boolean): Promise<void>
hideWindow(): Promise<void>
minimizeWindow(): Promise<void>
toggleMaximize(): Promise<boolean>
showMain(): Promise<void>
bubbleOpenChat(): Promise<void>
openSettings(section?: string): Promise<void>
```

**Chat**

```ts
chatSend(
  input: { turnId: string; conversationId: string | null; text: string;
           spokenLanguage: string | null; voice: boolean; attachmentIds: string[] },
  onEvent: (e: ChatEvent) => void,
): Promise<void>                                   // builds a Channel internally
chatStop(turnId: string): Promise<void>
chatExplain(id: string, selection: string, passage: string,
            onEvent: (e: ExplainEvent) => void): Promise<void>
chatConfirmTool(callId: string, approved: boolean): Promise<boolean>
```

**Attachments**

```ts
attachFiles(paths: string[]): Promise<AttachResult>
attachBytes(name: string, mime: string | null, data: string): Promise<Attachment>
attachText(name: string, text: string): Promise<Attachment>
attachRemove(id: string): Promise<void>
attachmentDataUrl(id: string, mime: string): Promise<string>
attachmentText(id: string): Promise<string>
```

**Conversations**

```ts
convList(limit = 200): Promise<Conversation[]>
convSearch(query: string): Promise<SearchHit[]>
convGet(id: string): Promise<[Conversation, Message[]]>
convRename(id: string, title: string): Promise<void>
convDelete(id: string): Promise<void>
convClearAll(): Promise<number>
convSetLast(id: string | null): Promise<void>
convExport(ids: string[], format: "json" | "markdown" | "txt", path: string): Promise<void>
convImport(path: string): Promise<string[]>
```

**Voice**

```ts
audioDevices(): Promise<{ inputs: AudioDevice[]; outputs: AudioDevice[] }>
voiceStart(mode: ListenMode): Promise<number>            // the session number
voiceStop(discard: boolean): Promise<ListenMode | null>
dictationCancel(): Promise<void>
dictationInsertNow(): Promise<void>
dictationConfirm(text: string): Promise<void>
dictationRetry(): Promise<void>
dictationSetLanguage(language: string): Promise<void>
dictationHistory(): Promise<DictationEntry[]>
dictationHistoryDelete(id: string): Promise<void>
dictationHistoryClear(): Promise<void>
dictationResetOverlayPosition(): Promise<void>
dictationOverlayMenu(menu: [number, number, number, number] | null): Promise<void>
voiceStatus(): Promise<ListenMode | null>
voiceSetMuted(muted: boolean): Promise<boolean>
voiceMuted(): Promise<boolean>
ttsVoices(): Promise<VoiceInfo[]>
ttsSpeak(text: string, language?: string | null, tag?: string): Promise<void>
ttsSetPaused(paused: boolean): Promise<{ speaking: boolean; paused: boolean }>
ttsState(): Promise<{ speaking: boolean; paused: boolean }>
ttsTest(language: string, text?: string): Promise<void>
ttsStop(): Promise<void>
ttsReplayLast(): Promise<boolean>
```

**Models**

```ts
modelsCatalog(): Promise<CatalogEntry[]>
modelsInstalled(): Promise<InstalledModel[]>
modelsIncompatible(): Promise<IncompatibleModel[]>
modelsDownload(ids: string[]): Promise<void>
modelsCancel(id: string): Promise<void>
modelsDelete(id: string): Promise<void>
```

**GPU pack**

```ts
gpuStatus(): Promise<GpuStatus>
gpuSetEnabled(enabled: boolean): Promise<void>
gpuInstall(): Promise<void>
gpuCancel(): Promise<void>
gpuRemove(): Promise<void>
```

**Models in memory**

```ts
memoryStatus(): Promise<MemoryItem[]>
memoryLoad(key: string): Promise<void>            // "stt" | "voice:<code>" | "llm" | "llm:<id>" | "all"
memoryUnload(key: string): Promise<void>
```

**MCP & web search**

```ts
mcpList(): Promise<ServerStatus[]>
mcpSave(config: McpServerConfig): Promise<McpServerConfig>
mcpDelete(id: string): Promise<void>
mcpSetEnabled(id: string, enabled: boolean): Promise<void>
mcpReconnect(id: string): Promise<void>
mcpSetPermission(serverId: string, tool: string, permission: Permission): Promise<Permission>
mcpSetAllPermissions(serverId: string, permission: Permission | null): Promise<void>
mcpSafeMode(): Promise<boolean>
mcpSetSafeMode(enabled: boolean): Promise<void>    // turning OFF needs Windows Hello
mcpImportPreview(): Promise<ImportCandidate[]>
mcpImport(names: string[]): Promise<number>
searchPublicInstances(refresh: boolean): Promise<PublicSearxInstance[]>
searchTest(): Promise<SearchTestResult>
```

### 6.2 `on<T>` — the event helper

```ts
export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}
```

It unwraps Tauri 2's `Event<T>` envelope to just `e.payload`, and resolves to the
unlisten function. `listen` returns a **promise**, so the listener is not registered
until it resolves — a common race when the triggering command is issued immediately
afterwards.

### 6.3 `newId`

```ts
export function newId(): string { return crypto.randomUUID(); }
```

Used for `turnId`, `conversationId` and the explain `id`. The chat store additionally
prefixes derived ids: `local-<turnId>` for the optimistic user bubble and
`failed-<turnId>` for a failed reply.

### 6.4 The error helpers

```ts
export function isAppError(e: unknown): e is AppErrorPayload {
  return typeof e === "object" && e !== null && "code" in e && "detail" in e;
}

export function toAppError(e: unknown): AppErrorPayload {
  if (isAppError(e)) return e;
  return { code: "other", detail: e instanceof Error ? e.message : String(e) };
}
```

`isAppError` is a **structural** check — any object with `code` and `detail` passes,
including a hand-rolled one from a test. `toAppError`'s `"other"` fallback is a real key
in `strings.en.json`, so the message always renders. Use `toAppError` at the point of
display, `isAppError` where you need to branch on a specific code.

---

## 7. Error-handling conventions

The three stores implement **three distinct tiers**. Nothing in `ipc.ts` decides policy;
this is entirely a caller-side choice.

### 7.1 Tier 1 — silent (no user-visible output)

The failure is invisible, or is only visible through a state change the user did not
initiate. Reserved for background work and for user-initiated cancellations.

| Case | Behaviour |
| --- | --- |
| `ChatEvent::Error` with `code === "cancelled"` | No banner. The user pressed Stop; an empty cancelled bubble is dropped entirely. |
| `chatSend` / `chatExplain` returning `Ok(())` | Not a failure at all — the result only signals completion. |
| `modelsDownload`, `gpuInstall`, `modelsCancel`, `gpuCancel`, `ttsStop`, `dictationRetry`, `attachRemove` | Fire-and-forget. The outcome is observable only through `models://changed` / `gpu://changed` and the disappearance of the download reservation. |
| `mcpReconnect` | Always `Ok(())`; a failure surfaces as the new `state` in `mcpList`. |
| `shortcutErrors` returning a non-empty array | Surfaced in the settings UI, never fatal. The app never refuses to start because a hotkey is taken. |
| `saveSettings` autostart failure | Logged at `warn` only; the save still succeeds. |
| Dictation grammar-correction failure | Warn-only; the **raw transcript is still inserted** — degraded, not blocked. |

### 7.2 Tier 2 — in-band (a message, a row, or a dialog)

The failure belongs to one piece of content. It is rendered **with** that content, not
as an application-level banner.

| Case | Behaviour |
| --- | --- |
| `ChatEvent::Error` with any code other than `cancelled` | An inline error on the assistant bubble, with a **Send again** affordance. `partialMessage` is kept so the streamed text is not lost. |
| `attachFiles` returning `failures` | A dismissible per-file list next to the composer. One bad file never loses the rest of the batch. |
| `conv_list` / `conv_search` rejecting | The History panel renders the error. **A database failure must never masquerade as "no conversations"** — there is a regression test for exactly this. |
| `modelsInstallIncompatible` entries | Listed **with their reason** rather than silently hidden. |
| `mcp` `status.error` | Rendered in that server's row. |
| `DictationStateEvent.state === "error"` | Rendered in the overlay, which then lingers 2 200 ms before hiding. |
| `tts` `TtsEvent.voiceUnavailable` | A notice naming the language, with the correct fallback so it can never render "undefined". |

### 7.3 Tier 3 — escalated (a persistent, application-level banner)

The app as a whole is impaired. The user must fix something outside the current view.

| Case | Behaviour |
| --- | --- |
| `ChatEvent::Error` with `code` `lmstudio_unavailable` **or** `no_model` | Also populates the persistent `connectionError` banner above the composer — with a direct link to the relevant settings section. |
| `scan_capabilities` reporting `lmStudio.connected === false` | Drives the setup wizard; a scan itself never fails, it reports. |
| `AppError::permission_denied` from `mcp_set_safe_mode` | A modal telling the user how to set up a PIN in Windows Settings. |
| `AppError::download` on a checksum mismatch | Escalated, because it is an integrity failure, not a transient one. |
| `voice://error` | A notice in the chat window; the session never started. |

### 7.4 The rejection-of-`invoke`-must-be-caught rule

Because no wrapper in `ipc.ts` catches, an unhandled rejection is possible. The stores
all wrap their calls and route the result through `toAppError`, and
`settingsStore.update` additionally **rolls back both the state and the theme** on
failure, since it applies optimistically before the round trip.

---

## 8. Security

### 8.1 Content Security Policy

`tauri.conf.json`, enforced by Tauri on every webview:

```
default-src 'self';
img-src 'self' data: asset:;
style-src 'self' 'unsafe-inline';
font-src 'self' data:;
connect-src ipc: http://ipc.localhost;
script-src 'self'
```

with `freezePrototype: true`.

- **`script-src 'self'` — no `unsafe-inline`, no `unsafe-eval`.** No inline script runs
  in a built app; `index.html`'s two classic scripts are part of the document, not an
  injected string. There is no `eval` of remote content anywhere in the frontend.
- **`style-src` needs `'unsafe-inline'`** because `settingsStore.applyAppearance` writes
  13 custom properties and `--font-scale` onto the document root at runtime. There is no
  alternative short of a full stylesheet rebuild per theme change.
- **`connect-src` allows only the IPC endpoints.** No `https:` wildcard, so a
  XSS-injected `fetch` to an external host is blocked by the webview. All outbound HTTP
  is Rust's job.
- **`freezePrototype: true`** prevents tampering with `Object.prototype` and friends.

There is deliberately **no** `<meta http-equiv="Content-Security-Policy">` in
`index.html`; the policy comes from the Tauri config only, so there is one source of
truth.

### 8.2 `withGlobalTauri: false`

The `window.__TAURI__` global is **not** injected. The frontend must import from
`@tauri-apps/api/*`, which means bundler-resolved, statically analysable imports only —
anything dynamic or injected at runtime has no path to the IPC bridge. This is the
single strongest frontend-side control.

### 8.3 The 9 Tauri capabilities

`src-tauri/capabilities/default.json` — one capability, `default`, covering
`windows: ["main", "settings", "overlay", "bubble"]`:

| # | Permission | Grants |
| --- | --- | --- |
| 1 | `core:default` | The default core set (events, app, path, window basics, image, menu, tray, webview). |
| 2 | `core:window:allow-start-dragging` | The frameless title-bar drag. |
| 3 | `core:window:allow-start-resize-dragging` | Frameless edge and corner resize. |
| 4 | `core:window:allow-set-position` | `setPosition` from JS, used only in custom-position mode. |
| 5 | `dialog:allow-open` | The open picker: attachments, model folders, settings backup import. |
| 6 | `dialog:allow-save` | The save picker: settings backup export. |
| 7 | `opener:allow-open-url` | `openUrl` for external links. |
| 8 | `opener:allow-default-urls` | Restricts `openUrl` to the allow-listed URL set. |
| 9 | `global-shortcut:default` | The global-shortcut plugin's default set. |

**Posture notes.**

- **There is no `fs`, `shell` or `http` permission.** All filesystem and network work
  goes through Rust commands, which validate their inputs (`open_folder`'s allow-list,
  `safe_join`'s `Component` walk, `models::delete`'s traversal rejection). The webview
  cannot read a file or open a socket.
- **Window creation stays in Rust.** The app does not request
  `core:webview:allow-create-webview-window`, so the webview cannot spawn a new window.
- **`ui.test.tsx` asserts `global-shortcut:default` is present** by reading
  `capabilities/default.json` off disk — a cross-language architectural invariant guarded
  by a test.
- **Known asymmetry:** a single capability covers all four windows, so there is **no
  per-window least-privilege split**. The overlay can technically call `open_settings`.
  Tightening this is a natural hardening step.

### 8.4 The MCP safe-mode Windows Hello gate

`mcp_set_safe_mode(false)` is the **only** command in the whole API gated on OS
verification, and it fails **closed**:

- The gate is `desktop::verify.rs::verify_user`, invoked with an `HWND`-parented
  `UserConsentVerifier` prompt ("Confirm it's you to turn off MCP safe mode"). It runs
  entirely on `spawn_blocking` because the WinRT `.join()` calls block.
- **Windows Hello not set up** → `PermissionDenied` with a message naming the fix
  ("Set up a PIN in Windows Settings › Accounts › Sign-in options").
- **User dismisses the prompt** → `Cancelled`, a distinct variant so the UI can treat it
  as "the user backed out" rather than a failure. **Safe mode stays on.**
- **Non-Windows** → always `PermissionDenied("User verification is only supported on
  Windows.")`. **Never a silent bypass.**
- **Safe mode resets to `true` at every launch**, so it can never persist as off across
  a restart.
- Turning safe mode **on** never requires verification, so the fail-safe direction is
  free.

Underneath, the permission policy is a two-layer clamp:

```
classify(name, description, readOnlyHint, destructiveHint) -> ToolCategory
  Search | Fetch | Read            -> default Allow
  Write  | Other                   -> default Ask
  Execute                           -> default Deny      (and hidden from the model)
default_permission(category)       = the table above
clamp_permission(category, req)    = if req == Allow && category.is_sensitive() { Ask } else { req }
                                     where is_sensitive = Write | Execute | Other
```

So with safe mode on (the default), **no write, execute or unclassified tool can ever run
without asking**, and `mcp_set_permission` returns the **effective** permission so the UI
cannot display a value the backend did not apply. MCP servers are never installed or
enabled automatically: new and imported servers are always stored `enabled: false`, and
`mcp_save` on an existing server re-applies the stored `enabled` value.

### 8.5 `env_keys`-only rule for MCP environment previews

`mcp_import_preview` returns `envKeys: string[]` — the **names** of the environment
variables found in LM Studio's `mcp.json`, **never the values**. A preview list is
rendered on screen, is included in the settings export path, and can end up in a support
log or a screenshot; a credential must never be in that shape. The values are read from
disk only when the user actually saves the server.

The same discipline applies to settings: API keys live in the **Windows Credential
Manager**, not in the settings JSON, `settings_export` strips them unless
`includeKeys` is explicitly set, and logs never contain conversation content unless
`general.logConversationContent` is switched on.

### 8.6 Outbound data

Exactly seven flows leave the machine, each enumerated in `PRIVACY.md` and each gated on
a user choice or an explicit user action: chat with LM Studio (local), chat with a
hosted provider (OpenRouter / Google Gemini / Hugging Face, only if chosen — the app
shows a notice), web search (only when the assistant uses the search tool), model
downloads (only on clicking Download), the NVIDIA GPU pack (only when installed), MCP
tools (only servers the user added; write/command tools ask first), and dictation
grammar cleanup (optional). There is **no telemetry** and no project-operated server.

### 8.7 Download integrity

`AGENTS.md` §3: *anything downloading a file must verify SHA-256.* The pipeline
downloads to a staging directory, streams the body, and **hashes each chunk in the same
loop that writes it**, so the digest covers exactly the bytes written. After `flush()`
the state becomes `verifying` and the digest is compared case-insensitively; a mismatch
is `Download("checksum mismatch for {url}")` and the staging directory is removed. Every
catalogue entry carries a digest. Archive extraction goes through `safe_join`, which
pushes `Normal` components, ignores `CurDir`, and **rejects everything else** —
`RootDir`, `Prefix` and `ParentDir` — so a malicious archive cannot escape the model
directory.
