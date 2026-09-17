# Local Assistant

A small, fast, local-first desktop AI assistant. It lives in a floating window (bottom-right by default), runs in the system tray, and opens with a global shortcut. You can type or talk to it in **English, Arabic or German**, and it can answer out loud with natural local voices.

- **LLM:** LM Studio only (its local OpenAI-compatible server). No other LLM backend, no cloud fallback.
- **Speech recognition:** local Whisper models via [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx).
- **Text-to-speech:** local neural voices. English uses Kokoro, and German and Arabic use Piper/VITS.
- **Tools:** generic MCP (Model Context Protocol) client with explicit per-tool permissions. Web search comes from an MCP server you choose.
- **Dictation:** optional global hotkey that types what you say into any app, with optional grammar cleanup by a small LM Studio model you pick.
- **Privacy:** conversations, settings and logs stay on disk locally. There is no telemetry. Logs contain no conversation content unless you turn that on.

## Requirements

- Windows 10/11 (the primary target; the code also builds for macOS and Linux)
- [LM Studio](https://lmstudio.ai) with the local server running (default `http://localhost:1234/v1`) and at least one model downloaded
- For development: Node 20+, Rust (stable, MSVC toolchain on Windows), and the WebView2 runtime

The first build downloads the prebuilt static sherpa-onnx libraries from the sherpa-onnx GitHub releases. This happens at build time only.

## Getting started

```bash
npm install
npm run tauri dev
```

On first launch, a setup wizard does the following:

1. Scans for LM Studio, models, microphones, speakers and installed voice models.
2. Lets you choose the response language (Automatic by default).
3. Offers to download voice models recommended for your hardware. Nothing is downloaded unless you click **Download**. Files come from pinned Hugging Face or GitHub URLs and are checked against SHA-256 hashes.

You can also copy compatible sherpa-onnx model folders into `%APPDATA%\com.localassistant.app\models`. The app detects them automatically.

### Default shortcuts

| Action | Shortcut |
| --- | --- |
| Open / focus assistant | `Ctrl+Space` |
| Push-to-talk (hold) | `Ctrl+Shift+Space` |
| Dictation into other apps (off by default) | `Ctrl+Alt+Space` |
| New conversation / History / Settings (in window) | `Ctrl+N` / `Ctrl+H` / `Ctrl+,` |

All shortcuts can be changed in **Settings → Keyboard Shortcuts**.

## Architecture

```
src/                         React + TypeScript UI (English only)
  app/                       typed IPC, zustand stores, UI strings
  components/ pages/         chat window, settings dashboard, setup wizard, dictation overlay
src-tauri/src/
  services/ai/               LM Studio client (SSE streaming, tool calls), automatic model selection
  services/chat/             orchestrator (history, tool rounds, permissions), prompt, freshness detection
  services/stt/              Whisper transcription, listening sessions (push-to-talk, hands-free VAD, dictation)
  services/tts/              sentence buffer, per-sentence language → voice selection, synthesis queue
  services/audio/            cpal capture (16 kHz mono) and playback queue
  services/language/         English/Arabic/German detection for typed and spoken text
  services/mcp/              MCP manager (stdio + streamable HTTP), permission policy, source extraction
  services/models/           voice model catalog, discovery, consented downloads
  services/dictation.rs      correction with a small LM Studio model, text insertion
  capabilities/              LocalCapabilityManager (scan of everything available locally)
  database/                  SQLite (conversations, messages + FTS5, settings, MCP config)
  desktop/                   tray, floating window placement and persistence, global shortcuts
```

### Key behaviours

- **Model selection:** loaded models win. Otherwise the app scores models by whether they fit in VRAM (or RAM on machines without a GPU), how practical their parameter count is, whether they support tool use, their context size and their quantization. It does not simply pick the largest model. You can always choose a model manually.
- **Current information:** the system prompt tells the model to use a web-search tool for "latest / current / today…" questions. If no search tool is enabled, the model must say it cannot verify current information. Sources shown in the UI come only from URLs the tools actually returned.
- **Tool security:** search, fetch and read tools are allowed by default. Tools that write data, and tools that can't be classified, always ask for confirmation. Command-execution tools are disabled by default, and they can never be set to run without confirmation. MCP servers are never installed or enabled automatically.
- **Streaming speech:** LLM tokens are buffered into complete sentences. Code blocks are skipped, and decimals, abbreviations and URLs don't end a sentence. Each sentence is spoken as soon as it is complete, in a voice that matches its detected language.
- **Voice acceleration:** this build runs the voice models on the CPU.

## Tests

```bash
# Rust unit + integration tests (LM Studio mock server, MCP fixture server over stdio, DB, language, TTS buffering…)
cd src-tauri && cargo test

# Frontend tests (RTL rendering, streaming chat store, tool UI, shortcuts, every UI string key exists)
npm test

# Optional: real voice models (downloads ~560 MB into the given folder)
LA_MODELS_DIR=/path/to/models cargo test --test voice_models -- --nocapture

# Optional: live test against your running LM Studio (streaming, 3 languages, web-search tool use)
LA_LIVE_LMSTUDIO=1 cargo test --test lmstudio_live -- --nocapture
```

## Build an installer

```bash
npm run tauri build
```

This produces NSIS and MSI installers in `src-tauri/target/release/bundle/`.
