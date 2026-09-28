# DEVELOPER GUIDE

**Project:** Open Local Assistant (`local-ai-assistant` v1.0.0)
**Repository:** `https://github.com/kz370/Open-Local-Ai-Assistant`
**License:** GPL-3.0-only
**Audience:** contributors and AI agents changing this repository

> Companion documents: [`full_documentation.md`](./full_documentation.md) (per-file
> analysis), [`api_reference.md`](./api_reference.md) (the complete IPC contract),
> [`runtime.md`](./runtime.md) (documentation pipeline state).
> `AGENTS.md` at the repository root has **binding** precedence over model defaults;
> the contribution section below summarises it.

---

## Table of Contents

1. [Prerequisites and verified toolchain](#1-prerequisites-and-verified-toolchain)
2. [Setup](#2-setup)
3. [Run in development](#3-run-in-development)
4. [Dependencies](#4-dependencies)
5. [Build, test and quality gates](#5-build-test-and-quality-gates)
6. [Test suite](#6-test-suite)
7. [Debugging](#7-debugging)
8. [Extension guide](#8-extension-guide)
9. [Style rules](#9-style-rules)
10. [Contribution guide](#10-contribution-guide)
11. [Packaging and release](#11-packaging-and-release)
12. [Known gaps and gotchas](#12-known-gaps-and-gotchas)

---

## 1. Prerequisites and verified toolchain

| Requirement | Minimum | Verified in this workspace | Notes |
| --- | --- | --- | --- |
| **Node.js** | 20 | **v24.19.0** | CI pins **Node 22** (`actions/setup-node@v4`). Node 20+ is the floor because `vite@8` and `vitest@5` require it. |
| **npm** | ships with Node | **12.0.1** | CI uses the npm cache from `package-lock.json`, so the lockfile must stay in sync with `package.json`. |
| **Rust (stable, MSVC)** | stable | **cargo 1.98.1 / rustc 1.98.1** (2026-08-05 / 2026-09-01) | Install from [rustup.rs](https://rustup.rs). On Windows the `stable-x86_64-pc-windows-msvc` toolchain is required — the GNU toolchain cannot link the Tauri/WebView2 stack. |
| **C++ build tools** | MSVC build tools workload | — | Required by `windows`-crate consumers and by `sherpa-onnx`'s build script on a cold cache. |
| **WebView2 Runtime** | Evergreen | preinstalled on Windows 11 | Already present on Windows 10 1803+ as an app, but not as an OS component. Without it the window never renders. |
| **Inno Setup 6 or 7** | 6 | — | Only needed to produce `Open-Local-Assistant-<version>-setup.exe`. `build-installer.bat` **searches for 7 only**; see [§12](#12-known-gaps-and-gotchas). |
| **LM Studio** | optional | — | Required to actually chat locally. The app is fully usable with a hosted provider instead. |
| **GitHub CLI (`gh`)** | optional | — | Required only by `upload-release.bat` and by the interactive upload prompt at the end of `build-installer.bat`. |
| **Python** | optional | — | Only to re-run `scripts/generate_icon.py`; the icon is committed. |

**Platform.** Windows 10/11 x64 is the primary and only tested target. The Rust tree
compiles in principle on macOS and Linux (`desktop/mod.rs` has **no** `#[cfg]` gating;
each module handles divergence internally), but this is untested and not supported.

---

## 2. Setup

```bash
npm install
```

That is the whole frontend setup. There is **no** `ui/` directory — the React app lives
at the repository root in `src/`, and `vite.config.ts`, `tsconfig.json` and
`package.json` are all root-level.

### The first `cargo build`

The very first Rust build downloads a **prebuilt static sherpa-onnx library archive**
from the sherpa-onnx GitHub releases into `src-tauri/target/`. This happens **at build
time only** — nothing is vendored in the repository. The archive name encodes the
pinned version, for example:

```
sherpa-onnx-v1.13.8-win-x64-shared-MT-Release-lib.tar.bz2
```

`Cargo.toml` enables `sherpa-onnx` with `default-features = false, features = ["shared"]`
so the ONNX Runtime and sherpa-onnx DLLs are **dynamically** linked. That is what lets
the optional NVIDIA CUDA pack replace them at launch (see `services::gpu`).

Expect a long first build. `build-installer.bat` halves the parallelism so the machine
stays usable, and CI overrides that with `BUILD_JOBS=%NUMBER_OF_PROCESSORS%`.

### The `.build-cache\sherpa-onnx` cache

`cargo clean` deletes `target/`, which also deletes the downloaded sherpa-onnx archive —
so every clean build would re-download it. The build pipeline therefore keeps its own
copy:

| Path | Contents | Git |
| --- | --- | --- |
| `.build-cache\sherpa-onnx\sherpa-onnx-v<version>-win-x64-shared-MT-Release-lib.tar.bz2` | one archive per version | **git-ignored on purpose** — it must survive `cargo clean` |

`build-installer.bat` manages it in three steps:

1. **Before the build** it reads the pinned version out of `src-tauri/Cargo.lock` with a
   regex (`name = "sherpa-onnx-sys"` / `version = "…"`), and if the matching archive is
   already in `.build-cache\sherpa-onnx\` it sets `SHERPA_ONNX_ARCHIVE_DIR` so the
   sherpa-onnx build script uses the cached copy instead of downloading. Output line:
   `sherpa-onnx 1.13.8: using the cached copy, no download`.
2. **After the build** it copies `target\sherpa-onnx-prebuilt\<archive>` back into
   `.build-cache\` the first time it is produced:
   `sherpa-onnx 1.13.8: saved to .build-cache for next time`.
3. **After an upgrade** the version in `Cargo.lock` changes, the cached filename no
   longer matches, and the new archive is fetched and cached automatically. No manual
   step is needed.

**If you run `cargo build` directly** (not through the batch file) the cache is not
consulted and the archive is re-downloaded into `target/` on every clean. Use
`build-installer.bat` for release builds, and accept the re-download during plain
iteration.

### Runtime data directory

The app writes everything under the Tauri app-data directory, on Windows
`%APPDATA%\com.localassistant.app`:

| Path | Contents |
| --- | --- |
| `assistant.db` | SQLite: conversations, messages + FTS5, settings, MCP config, dictation history |
| `models\` | downloaded voice and speech models |
| `attachments\` | staged composer attachments |
| `logs\` | rolling `tracing` output (see [§7](#7-debugging)) |

Delete the whole directory to uninstall cleanly.

---

## 3. Run in development

```bash
npm run tauri dev
```

This is `tauri dev`, which:

1. runs `beforeDevCommand` → `npm run dev` (Vite),
2. waits for the dev server,
3. `cargo build`s and launches `src-tauri/src/main.rs`.

### The dev server

Configured in `vite.config.ts`; `tauri.conf.json` points at the same URL.

```ts
server: {
  port: 1420,
  strictPort: true,
  host: host || "127.0.0.1",          // host = process.env.TAURI_DEV_HOST
  hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
  watch: { ignored: ["**/src-tauri/**"] },
}
```

- **Port 1420 is `strictPort: true`.** Vite will fail rather than fall back to 1421/1422,
  because `tauri.conf.json` hard-codes `"devUrl": "http://127.0.0.1:1420"`. If something
  already owns 1420, free it — do not change the port in one file only.
- **IPv4 `127.0.0.1` on purpose.** The source comment records the reason: on Windows
  `localhost` can resolve and bind to `[::1]` only, and the Tauri webview then cannot
  reach the dev server. Do not "fix" this to `localhost`.
- **`watch.ignored: ["**/src-tauri/**"]`** stops Rust rebuild output from triggering a
  frontend reload. Rust changes are picked up by the `tauri dev` watcher, not Vite.
- `clearScreen: false` — Tauri needs the terminal to show the build output.

### `TAURI_DEV_HOST` and HMR

Set `TAURI_DEV_HOST` to expose the dev server on a network interface instead of loopback
(e.g. for a phone or a second machine on the LAN, or for a Linux/WSL host):

```bash
set TAURI_DEV_HOST=192.168.1.20
npm run tauri dev
```

It is read defensively off `globalThis.process?.env`, because the config is also
evaluated in contexts without `process`. When it is set, the host becomes the HMR
websocket host too, on **port 1421** over `ws://` — so 1421 must also be reachable.
When it is unset, HMR uses the default (same-origin websocket on 1420).

### `LA_OPEN` — which window to open at startup

A debug-only environment variable read in `lib.rs::run()`'s `setup` closure, after the
tray and the windows are ready:

| Value | Effect |
| --- | --- |
| `LA_OPEN=chat` | `window::show_main(&handle, true)` — show and focus the chat window |
| `LA_OPEN=settings` | open the settings window on the `general` section |
| `LA_OPEN=settings:voice` | open the settings window on the named section |
| unset / anything else | normal startup (tray + bubble, or first-run wizard) |

```bat
set LA_OPEN=settings:voice
npm run tauri dev
```

The section suffix is split with `other.split_once(':')`, so exactly one `:` is
supported. Anything that is neither `chat` nor prefixed with `settings` is ignored.

---

## 4. Dependencies

### 4.1 Direct Rust crates (`src-tauri/Cargo.toml`)

Crate names are as declared; exact resolved patch versions live in `Cargo.lock`.

#### Build and framework

| Crate | Version | Why |
| --- | --- | --- |
| `tauri` | `2` (`image-png`, `tray-icon`) | Window/webview host, tray, menu, IPC. `image-png` decodes the generated tray/taskbar icon; `tray-icon` enables `TrayIconBuilder`. |
| `tauri-build` | `2` (build-dep) | `build.rs` calls `tauri_build::build()` to generate the context from `tauri.conf.json` and `capabilities/`. |
| `serde` | `1` (`derive`) | `Serialize`/`Deserialize` for every IPC payload and the settings document. |
| `serde_json` | `1` | `serde_json::Value` for tool arguments, event payloads and the settings blob. |

#### Plugins

| Crate | Version | Why |
| --- | --- | --- |
| `tauri-plugin-opener` | `2` | `open_url` for the links in the About section. |
| `tauri-plugin-dialog` | `2.7.3` | `open`/`save` pickers for attachments, model folders and the settings backup. |
| `tauri-plugin-global-shortcut` | `2.3.2` | The three global hotkeys (`Ctrl+Space`, `Ctrl+Shift+Space`, `Ctrl+Alt+Space`). |
| `tauri-plugin-single-instance` | `2.4.4` | Only one assistant runs; a second launch focuses the existing window. |
| `auto-launch` | `0.5` | "Start with Windows" via the registry. **Deliberately not** `tauri-plugin-autostart`, because the plugin registers `current_exe()`, which is wrong while the GPU pack is active — see `desktop/autostart.rs`. |

#### Async, errors, diagnostics

| Crate | Version | Why |
| --- | --- | --- |
| `tokio` | `1.53.1` (`full`) | The async runtime for every network call, stream, cancellation and sleep. |
| `tokio-util` | `0.7.19` | `CancellationToken` — the download registry, the dictation correction timeout and the TTS job generation. |
| `async-trait` | `0.1.92` | Object-safe `async` traits: `AiService`, `SpeechSink`, the orchestrator's test doubles. |
| `thiserror` | `2.0.20` | Derive for `AppError` and the other error enums. |
| `anyhow` | `1.0.104` | Startup/one-off context in `init_state` and the build script. |
| `tracing` | `0.1.44` | Structured logging macros used throughout. |
| `tracing-subscriber` | `0.3.23` (`env-filter`, `fmt`, `json`) | The subscriber, the `EnvFilter` driven by `LOCAL_ASSISTANT_LOG`, and the JSON formatter. |
| `tracing-appender` | `0.2.5` | Daily-rotation **non-blocking** file appender with a 7-file retention cap. |

#### Storage, crypto, parsing

| Crate | Version | Why |
| --- | --- | --- |
| `rusqlite` | `0.40.2` (`bundled`) | SQLite compiled in, so there is no system SQLite dependency. Conversations, messages with **FTS5**, settings, MCP config, dictation history. |
| `chacha20poly1305` | `0.10` | Authenticated encryption for the settings backup file. |
| `sha2` | `0.11.0` | **SHA-256 verification of every downloaded file** — a binding rule from `AGENTS.md` §3. |
| `hex` | `0.4.3` | Rendering and comparing digests. |
| `uuid` | `1.26.1` (`v4`) | Conversation/message/turn ids and the default TTS tag. |
| `chrono` | `0.4.45` (`serde`) | RFC 3339 timestamps in the database, the `.complete` marker and the prompt's date line. |
| `regex` | `1.13.1` | SSE parsing, `speech_text` Markdown stripping, HTML search-result parsing. |
| `url` | `2.5.8` | Host extraction in `privacy_status`, MCP HTTP endpoint validation. |
| `tar` | `0.4.46` | Model archive extraction. |
| `bzip2` | `0.6.1` | The `.tar.bz2` compression of the model archives. |
| `zip` | `2` (`deflate`, no default features) | Not used for models; available for the read side where an archive format is needed. |

#### Speech and audio

| Crate | Version | Why |
| --- | --- | --- |
| `sherpa-onnx` | `1.13.8` (`default-features = false`, `shared`) | **The whole speech engine**: Whisper / Parakeet-Nemo / SenseVoice / Moonshine recognition, Kokoro / Piper / Kitten / VITS / Supertonic synthesis, the Silero VAD, and `LinearResampler`. |
| `sherpa-onnx-sys` | `1.13.8` (`default-features = false`, `shared`) | Raw C API for the **one** call the Rust wrapper lacks: switching Whisper's language on an already-loaded model (`set_whisper_language`, `services/stt/engine.rs`). Must be kept at the same version as `sherpa-onnx`. |
| `cpal` | `0.18.2` | Microphone capture (16 kHz mono, downsampled + high-pass filtered) and PCM playback. |
| `enigo` | `0.6.1` | Typing dictation text into arbitrary foreground applications. |
| `arboard` | `3.6.1` | Clipboard for the dictation paste mode and the Explain pop-up's copy action. |

**Licensing note (important).** `sherpa-onnx` and the **ONNX Runtime** it embeds ship
under **their own licences** (Apache-2.0 for the sherpa-onnx wrapper; MIT for ONNX
Runtime) and are **not** covered by this project's GPL-3.0. They are dynamically linked
and the corresponding DLLs are redistributed in the installer and the portable zip.
This is called out in `README.md`, `LICENSE` notices and `PRIVACY.md`.

#### Network and MCP

| Crate | Version | Why |
| --- | --- | --- |
| `reqwest` | `0.13.5` (`default-features = false`; `json`, `stream`, `native-tls`, `http2`, `gzip`, `deflate`) | The OpenAI-compatible HTTP client, SSE streaming, model downloads, searx.space. `default-features = false` keeps OpenSSL out on Windows. |
| `futures-util` | `0.3.34` | `StreamExt` over SSE and download bodies. |
| `bytes` | `1.12.1` | Buffer type for streamed response bodies. |
| `rmcp` | `3.4.0` (`client`, `transport-child-process`, `transport-streamable-http-client-reqwest`) | The MCP client: stdio child-process and streamable-HTTP transports. |
| `sysinfo` | `0.39.6` | CPU/RAM/GPU inventory (`HardwareInfo`) for the capability scan, thread budgets and NVIDIA detection. |
| `whatlang` | `0.18.0` | English/Arabic/German language detection for typed and spoken text. |

#### Windows-only (`[target."cfg(windows)".dependencies]`)

| Crate | Version | Why |
| --- | --- | --- |
| `windows` | `0.62.2` | DXGI/GDI for GPU info; `Win32_UI_WindowsAndMessaging` for the frameless overlay, window-region clipping and `WM_SETICON`; `Win32_UI_Input_KeyboardAndMouse` for `GetAsyncKeyState`; `Win32_System_WinRT` + `Security_Credentials_UI` for the Windows Hello prompt; `Win32_System_JobObjects` for the GPU relaunch. |
| `windows-future` | `0.3.2` | The async WinRT bridge (`IUserConsentVerifierInterop`) used by `desktop/verify.rs`. |

#### Dev-dependencies

| Crate | Version | Why |
| --- | --- | --- |
| `axum` | `0.8.9` | The in-test LM Studio mock server (`services/ai/lmstudio.rs` tests and `tests/lmstudio_live.rs` harness). |
| `tempfile` | `3.27.0` | Temporary directories for database, settings-backup and model tests. |

#### Declared binaries

| Binary | Path | Test? |
| --- | --- | --- |
| `local-ai-assistant` | `src/main.rs` | yes |
| `mcp_fixture_server` | `src/bin/mcp_fixture_server.rs` | **`test = false`** — it is launched as a child process by `tests/mcp_integration.rs` and `tests/lmstudio_live.rs` via `env!("CARGO_BIN_EXE_mcp_fixture_server")` |
| `gpu_probe` | `src/bin/gpu_probe.rs` (auto-discovered) | standalone diagnostic, not part of the test gate |

### 4.2 Direct npm packages

#### Runtime dependencies

| Package | Version | Why |
| --- | --- | --- |
| `@tauri-apps/api` | `^2` | `invoke`, `Channel`, `listen`. The only Tauri import surface. |
| `@tauri-apps/plugin-dialog` | `^2.7.3` | `open` (file and folder pickers) and `save` (settings backup). |
| `@tauri-apps/plugin-global-shortcut` | `^2.3.2` | Keyboard-event helpers used by the shortcut recorder. |
| `@tauri-apps/plugin-opener` | `^2` | `openUrl` for external links. |
| `react` / `react-dom` | `^19.1.0` | UI. React 19 StrictMode double-invokes effects; `main.tsx` and `voiceStore` are written to survive that. |
| `zustand` | `^5.0.15` | The four stores: `chatStore`, `settingsStore`, `voiceStore`, `settingsHighlight`. |
| `react-markdown` | `^10.1.0` | Assistant replies. `remark-gfm` for tables, task lists and strikethrough. |
| `remark-gfm` | `^4.0.1` | GFM plugin for the above. |
| `lucide-react` | `^1.47.0` | The icon set. |
| `@tauri-apps/plugin-autostart` | `^2.5.1` | **Declared but unused.** Autostart is implemented in Rust with `auto-launch`; nothing in `src/` imports this package. Safe to drop, but see [§12](#12-known-gaps-and-gotchas). |

#### Development dependencies

| Package | Version | Why |
| --- | --- | --- |
| `@tauri-apps/cli` | `^2` | `tauri dev` / `tauri build`. |
| `vite` | `^8.0.16` | Dev server and production bundler (`target: "es2022"`). |
| `typescript` | `~6.0.3` | Type checking (`tsc --noEmit`) and the build's `tsc` step. `strict: true`. |
| `vitest` | `^5.0.1` | The frontend test runner, configured in `vite.config.ts`. |
| `jsdom` | `^30.1.0` | The test DOM environment. |
| `@vitejs/plugin-react` | `^6.0.2` | React Fast Refresh. The only Vite plugin. |
| `@testing-library/react` | `^16.3.3` | Component rendering. |
| `@testing-library/jest-dom` | `^7.0.1` | DOM matchers; loaded in `src/test/setup.ts`. |
| `@testing-library/user-event` | `^14.6.7` | Realistic keyboard/pointer interaction. |
| `@types/node` | `^22.20.3` | Needed by `tsconfig.json`'s `types` so tests can use `node:fs`. |
| `@types/react`, `@types/react-dom` | `^19.1.8`, `^19.1.6` | Type definitions. |

### 4.3 Model licences

Downloaded voice and speech models keep **their own** licences, recorded per entry in
the `CatalogModel::license` field and shown in Settings → Assistant voice:

| Model | Licence |
| --- | --- |
| `whisper-small`, `whisper-turbo` (OpenAI Whisper, int8 export) | **MIT** |
| `silero-vad` (Silero VAD) | **MIT** |
| `supertonic-3-int8` (Supertonic 3 multilingual TTS) | **OpenRAIL-M** |
| Kokoro voices (user-supplied) | **MIT** |
| Piper voices (user-supplied) | **MIT** |

`whisper-small` + `silero-vad` + `supertonic-3-int8` is the **recommended starter set**
(≈ 505 MB total, a test asserts it stays under 550 000 000 bytes). `whisper-turbo` adds
a further ≈ 1.04 GB.

---

## 5. Build, test and quality gates

### 5.1 `AGENTS.md` §2 is stale

`AGENTS.md` §2 ("Quality gates (existing, still required)") lists:

```text
cargo fmt --all
cargo clippy -p ols-core -p ols-helper --all-targets
cargo test -p ols-core -p ols-helper
cd ui && npm run lint && npm run build
```

**All four lines are wrong for this repository.** Do not follow them:

| `AGENTS.md` says | Reality |
| --- | --- |
| `cd ui && npm run lint` | **There is no `ui/` directory.** The frontend is at the repository root in `src/`. Worse, `package.json` has **no `lint` script at all** — the scripts are `dev`, `build`, `preview`, `tauri`, `test`, `test:watch` and `typecheck`. There is no ESLint, no Prettier and no lint step anywhere in the project; the linters are `cargo clippy` and `tsc`. |
| `cargo clippy -p ols-core -p ols-helper` | Those crates do not exist. The workspace has exactly one package: `local-ai-assistant` (lib name `local_ai_assistant_lib`, bin name `local-ai-assistant`). |
| `cargo test -p ols-core -p ols-helper` | Same. |
| `cargo fmt --all` | Correct, but it **rewrites** files. Use `--check` in a gate. |

The same `AGENTS.md` section also says "UI (`ui/src/`)" in the specs-sync mandate
([§10](#10-contribution-guide)); the real path is `src/`.

### 5.2 The corrected gate list

Run all of these before opening a pull request. Every command is from the repository
root unless noted.

```bash
# --- Rust ---
cd src-tauri
cargo fmt --all --check
cargo clippy -p local-ai-assistant --all-targets
cargo test  -p local-ai-assistant
cd ..

# --- Frontend ---
npm run typecheck
npm test
npm run build
```

What each one does and why it matters here:

| Command | Purpose |
| --- | --- |
| `cargo fmt --all --check` | rustfmt verification without writing. `cargo fmt --all` (no `--check`) is the fix. |
| `cargo clippy -p local-ai-assistant --all-targets` | `--all-targets` compiles the test binaries too, so lint errors in `#[cfg(test)]` code surface. There is **no** `#![deny(warnings)]` in the crate, so warnings do not fail the build — read them. |
| `cargo test -p local-ai-assistant` | 186 test functions (169 unit + 17 integration). The `#[ignore]`d ones are skipped. |
| `npm run typecheck` | `tsc --noEmit` with `strict: true`. Catches the React 19 and zustand typing mistakes `vite build` would only surface later. |
| `npm test` | `vitest run` — 8 test files, ~70 cases, in jsdom. |
| `npm run build` | `tsc && vite build`. `tsc` here is a real emit-less typecheck because `noEmit: true`; the output is `dist/`, which `tauri::generate_context!()` embeds. |

**Notes.**

- `npm run build` writes `dist/`. Cargo does not know `dist/` changed on its own, which
  is why `src-tauri/build.rs` contains `println!("cargo:rerun-if-changed=../dist")`.
  Without that directive a stale `.exe` ships an old UI.
- `cargo test` also runs the dev-dependency test targets; the LM Studio mock server in
  `services/ai/lmstudio.rs` binds a loopback port. Nothing in the default gate touches
  the network — every network-touching test is either `#[ignore]`d or environment-gated.
- **CI runs none of this.** `.github/workflows/build.yml` has a single `build` job and
  no test or lint step. The gates are local-only; see [§12](#12-known-gaps-and-gotchas).

---

## 6. Test suite

186 Rust test functions and 8 Vitest files. Everything below is gated so that a clean
checkout with no models, no microphone and no network still runs green.

### 6.1 Rust tests

**Unit tests live next to the code** in `#[cfg(test)] mod tests` blocks — **41** of them
across `src-tauri/src/`. They test pure logic and need no service, no network and no
models. Representative blocks:

| Area | Files | What is covered |
| --- | --- | --- |
| AI | `ai/sse.rs`, `ai/model_selector.rs`, `ai/lmstudio.rs` | SSE frame parsing (incl. `[DONE]`, split chunks, multi-line `data:`), the model scoring rules, the request builder. `lmstudio.rs` spins up an `axum` mock server. |
| Chat | `chat/orchestrator.rs`, `chat/prompt.rs`, `chat/freshness.rs`, `chat/think.rs`, `chat/attach.rs`, `chat/explain.rs` | The full send loop against stubbed AI, the stable system prompt, `<think>` filtering, freshness detection, attachment injection, selection clipping. |
| Voice | `stt/mod.rs`, `stt/engine.rs`, `stt/session.rs`, `tts/sentence_buffer.rs`, `tts/speech_text.rs`, `tts/voices.rs`, `audio/mod.rs`, `audio/devices.rs`, `dictation.rs` | Model resolution, the family-detection ladder, session lifecycle flags, sentence boundary detection (8 tests), Markdown-to-speech cleanup, per-language voice selection, the RMS/frequency spectrum (one `#[ignore]`d as a cost benchmark), loopback-device blacklisting, `LiveTyper` delta reconciliation. |
| Models | `models/catalog.rs`, `models/mod.rs`, `models/download.rs` | The `catalog_is_consistent` invariants, custom-folder detection, path-traversal rejection, the starter-set size cap, `safe_join` archive-entry validation. |
| MCP | `mcp/mod.rs`, `mcp/config.rs`, `mcp/permissions.rs`, `mcp/sources.rs` | Config validation, `mcp.json` parsing, classification + the permission policy table, URL extraction from tool text. |
| Desktop | `desktop/window.rs`, `desktop/shortcuts.rs`, `desktop/icon.rs` | The pure geometry math (Tauri-free), accelerator parsing and single-modifier rejection, icon rendering. |
| Persistence | `database/*.rs`, `settings/mod.rs`, `settings/backup.rs` | CRUD, FTS5 search, JSON/Markdown/plain-text export and import, `sanitize()` clamps, backup encryption round-trip. |
| Infra | `gpu/mod.rs`, `hardware/mod.rs`, `search/mod.rs` (one `#[ignore]`d), `attachments/*.rs` | Pack path layout, CUDA version compatibility, the search-result parsers, base64 and Office/text extraction. |

**Integration tests** are the 6 files in `src-tauri/tests/`. Each is its own binary,
which is why `profile.dev` sets `debug = "line-tables-only"` (see [§12](#12-known-gaps-and-gotchas)):

| File | Test fns | Hermetic? | What it proves |
| --- | --- | --- | --- |
| `mcp_integration.rs` | 3 | **Yes — always runs** | Spawns the real `mcp_fixture_server` over stdio and exercises the real `McpManager`: discovery, default permissions derived from category, safe-mode capping, `available_tools()` hiding execution tools, source extraction, tool errors, `mcp.json`-free failure paths, an unreachable HTTP server that must not report `connected`. |
| `db_smoke.rs` | 1 | Gated on `LA_DB` | Reads a **real on-disk** database the way the History panel does; an empty query must return the same count as `list_conversations`. |
| `voice_models.rs` | 9 | Gated on `LA_MODELS_DIR` / `LA_LIVE_LMSTUDIO` / `LA_MIC_TEST` | Real end-to-end voice: TTS→STT round-trips in en/de/ar, in-place Whisper language switching, one multilingual voice for three languages, a user-supplied model folder, and the hands-free and dictation pipelines driven by **synthesized** audio pushed through the real `CaptureEvent::Samples` channel. |
| `lmstudio_live.rs` | 1 | Gated on `LA_LIVE_LMSTUDIO=1` | Live chat against a running LM Studio: streaming in three languages with no tool use for general knowledge, and web-search tool use plus sources for a "latest" question. |
| `gpu_pack.rs` | 1 | **`#[ignore]`** + `LA_GPU_DIR` | Downloads the CUDA pack into a chosen folder and asserts **every** file in `gpu::REQUIRED` is present. |
| `web_search_live.rs` | 4 | **`#[ignore]`**, network | Real DuckDuckGo parsing, a SearXNG instance served under a sub-path that only answers browser-like HTML, a public instance from searx.space, and the exact `"Built-in search (DuckDuckGo)"` label when DuckDuckGo is primary. |

#### Every Rust test environment variable

| Variable | Required value | Gates |
| --- | --- | --- |
| `LA_DB` | path to an existing `assistant.db` | `db_smoke.rs` — the whole file. Unset ⇒ prints a skip message and returns `Ok`. |
| `LA_GPU_DIR` | destination folder | `gpu_pack.rs` — via a **hard `expect`** (the only test in the suite that hard-fails instead of skipping). Also `#[ignore]`d. |
| `LA_LIVE_LMSTUDIO` | **exactly the string `"1"`** | `lmstudio_live.rs` (the whole file) and `voice_models.rs::hands_free_conversation_speaks_the_answer`. Any other value — including `"true"` or `"0"` — leaves the tests skipped. |
| `LA_MODELS_DIR` | path to a folder containing the real model folders | 5 of the 9 `voice_models.rs` tests. |
| `LA_MULTILINGUAL_MODELS_DIR` | path to a folder containing **only** `supertonic-3-int8` | `voice_models.rs::multilingual_voice_speaks_three_languages` — proves one voice serves all three languages. |
| `LA_STREAM_MODEL` | path to a **streaming** STT model folder | `voice_models.rs::custom_model_folder_transcribes`. **Both** `LA_MODELS_DIR` **and** `LA_STREAM_MODEL` are required, so it can never half-run. |
| `LA_MIC_TEST` | **exactly the string `"1"`** | `voice_models.rs::microphone_delivers_audio_events` — a 2-second real-mic check. |
| `ENV.CARGO_BIN_EXE_mcp_fixture_server` | *(not set by you)* | A **compile-time** Cargo variable (`env!("CARGO_BIN_EXE_mcp_fixture_server")`), the absolute path to the freshly built fixture binary. It is what lets the MCP tests spawn a real server without installing anything. |
| `LOCAL_ASSISTANT_LOG` | `trace`/`debug`/`info`/… | Not a test gate: the `tracing` filter. Set it to `debug` to see the audio-capture diagnostics and the TTS thread timings. |
| `TAURI_DEV_HOST` | IP or hostname | Not a test gate: switches the Vite dev server and HMR off loopback. |

**Every gate is a soft skip** (`eprintln!(); return;`) except `LA_GPU_DIR`, which is a
hard `expect` and is additionally `#[ignore]`d. **A clean checkout therefore runs the
whole default suite green with no environment at all.**

#### Exact run commands

```bash
# Everything that runs by default (unit + hermetic integration).
cd src-tauri
cargo test -p local-ai-assistant

# Show the skip messages so you can see which optional suites were disabled.
cargo test -p local-ai-assistant -- --nocapture

# One integration binary.
cargo test -p local-ai-assistant --test mcp_integration -- --nocapture

# The network tests (all #[ignore]d).
cargo test -p local-ai-assistant --test web_search_live -- --ignored --nocapture

# The CUDA pack download (~1.65 GiB). Hard-fails if LA_GPU_DIR is unset.
set LA_GPU_DIR=C:\temp\gpu-pack
cargo test -p local-ai-assistant --test gpu_pack -- --ignored --nocapture

# Real voice models (~560 MB download). PowerShell:
$env:LA_MODELS_DIR = "C:\temp\models"
cargo test -p local-ai-assistant --test voice_models -- --nocapture

# One voice model for three languages.
$env:LA_MULTILINGUAL_MODELS_DIR = "C:\temp\models-multilingual"
cargo test -p local-ai-assistant --test voice_models multilingual_voice -- --nocapture

# A streaming model outside the app folder.
$env:LA_STREAM_MODEL = "C:\temp\models-streaming\parakeet"
cargo test -p local-ai-assistant --test voice_models custom_model_folder -- --nocapture

# The real microphone.
$env:LA_MIC_TEST = "1"
cargo test -p local-ai-assistant --test voice_models microphone_delivers -- --nocapture

# Live LM Studio (streaming, 3 languages, web-search tool use).
$env:LA_LIVE_LMSTUDIO = "1"
cargo test -p local-ai-assistant --test lmstudio_live -- --nocapture

# A real on-disk database.
$env:LA_DB = "$env:APPDATA\com.localassistant.app\assistant.db"
cargo test -p local-ai-assistant --test db_smoke -- --nocapture

# The #[ignore]d cost benchmark in services/audio/mod.rs.
cargo test -p local-ai-assistant --lib -- --ignored --nocapture
```

`--nocapture` is essential for the optional suites: their skip messages and progress
output go to the real stdout, which Cargo otherwise captures.

### 6.2 Frontend tests (Vitest)

Configuration lives in `vite.config.ts` under `test`:

```ts
test: { environment: "jsdom", globals: true, setupFiles: ["./src/test/setup.ts"], css: false }
```

- **`environment: "jsdom"`** — no browser. `css: false` means class-name assertions are
  pure DOM checks and never validate actual CSS.
- **`globals: true`** — `describe`/`it`/`expect` are ambient; `tsconfig.json` adds
  `vitest/globals` to `types`.
- **No `include` override**, so Vitest's default glob picks up every `*.test.ts(x)`.
- **No coverage configuration.**

**9 files in `src/test/`** (1 setup + 8 suites):

| File | Cases | Focus |
| --- | --- | --- |
| `setup.ts` | — | The global mocks (below). |
| `ui.test.tsx` | 18 | Text direction, `SpokenText`, `MessageBubble`, the chat store's full streaming lifecycle and barge-in/queue rules, `Composer`, `CallView`, shortcut accelerators, HTML-injection regression guard, and a **cross-language invariant** that reads `src-tauri/capabilities/default.json` off disk and asserts `global-shortcut:default` is present. |
| `settings.test.tsx` | 30 | An `it.each` over **all 13 settings sections** (a mount smoke test), the model manager, the model picker, the bubble, a web-search round-trip, `mockBackend()` — a 17-command `invoke` router that echoes `save_settings` back like the real backend. |
| `selection.test.tsx` | 5 | The right-click selection menu and the Explain pop-up. Re-implements `Channel` plumbing by hand because the mock `Channel` only provides `onmessage`. |
| `history.test.tsx` | 4 | Recency grouping, the empty state, delete resetting `conv_set_last({id:null})`, and **a failing `conv_list` must render the error, not the empty state**. |
| `shortcutinput.test.tsx` | 3 | Shortcut capture, waiting for the final modifier release, rejecting single-modifier combos. |
| `strings.test.ts` | 5 | i18n interpolation, error-code → message mapping, **every literal `t()` key in the source tree exists**, the dynamic key families (below), and MiB-based `formatBytes`. Also reads `src-tauri/src/services/chat/prompt.rs` from disk and asserts the Rust `DEFAULT_EXPRESSIVE_INSTRUCTION` equals `t("settings.voice.expressiveInstructionDefault")`. |
| `soundTags.test.ts` | 2 | `<laugh>`/`<sigh>`/`<breath>` stripping without touching `a < b`. |
| `voiceSessions.test.ts` | 2 | Two real races: a late `idle` from a previous session must not end the new one, and a session that ended before `voice_start` resolved stays ended. |

#### The Tauri module mocks (`src/test/setup.ts`)

Tauri APIs do not exist in jsdom, so `setup.ts` mocks **four modules** (there is no
`__TAURI__` global mock anywhere):

| Module | Mock | Consequence |
| --- | --- | --- |
| `@tauri-apps/api/core` | `{ invoke: vi.fn(async () => undefined), Channel }` where `Channel<T>` is a stub class with a single `onmessage` field | `invoke` resolves to `undefined`; each test overrides it, usually with a **command router** keyed by command name. |
| `@tauri-apps/api/event` | `{ listen: vi.fn(async () => () => {}) }` | `listen` resolves to a no-op unlisten. Tests that need real delivery extract the registered handler from `vi.mocked(listen).mock.calls`. |
| `@tauri-apps/plugin-opener` | `{ openUrl: vi.fn() }` | |
| `@tauri-apps/plugin-dialog` | `{ open: vi.fn(), save: vi.fn() }` | |

Plus one browser shim: `window.matchMedia` always returns `{ matches: false, … }` with a
no-op `addEventListener`. **Therefore `theme: "system"` always resolves to light in
tests, and the theme `change` listener never fires.**

#### Commands

```bash
npm test                 # vitest run  (the gate)
npm run test:watch       # vitest in watch mode

npx vitest run src/test/strings.test.ts     # one file
npx vitest run -t "queues typed messages"   # one case by name
npx vitest                                # watch mode
```

No environment variable gates any frontend test.

---

## 7. Debugging

### 7.1 Logs

| Property | Value |
| --- | --- |
| **Location** | `app.path().app_log_dir()` → on Windows `%APPDATA%\com.localassistant.app\logs\` |
| **Fallback** | `std::env::temp_dir().join("local-assistant-logs")` if the OS log dir cannot be resolved |
| **File name** | `local-assistant.YYYY-MM-DD.log` (daily rotation, prefix `local-assistant`, suffix `log`) |
| **Retention** | **7 days** (`max_log_files(7)` — the oldest files are deleted) |
| **Writer** | non-blocking, in-memory queue flushed by a background worker |
| **Ansi colours** | off (`with_ansi(false)`), so the files are plain text |
| **Targets** | on (`with_target(true)`) — every line names its Rust module |

Open the folder from **Settings → Apps → Diagnostics** (`ipc.openFolder("logs")`), or
read the tail in-app: `ipc.readLogTail(lines)` returns the last N lines of **today's**
file (default 200, hard cap 2000).

**Content rule.** Logs never contain conversation content by default
(`settings.general.logConversationContent`, off by default). Call sites log events,
error details and timings only — never message text. `PRIVACY.md` states this
explicitly.

**The one real footgun:** the non-blocking appender's `WorkerGuard` must live for the
whole process. `lib.rs` does `Box::leak(Box::new(guard))` in the `setup` closure for
exactly that reason. If you add another logging initialisation site, do the same.

### 7.2 `LOCAL_ASSISTANT_LOG`

The env-filter level. Anything `EnvFilter` accepts works:

```bat
set LOCAL_ASSISTANT_LOG=debug     :: verbose
set LOCAL_ASSISTANT_LOG=trace     :: everything, including the per-second capture ticks
set LOCAL_ASSISTANT_LOG=info      :: the default
```

- The default is **`info`**.
- It is parsed with **`EnvFilter::try_new`**, not `new`. **A malformed value never
  panics** — it falls back to `EnvFilter::new("info")`. So a typo gives you a normal log,
  not a crash.
- Only the **file** layer is filtered by it; the console layer in a debug build
  (below) shares the same registry filter.

### 7.3 The debug console layer

`logging.rs` adds an **extra stdout/stderr layer under `#[cfg(debug_assertions)]`**:

```rust
#[cfg(debug_assertions)]
registry = registry.with(console_layer);
```

So in `npm run tauri dev` you see log lines in the terminal **and** in the file. In a
release build (`windows_subsystem = "windows"`, no console) only the file exists. To get
the console in a release build, run the exe from a terminal with output redirected:
`local-ai-assistant.exe > log.txt 2>&1`.

### 7.4 The tray Diagnostics entry

The tray context menu (`desktop/tray.rs`) has 8 items:

| id | Label | Enabled | Action |
| --- | --- | --- | --- |
| `title` | Open Local Assistant | false (header) | — |
| `open` | Open | true | `window::show_main(app, true)` |
| `new` | New Conversation | true | show main, then `app://new-conversation` → `MAIN` |
| `voice` | Voice Mode | true | show main **without** focus steal, then `app://toggle-hands-free` → `MAIN` |
| `dictation` | Dictation | true | `shortcuts::toggle_dictation` |
| — | separator | | |
| `settings` | Settings | true | `open_settings(app, None)` |
| — | separator | | |
| `diagnostics` | Diagnostics | true | `open_settings(app, Some("diagnostics"))` |
| — | separator | | |
| `quit` | Quit | true | `tray::quit` |

**The Diagnostics item is the in-app console.** It opens the settings window on the
`diagnostics` section, which is the log viewer: it shows the tail of today's log via
`ipc.readLogTail`, the app version, the resolved paths, the hardware inventory, the
`scan_capabilities` report, the privacy status, the shortcut-registration errors and the
live model-inventory list. There is **no separate stdout/console subsystem item** — this
is the only in-app diagnostic surface, and it is reached from the tray or `Ctrl+,`.

The tray is configured with `show_menu_on_left_click(false)`, so a **left click toggles
the chat window** instead of opening the menu. The menu opens on right click.

### 7.5 "The window never appeared"

Two independent watchdogs exist, at different layers:

**(a) 3-second React watchdog — `src/main.tsx`.** While the app has not finished
booting, a 3000 ms timer flips a `stuck` flag and the `.boot` placeholder renders:

> Still loading “{route}”… backend not answering. Quit from tray, restart dev, retry.

The route in the message is the frozen hash route (`chat` / `settings` / `overlay` /
`bubble`). The source comment: *"Stuck detector: if boot takes >3s (hang, not crash),
say so visibly."* The `catch` handler always calls `setReady(true)`, so a boot that
**failed** shows the error screen with a Retry button rather than hanging on the
watchdog. This message means an `await` on a Tauri call never resolved — usually the
backend panicked during `init_state`, or a single-instance guard redirected the launch
to an already-running process.

**(b) 5-second document fallback — `index.html`.** A 500 ms `setInterval` un-hides
`#boot-fallback` ("The window did not finish loading." + a Reload button) when
`#root` has no children **and** 5 000 ms have elapsed, and re-hides it and clears the
interval the moment React mounts. It exists because "Never leave a silent blank window".
It fires when the **module never executes at all** — a JS parse error, a failed chunk
fetch, a CSP block. If you see (b) but not (a), look at the browser devtools console.

`index.html` also force-reloads on **F5** and **Ctrl/Cmd-R**, because Tauri webviews
swallow those keys.

### 7.6 `app://window-shown` pre-arm ordering

`desktop/window.rs::show_main` is the bubble→chat morph, and its ordering is
load-bearing:

1. Capture the bubble rect **before** hiding it (only if it exists and is visible).
2. If the chat was hidden and the position is not `"custom"`, heal the size and compute
   the target position.
3. Convert the bubble centre to **main-window logical pixels** for the zoom origin.
4. **Emit `app://window-shown` to `MAIN` while the webview is still hidden**, then
   `sleep(50 ms)`. This is the pre-arm: the first *visible* frame is already mid-zoom at
   the bubble's position. The source states the goal as *"never a frame with both bubble
   + full chat, never stale veil"*.
5. `set_position(target)` — a **snap**, not a resize loop, because "24 window-manager ops
   block, feel sluggish".
6. `hide_bubble` **then** `show`, in the same tick, so they are never both visible.
7. `set_focus()` and `keep_off_taskbar()`.

If the zoom origin is wrong, step 4's payload (`{ origin, animated, fx, fy }`) is the
thing to inspect. The reverse path, `minimize_to_bubble`, emits `app://window-closing`
with the bubble centre, sleeps **190 ms** (the CSS shrink duration) and only then hides —
remove that sleep and the shrink is cut in half.

### 7.7 The temporary red-background probe

`desktop/window.rs::open_settings` contains a **self-flagged temporary diagnostic**:
about 800 ms after opening the settings window it runs

```rust
win.eval("document.body.style.background='#ff0000'")?;
```

Reading the result:

| Result | Meaning |
| --- | --- |
| **red** | `app://navigate` arrived and JS is executing, but React never mounted |
| **white** | navigation did not happen at all |
| nothing | healthy — the probe is invisible |

It is safe to leave in, but it **must be removed before a release**. It is the fastest
way to bisect "the settings window opens blank".

### 7.8 Running the two diagnostic binaries standalone

**`gpu_probe.exe`** — proves the CUDA pack can actually *run* a model on this machine,
which the app's status flags cannot.

```bat
cd src-tauri
cargo build --bin gpu_probe
REM copy the exe into the GPU pack folder so it loads the same DLLs the app would
copy src-tauri\target\debug\gpu_probe.exe "%APPDATA%\com.localassistant.app\gpu\cuda-1.13.8\"
cd "%APPDATA%\com.localassistant.app\gpu\cuda-1.13.8"
gpu_probe.exe "..\..\models\supertonic-3-int8" cuda
```

| Argument | Meaning |
| --- | --- |
| `argv[1]` (required) | the TTS model directory |
| `argv[2]` (default `cuda`) | the ONNX Runtime provider |

It runs **three warm-up iterations** — "the first one pays for kernel warm-up, the later
ones show the speed the app would actually see". Set `LA_STT_DIR` to also benchmark a
speech model in the same run; without it the program returns after the TTS section. It
has no Tauri, no app crate and no `AppError`, precisely so it can be copied into the pack
folder; it is `expect`-heavy, which is appropriate for a diagnostic.

**`mcp_fixture_server.exe`** — the stdio JSON-RPC/MCP server the integration tests use.
Useful by hand to reproduce an MCP transport problem without running the app:

```bat
cd src-tauri
cargo build --bin mcp_fixture_server
mcp_fixture_server
```

It speaks line-delimited JSON on stdin/stdout, answers only messages that carry an
`id` (notifications get no reply), skips malformed lines, and reports protocol version
`2025-06-18` as server `fixture` v`1.0.0`. It exposes four fake tools — `web_search`
(read-only hint, returns two `php.net` URLs to exercise source extraction), `write_file`,
`run_command` (listed but with **no** `call` arm) and `fail` (returns `isError: true`).
Unknown methods get JSON-RPC `-32601`, unknown tools `-32602`. In PowerShell, pipe JSON
in to see it work:

```powershell
'{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | .\target\debug\mcp_fixture_server.exe
```

---

## 8. Extension guide

### 8.1 Add a Tauri command, end to end

Five files, in this order. The mechanical rule that makes this tractable:
**`src/app/ipc.ts` is the only frontend module that may name a Rust command.**

**Step 1 — implement it.** In the right module under `src-tauri/src/commands/`
(`app`, `attachments`, `chat`, `mcp`, `memory`, `voice`):

```rust
use super::CmdResult;                       // = Result<T, AppError>
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn my_command(state: State<'_, AppState>, id: String, limit: Option<u32>) -> CmdResult<u32> {
    let n: u32 = state.db.count(&id)?;      // `?` converts into AppError
    Ok(n.min(limit.unwrap_or(100)))
}
```

Rules:
- Return `CmdResult<T>` (`Result<T, AppError>`) for anything fallible. Because
  `AppError` implements `Serialize`, a failure becomes `{code, detail}` on the wire.
- A command that genuinely cannot fail returns `T` directly (there are many: window
  control, `chat_stop`, `tts_stop`). Pick `async fn` only if the body awaits; heavy
  blocking work goes through `tokio::task::spawn_blocking`.
- `#[serde(rename_all = "camelCase")]` on any struct that crosses the boundary.
- Add a `///` doc comment when the semantics are not obvious from the signature —
  the codebase documents *why*, not *what*.

**Step 2 — register it.** In `src-tauri/src/lib.rs`, inside
`tauri::generate_handler![ … ]`. The list has exactly 93 entries and must stay in sync
1:1 with the `#[tauri::command]` functions — a `strings.test.ts`-style cross-check is
worth considering, but today the count is maintained by hand.

```rust
.invoke_handler(tauri::generate_handler![
    commands::app::get_settings,
    …
    commands::chat::my_command,      // keep the grouping, do not alphabetise
])
```

**Step 3 — add the typed wrapper.** In `src/app/ipc.ts`, in the matching group
comment block. camelCase keys on the wire, matching `ipc.ts` exactly:

```ts
// conversations
myCommand: (id: string, limit?: number) => invoke<number>("my_command", { id, limit }),
```

**Step 4 — add the TypeScript types.** In `src/app/types.ts`. Use a **required property
plus `| null`** for "absent in the data", not `?` — that is the prevailing convention
and only seven `?` properties exist in the whole file.

```ts
export interface MyThing { id: string; label: string; detail: string | null }
```

**Step 5 — add the i18n string.** In `src/app/strings.en.json` under the right family.
`src/test/strings.test.ts` walks every non-test `.ts`/`.tsx` file with
`/\bt\(\s*"([a-zA-Z0-9_.]+)"/g` and fails on any literal key that is missing. It also
enumerates the **dynamic** families, which are built at runtime and therefore invisible
to that scan — extend the list when you add one:

| Family | Count | Rule |
| --- | --- | --- |
| `settings.ai.reasons.*` | 11 | One per `ModelSelection::reasons` string. |
| `settings.mcp.category.*` | 6 | One per `ToolCategory` variant: `search`, `fetch`, `read`, `write`, `execute`, `other`. **Adding a `ToolCategory` variant means adding a string here.** |
| `settings.sections.*` | 14 | One per settings section id. |
| `errors.*` | 7 asserted | One per `AppError::code()`. `strings.test.ts` checks 7 of the 16; `errorMessage()` falls back to `errors.other` for anything unrecognised, so adding a code without a string degrades gracefully. |

**Then**: update [`api_reference.md`](./api_reference.md) (the AGENTS.md specs-sync
mandate requires it for any `CoreCommand`/command change), update
`full_documentation.md`, `catalog.txt`, `relationships.txt`, and append a line to
`runtime.md` under `## Log`. If the command is not silent, decide its error tier — see
[§7 Error-handling conventions](./api_reference.md#7-error-handling-conventions).

### 8.2 Add an MCP tool category

The policy lives in `src-tauri/src/services/mcp/permissions.rs`, with the category enum
itself in `src-tauri/src/services/chat/tools.rs`.

`classify(name, description, read_only_hint, destructive_hint)` tokenises the camelCase
tool name into words and matches them against five word lists, **in this order**
(first match wins):

| Order | Constant | Category |
| --- | --- | --- |
| 1 | `EXEC` — `exec, execute, run, shell, command, cmd, terminal, powershell, bash, script, eval, spawn, kill, process, sudo, install` | `Execute` |
| 2 | `destructive_hint == Some(true)`, or `WRITE` — `write, create, delete, remove, update, edit, move, rename, upload, send, post, push, commit, set, modify, insert, drop, patch, append, save, merge, close, archive, publish, mkdir, rm, put` | `Write` |
| 3 | `SEARCH` — `search, query, lookup, google, bing, duckduckgo, searxng, brave, tavily, websearch` | `Search` |
| 4 | `FETCH` — `fetch, scrape, crawl, browse, navigate, url, webpage, page, extract, download` | `Fetch` |
| 5 | `web, internet, news` | `Search` |
| 6 | `read_only_hint == Some(true)`, or `READ` — `read, get, list, find, view, show, describe, stat, info, status, open, load, tree, cat` | `Read` |
| 7 | description contains `search the web` / `web search` / `search engine` | `Search` |
| 8 | description contains `execute` / `shell command` / `run a command` | `Execute` |
| 9 | — | `Other` |

Word matching is `has_word`: split on non-alphanumerics, then exact match, or a
prefix match when the list word is longer than 4 characters. So `writeFile` and
`write_file` both hit `WRITE`, and `installSomething` hits `EXEC`.

Adding a category therefore means:

1. **Add the variant** to `ToolCategory` in `services/chat/tools.rs` (it is
   `#[serde(rename_all = "camelCase")]`, so the wire name is lowerCamel).
2. **Add the words** to the relevant `const` list in `classify` — or add a new list and
   a new ordered branch, remembering that order *is* the precedence.
3. **Update `default_permission`** in `permissions.rs`:
   `Search | Fetch | Read → Allow`, `Write | Other → Ask`, `Execute → Deny`. A new
   variant must be given an explicit arm here.
4. **Update `ToolCategory::is_sensitive`** in `tools.rs` — currently
   `Write | Execute | Other`. Sensitive categories are the ones safe mode caps at
   `Ask`.
5. **Check `clamp_permission`** — it needs no change; it is expressed in terms of
   `is_sensitive()`, so a new variant is covered automatically once step 4 is right.
   This is the design intent: `clamp_permission(category, requested)` returns `Ask`
   whenever `requested == Allow && category.is_sensitive()`.
6. **Add the i18n string** `settings.mcp.category.<variant>` to `strings.en.json`, and
   extend the `ToolCategory` list in the `strings.test.ts` "covers dynamic key
   families" case.
7. **Add unit tests** in `permissions.rs`'s `mod tests`: one `classify` case per word
   that should trip the new rule, and one `default_permission` / `clamp_permission`
   assertion.
8. **Update the spec** (`full_documentation.md` §File Analysis for `permissions.rs` and
   `tools.rs`, plus `api_reference.md` §Security and the `ToolCategory` row in the
   command tables).

**Ordering matters and is a security property.** `Execute` is checked first, so
`run_shell` can never be downgraded to `Write` by the `WRITE` list. If you add a word
to a lower-priority list that also appears in a higher-priority one, you change the
classification of existing tools.

### 8.3 Add a voice model

Three coordinated edits plus one invariant test.

**Step 1 — add a `CatalogModel` entry** in
`src-tauri/src/services/models/catalog.rs`. A catalogue entry is fully static data
with **pinned** download URLs and **pinned SHA-256 digests**:

```rust
CatalogModel {
    id: "whisper-base",
    kind: ModelKind::Stt,
    engine: Engine::Whisper,
    name: "Whisper base (int8)",
    languages: &["*"],
    files: &[
        RemoteFile { url: concat!(HF, "/sherpa-onnx-whisper-base/resolve/main/encoder.int8.onnx"),
                     sha256: Some("…64 hex chars…"), dest: "encoder.int8.onnx",
                     archive: false, size_bytes: 77_457_322 },
        RemoteFile { url: …, sha256: Some("…"), dest: "decoder.int8.onnx", archive: false, size_bytes: 138_426_216 },
        RemoteFile { url: …, sha256: Some("…"), dest: "tokens.txt",          archive: false, size_bytes: 816_730 },
    ],
    quality: 2,
    min_ram_gb: 2,
    license: "MIT",
    gender: "",
}
```

- `url` **must** start with `https://` (asserted). Use the compile-time
  `const_format_concat!` macro local to the file — it concatenates at compile time and
  adds no dependency.
- `sha256` **must** be exactly 64 hex characters when present. **Never omit it for a
  remote file.** `AGENTS.md` §3 requires SHA-256 verification on every download, and
  `download.rs` compares with `eq_ignore_ascii_case`; a mismatch is
  `AppError::Download("checksum mismatch for {url}")`.
- `size_bytes` must be the **real** archive size — the UI shows download totals and the
  build script checks them.
- `languages` uses `"*"` for multilingual. For a TTS model the catalogue test requires
  that `en`, `ar` and `de` are each covered by some TTS model (or a `"*"` one).
- Adding the id to `PROTECTED_MODELS` in `services/models/mod.rs` makes it undeletable
  from the UI — reserve that for genuinely built-in models.

**Step 2 — teach discovery about it (only for a new *engine*).** If the model is a
sherpa-onnx format the app already knows (Whisper, Parakeet/Nemo, SenseVoice, Moonshine,
Kokoro, Piper, Kitten, VITS, Supertonic, Silero), nothing more is needed. If it is a new
layout, add a branch to `detect_custom` in `src-tauri/src/services/models/mod.rs`, which
is an **ordered** recognition chain:

1. `silero_vad*` / `ten-vad*` → `Vad`
2. `tts.json` + `unicode_indexer.bin` + `voice.bin` → Supertonic, languages `["*"]`
3. `voices.bin` + `tokens.txt` → Kitten or Kokoro (told apart by a `kitten` path hint;
   language `ar`/`nabra`/`-ar` → `ar`, `multi-lang` → `"*"`, else `en`)
4. `tokens.txt` + (`espeak-ng-data` or `lexicon.txt`) → Piper (language from the first
   two characters of the first `.onnx` when `file[2] == '_'`)
5. otherwise `engine::detect` → STT

The **first** matching branch wins, so a new branch must be inserted before the generic
`engine::detect` fallback, and the same `detect_custom` ladder must be reflected in
`stt/engine.rs::SttFamily` (ordered: Moonshine → Transducer → Whisper → single-file)
if the model is a recogniser.

**Step 3 — the invariants.** `catalog_is_consistent` in `catalog.rs` must still hold.
It asserts:

- no duplicate ids;
- every model has at least one file;
- every URL starts with `https://`;
- every present SHA-256 is exactly 64 hex characters;
- for each of `en`, `ar`, `de`, at least one TTS model covers that code or `"*"`;
- `silero-vad` exists in the catalogue;
- `whisper-small`'s first URL equals the fully expanded `HF` + path string, proving the
  const-concat macro works.

A separate test in `models/mod.rs` asserts the **recommended starter set stays under
550 000 000 bytes**. If your entry is in `recommend()` (or you change it), update that
number deliberately, not accidentally.

**Step 4 — spec.** Update `full_documentation.md` (the catalogue table, the digest table
and the totals), `data_models.txt`, and the Models section of `api_reference.md`.

### 8.4 Add a settings key

Settings are **one JSON document** stored in SQLite under the key `app_settings`, and
every field has a serde default so documents written by older versions keep loading.
Adding a key is therefore backward-compatible by construction.

**Step 1 — the Rust field and its default.** In `src-tauri/src/settings/mod.rs`, in the
right section struct (`GeneralSettings`, `AiSettings`, `LanguageSettings`,
`SttSettings`, `TtsSettings`, `DictationSettings`, `SearchSettings`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SttSettings {
    …
    /// Runs the recogniser on this many threads. 0 = auto.
    pub recognizer_threads: u32,
}

impl Default for SttSettings {
    fn default() -> Self {
        Self { …, recognizer_threads: 0 }
    }
}
```

Three rules that are not negotiable:
- The struct carries `#[serde(rename_all = "camelCase", default)]`, so snake_case fields
  serialise as camelCase and a missing key falls back to `Default::default()`.
- The field name is the **wire name in camelCase**. Pick it now; renaming later is a
  migration.
- `#[derive(PartialEq)]` is what lets `save_settings` diff old against new and fire only
  the side effects that actually changed.

**Step 2 — the `sanitize()` clamp.** `Settings::sanitize(&mut self)` runs on **every**
save, before persistence. Any numeric range, enum value, URL shape, accent name or
shortcut string must be clamped or replaced there. It is the trust boundary for
`settings_import` and for any hand-edited settings file. If the field can hold a bounded
set of strings, add a `matches!(…) else default` arm — that is how `ai.provider`,
`general.accent` and the shortcut modifier tokens are already handled. The
`single-modifier shortcut → safe default` rule lives here too.

**Step 3 — the TypeScript mirror.** In `src/app/types.ts`, inside the matching
sub-object of `Settings`, with the same camelCase name and the same default value. This
is a hand-maintained mirror, not a generated one, so a mismatch is a real bug: the
`settingsStore.update()` optimistic path writes your value, the backend returns its
sanitised version, and the store **adopts the server's document as authoritative** — so
a missing or wrongly-typed mirror field visibly reverts.

**Step 4 — the UI row.** In the matching section component under
`src/pages/Settings/sections/`, using the shared primitives from
`src/components/settings/layout.tsx` (`Row`, `Card`, switch/select/range controls).
Every control must:
- be controlled by `useSettings` state and commit through `useSettings.getState().update(...)`,
  never a local copy that can drift;
- have a label and, where the effect is not obvious, help text;
- use the design tokens from `src/styles/tokens.css` — **never** a literal colour;
- add a `t()` key to `strings.en.json`.

**Step 5 — if it needs a side effect, wire it into `save_settings`.**
`commands/app.rs::save_settings` is a diff-driven table; add a branch guarded by the
actual change, mirroring the existing style:

```rust
if before.stt.recognizer_threads != saved.stt.recognizer_threads {
    state.stt.apply_thread_budget(saved.stt.recognizer_threads);
}
```

**Step 6 — spec.** `full_documentation.md` (the settings section), `data_models.txt`, and
`api_reference.md` (the `Settings` payload). The `strings.test.ts` "every literal `t()`
key" case will fail if you forget the string.

---

## 9. Style rules

These are the conventions the code actually follows. They are not aspirational — a
change that violates them is out of style even if it works.

### 9.1 Rust

| Rule | Detail |
| --- | --- |
| **Errors are `thiserror`** | `#[derive(Debug, thiserror::Error)]` on every error enum, with a `#[error("…")]` message per variant. |
| **`AppError` has a stable `code()`** | The `code()` match is the IPC contract with the UI and with `strings.en.json`'s `errors.*` family. **Renaming a code is a breaking change**; add a variant instead. `detail` is the `Display` string, not the raw payload. |
| **Poison-tolerant lock acquisition, everywhere** | `.lock().unwrap_or_else(\|p\| p.into_inner())`. There is no `.lock().unwrap()` in the crate. A panicking audio or ONNX thread must not make the whole app unwrapable. |
| **`#[serde(rename_all = "camelCase")]`** | On every struct that crosses the IPC boundary, and on internally-tagged enums with `rename_all_fields = "camelCase"`. Rust is snake_case internally; only the wire is camelCase. |
| **No `unwrap()` / `expect()` in non-test code** | Errors propagate with `?` into `AppError`. The two deliberate exceptions: `lib.rs`'s `.expect("error while building Open Local Assistant")` (a build failure must be loud) and the standalone `gpu_probe` diagnostic binary, which is `expect`-heavy on purpose. |
| **Blocking work leaves the async runtime** | Native ONNX calls, cpal, filesystem IO and text insertion all run under `tokio::task::spawn_blocking` or on a named thread. |
| **One named OS thread per long-lived subsystem** | `mic-capture`, `audio-playback`, `tts-synth`, `tts-progress`, plus one per listening session and one for the Windows modifier watcher. Named so they are identifiable in a debugger. |
| **Cosmetics never fail the app** | Window operations use `let _ = win.set_*()`. Only constructors surface a `tauri::Result`. |
| **The system prompt is stable** | The clock, the per-message language and the freshness hint go into the **latest user message**, never into the system prompt, so LM Studio's prompt cache stays valid. |
| **Comments explain *why*** | A short comment above a non-obvious constant or branch, with the reason and often the cost. Avoid restating the code. |
| **Doc comments on non-obvious commands** | `///` on `#[tauri::command]` functions whose contract the signature does not convey. |

### 9.2 CSS

| Rule | Detail |
| --- | --- |
| **Design tokens live in `tokens.css` only** | That file declares custom properties and contains **zero** class or element selectors. No literal colour, radius, spacing or duration in `base.css`, `chat.css` or `settings.css` — except three documented fallbacks (`var(--card-hover, var(--surface-2))` and `#e5484d`/`#fff` on `.icon-btn.danger:hover`). Derive new colours with `color-mix(in srgb, …)` so they track the runtime accent. |
| **Flat kebab-case class names, compound modifier classes, no BEM** | `.btn` + `.btn-primary` / `.btn-danger` / `.btn-ghost` / `.btn-sm`; `.icon-btn` + `.active` / `.danger`; `.dot` + `.ok` / `.warn` / `.err` / `.busy`; `.badge` + `.accent` / `.warn` / `.err` / `.ok`. The modifier is a **second class**, not a `__element` chain. |
| **State is expressed with ARIA, not classes, where the element has a role** | Switch and segmented state use `[aria-checked="true"]`; nav items use `[aria-current="page"]`; search results use `[aria-selected="true"]`. |
| **`prefers-reduced-motion` is mandatory** | `base.css` has the global kill switch (`animation-duration`/`transition-duration: 1ms !important` on `*`); `chat.css` re-declares it. Any new animation must be disabled there. |
| **Logical CSS properties for RTL** | `inset-inline-start`, `margin-inline`, `padding-inline`, `text-align: start`. Mirroring is done with `[dir="rtl"]` / `:dir(rtl)` selectors and logical properties — never with a mirrored left/right rule. Asymmetric radii are flipped explicitly. The `.switch` knob transitions `inset-inline-start` so it is RTL-safe with no extra rule. |
| **Performance-conscious animation: `transform` and `opacity` only** | Animate compositor-only properties. The thinking indicator animates only `opacity`/`scale` with `will-change`. **The streaming header uses no `backdrop-filter`** — the source comment records why: "it re-blurs the scrolling list on every streamed batch". The per-word highlight has deliberately **no** text-shadow and **no** scale transform, "it smears the joined Arabic letterforms and would reflow the paragraph on every word". |
| **No breakpoints except one** | `settings.css` owns the single `@media (max-width: 700px)` in the whole frontend (below it the sidebar hides and rows stack). Everything else is intrinsically responsive: `minmax(0, 1fr)`, `min-width: 0`, `vw`-clamped popovers, line clamps. |
| **Layout choice is deliberate and commented** | `.msg.assistant` is CSS Grid, "so long words can never squeeze the text column". Do not "simplify" it to flex. |
| **A mask, not a second gradient, for masked scroll** | `.spoken` fades with `mask-image: linear-gradient(...)` and hides its scrollbar. |

### 9.3 TypeScript and React

| Rule | Detail |
| --- | --- |
| **`strict: true`, nothing weaker** | `tsconfig.json` enables `strict` only (no `noUncheckedIndexedAccess`, no `exactOptionalPropertyTypes`), plus `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`. |
| **Required plus `\| null`, not `?`** | "Absent in the data" is `T \| null`. Only seven optional properties exist in `types.ts`; do not add more. `number \| null` universally means "measure unknown / not loaded / unset". `apiKey` is never `""` on the wire. |
| **`ipc.ts` is the only Tauri surface** | No component may `import { invoke } from "@tauri-apps/api/core"`. This is what makes the test suite possible. |
| **No wrapper in `ipc.ts` catches** | Every `invoke` rejects and every **caller** decides its policy. That is deliberate: it is what lets the stores implement three different error tiers. |
| **Stores adopt the backend's answer** | Optimistic updates apply first, then the **server's** returned document replaces local state, and the theme is re-applied. A local value is never authoritative once the round trip completes. |
| **React 19 StrictMode-safe effects** | Listeners register **once per window and are never torn down** (`voiceStore` uses a module-level `subscription` promise as the idempotency key). `main.tsx` uses a `cancelled` flag plus a `track(un)` helper so a double-invoked effect cannot leak or double-unlisten. |
| **Streaming is batched** | `chatStore` coalesces `delta`/`reasoning` into a buffer applied at most every 50 ms, because rendering each token re-parses the whole Markdown message. Non-text events flush first so ordering is preserved. |
| **No raw HTML** | Assistant Markdown goes through `react-markdown`; there is a regression test asserting no `<b>`/`<img>` element is ever produced. |
| **Every user-visible string is a `t()` key** | Hard-coded UI text fails `strings.test.ts`. |

---

## 10. Contribution guide

`AGENTS.md` at the repository root is **binding** and takes precedence over model
defaults. Summarised:

### 10.1 The specs-sync mandate

`specs/` is the single source of truth for architecture, modules, data models and API.
**Any code change must update the corresponding `specs/` files in the same commit or
PR.**

| Change type | Files that must change |
| --- | --- |
| **Rust** (`src-tauri/`) | `specs/full_documentation.md`; `specs/architecture_overview.md` **if layers change**; `specs/catalog.txt`; `specs/relationships.txt`; `specs/data_models.txt` **if types change**; `specs/api_reference.md` **if commands/IPC change**; the relevant `specs/diagrams/*.mmd`. |
| **UI** (note: the real path is **`src/`**, not `ui/src/`) | `specs/full_documentation.md` (File Analysis); `specs/catalog.txt`; `specs/relationships.txt`; `specs/api_reference.md` **if IPC commands change**. |
| **Config / manifests / catalogues / plugins / quick-apps** | `specs/data_models.txt`; `specs/full_documentation.md` (Business Rules / Data Models). |

Plus, every time you update specs:

- append a log line to `specs/runtime.md` under `## Log`, and
- keep **`Files Processed` equal to `Files to Process`** in `runtime.md`.

**If no spec update is needed, say why explicitly in the PR description** (for example
"typo-only, no behaviour change"). A PR with a behaviour change and no spec update
**MUST be rejected in review**.

### 10.2 Safety rules

- **Errors shown to users must say what went wrong, why, and how to fix it.** The
  `Diagnostic { problem, cause, fix }` shape is the model. The best example in the
  codebase is `desktop/verify.rs`:
  *"Windows Hello is not set up on this device. Set up a PIN in Windows Settings ›
  Accounts › Sign-in options."* **Known divergence:** the IPC wire format is
  `{code, detail}` (see `errors.rs`), not that triple. The `detail` string should still
  read as a diagnosis, and `AGENTS.md` §3 is the authority to point at in review.
- **Anything that downloads a file must verify SHA-256.** Every catalogue entry carries
  a digest; `download.rs` compares it case-insensitively and refuses on mismatch. No
  exceptions, including the optional GPU pack.
- **Secrets stay in the OS keyring; never in logs, files or bundles.** API keys are
  stored in Windows Credential Manager, never in the settings JSON, and
  `settings_export` strips them unless `include_keys` is explicitly set.
  `mcp_import_preview` returns **`env_keys` only — the key names, never the values**,
  so an LM Studio `mcp.json` credential can never be rendered on screen or logged.
- **Never log conversation content.** Off by default
  (`general.logConversationContent`). `PRIVACY.md` promises it.
- **Path traversal is a hard boundary.** `models::delete` rejects ids containing `/`,
  `\`, `..` and requires the resolved path to stay inside the models directory;
  `download::safe_join` rejects every `Component` that is not `Normal` or `CurDir`.
- **MCP servers are never installed or enabled automatically.** New and imported
  servers are always stored with `enabled = false`, and `mcp_save` on an existing server
  **ignores the incoming `enabled`** and re-applies the stored value. Command-execution
  tools are `Deny` by default and can never reach the model.

### 10.3 Commits and pull requests

- **Conventional Commits:** `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `build:`,
  `chore:`. The release commit message lives in `commit-message.txt` (git-ignored,
  rewritten per release) and is applied by `upload-release.bat`.
- **Before opening a PR**, run the corrected gate list in
  [§5.2](#52-the-corrected-gate-list) — all six commands.
- **Open an issue first for anything large**, so the approach can be agreed before the
  work.
- **In the PR description**: what you changed, why, which specs you updated (or why
  none were needed), and the gate output.

---

## 11. Packaging and release

### 11.1 `build-installer.bat` — the single build entry point

Used identically by developers and by CI. Four modes:

| Invocation | Behaviour |
| --- | --- |
| `build-installer.bat` | Build the frontend + release exe + setup exe, then **offer** to publish as GitHub release `v<version>`. |
| `build-installer.bat upload` | Same, but **upload without asking**. |
| `build-installer.bat noupload` | Same, but **never ask**. **This is what CI uses.** |
| `build-installer.bat install` | **Skip the installer entirely** and copy the app straight into `C:\Program Files\Open Local Assistant` (elevated). Copies *all* matching `sherpa-onnx|onnxruntime` DLLs, `icon.ico`, and creates a Start-menu shortcut. Kills any running instance first. |

Phases:

0. **Preflight** — `where cargo`; measure `src-tauri\target` **and**
   `src-tauri\target-test` with inline PowerShell. Over `CACHE_LIMIT_GB` (default **8**,
   overridable) it runs `cargo clean` for both. The reason is in the source: *"Cargo
   never deletes old build output, so `target\` and `target-test\` only grow (**they
   reached 146 GB once**)."*
1. **Frontend** — `npm install` if `node_modules` is missing, then `npm run build`.
   Post-condition: `dist\index.html` must exist. Also probes the sherpa-onnx cache
   ([§2](#2-setup)).
2. **Rust release** — `cargo build --release` (default `NUMBER_OF_PROCESSORS / 2` jobs, or
   `BUILD_JOBS` if set). Post-condition: the exe must exist. Then the archive is copied
   into `.build-cache`, and the **portable output** is staged in `release\`.
3. **Find Inno Setup** — four known locations, then `where iscc`.
4. **Compile the installer** — `ISCC /Q /DAppVersion /DSourceExe /DLibDir /DOutputDir
   installer\open-local-assistant.iss`.

**Version** is parsed from the `version` line of `src-tauri/Cargo.toml`, with a
**stale `0.1.0` fallback** — keep `Cargo.toml` authoritative.
**`DIST` is `release\`, never `dist\`** (`dist\` is the Tauri frontend folder and is
embedded into the exe as-is).

**Missing Inno Setup is a warning, not a failure**: it prints
`[!] Inno Setup 7 not found, so no setup file was made.`, points at jrsoftware.org and
at `build-installer.bat install`, and exits **0**.

### 11.2 The `release/` layout — 5 portable files + 1 installer

```
release\
  Open Local Assistant.exe                          ← renamed from local-ai-assistant.exe
  sherpa-onnx-c-api.dll
  sherpa-onnx-cxx-api.dll
  onnxruntime.dll
  onnxruntime_providers_shared.dll
  Open-Local-Assistant-<version>-setup.exe           ← built by Inno Setup
```

The **five** portable files are the exact set `upload-release.bat` and the CI check
require. **Keep the four DLLs next to the `.exe`** — the portable version will not start
without them.

`tauri.conf.json` declares `bundle.targets: ["nsis", "msi"]`, but **nothing in the
repository invokes `tauri build` or cargo-bundler**. The Inno Setup script is the real
installer. The `.iss` is configured for a per-machine install: `DefaultDirName
={autopf}\Open Local Assistant`, `PrivilegesRequired=admin`,
`ArchitecturesAllowed=x64compatible`, `Compression=lzma2/max`, Start-menu and optional
desktop icons, and a post-install "Launch Open Local Assistant" entry.

### 11.3 `upload-release.bat` — publishing (maintainers)

```bat
upload-release.bat            REM tag derived from the newest setup exe
upload-release.bat v1.0.0     REM explicit tag
```

Sequence:

1. Verify `gh` and `git` are on `PATH`; verify the setup exe and **all five** portable
   files exist.
2. **Commit and push first**, then force-update `refs/tags/<tag>` to the pushed
  `HEAD`. This also refreshes GitHub's source-code archives for an existing
  release. If the working tree is dirty, `commit-message.txt` is **required**;
  otherwise `git add -A` + `git commit -F commit-message.txt` + `git push`.
3. Zip the five portable files (staged in a temp directory first — "only the portable
   files go in the zip, not the setup exe next to them") into
   `%TEMP%\Open-Local-Assistant-<version>-portable-win-x64.zip`.
4. `gh release create <tag> --notes-file release-notes\<tag>.md` if that file exists,
   otherwise `--generate-notes`. An **existing** release keeps its notes **unless** the
   file exists, but its assets are replaced.
5. `gh release upload <tag> <setup> <zip> --clobber`.

The maintainer checklist from `README.md`: write `release-notes\v<version>.md`, write
`commit-message.txt`, run `upload-release.bat`.

### 11.4 CI — one job

`.github/workflows/build.yml`, name **Build Windows release**:

- **Triggers:** `workflow_dispatch` and `push` on tags `v*`.
- **Permissions:** `contents: read` — the workflow never writes to the repository and
  **never creates a GitHub Release**.
- **Exactly one job**, `build`, on `windows-latest`, `timeout-minutes: 90`. No matrix,
  no separate lint/test/release job.

| # | Step | Detail |
| --- | --- | --- |
| 1 | `actions/checkout@v4` | |
| 2 | `actions/setup-node@v4` | Node **22**, npm cache |
| 3 | `dtolnay/rust-toolchain@stable` | |
| 4 | `Swatinem/rust-cache@v2` | `workspaces: src-tauri` |
| 5 | Install Inno Setup | `choco install innosetup`, guarded on `Test-Path "%{ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"` |
| 6 | `npm ci` | repo root |
| 7 | `set BUILD_JOBS=%NUMBER_OF_PROCESSORS%` then `call build-installer.bat noupload < NUL` | `cmd`; `< NUL` neutralises the script's trailing `pause` |
| 8 | PowerShell output check | requires the five portable files **and at least one** `*-setup.exe`; then prints `release\` as a table |
| 9 | `actions/upload-artifact@v4` | `name: open-local-assistant-windows`, `path: release/`, `if-no-files-found: error` |

The workflow exists so release files are traceable to the source they came from (needed
for SignPath code signing) and so anyone can reproduce a build.

---

## 12. Known gaps and gotchas

Ordered by consequence.

### 12.1 The Inno Setup 6 vs 7 mismatch in CI — the installer is silently missing

`.github/workflows/build.yml` installs and checks for **Inno Setup 6**:

```pwsh
if (-not (Test-Path "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe")) {
  choco install innosetup --no-progress -y
}
```

But `build-installer.bat` searches for **Inno Setup 7** in four locations
(`C:\Program Files\Inno Setup 7\ISCC.exe`, `%ProgramFiles(x86)%\...`,
`%ProgramFiles%\...`, `%LocalAppData%\Programs\Inno Setup 7\ISCC.exe`) and then falls
back to `where iscc`. `installer/open-local-assistant.iss`'s own header comment says
"**Inno Setup 6** script".

If Chocolatey installs IS 6, the batch's search fails. It then prints
`[!] Inno Setup 7 not found, so no setup file was made.` and jumps to `:done` with
**exit code 0** — a warning, not a failure. Step 8 then runs:

```pwsh
$files += (Get-ChildItem release\Open-Local-Assistant-*-setup.exe).FullName
```

With no setup exe, `Get-ChildItem` returns **nothing**, so `$files` still contains only
the five hard-coded paths. All five exist, the loop does not throw, and **the check
passes with no installer in the artifact.** `upload-artifact` then uploads a
`release/` directory containing only the portable files.

**Fix:** either install Inno Setup 7 in the workflow
(`choco install innosetup --version=7.*`) and change the guard path to
`Inno Setup 7\ISCC.exe`, or make `build-installer.bat` also accept IS 6. The
independent check in step 8 should also **assert** the setup exe count is exactly 1,
not merely `Test-Path` each name it found.

### 12.2 The `window.rs` red-background probe

`desktop/window.rs::open_settings` contains a **self-flagged temporary** debug probe
(see [§7.7](#77-the-temporary-red-background-probe)): 800 ms after opening the settings
window it evaluates
`document.body.style.background='#ff0000'`. It is invisible when healthy, but it is
debug scaffolding in a shipping path and must be removed before a release.

### 12.3 `git add -A` in `upload-release.bat`

`upload-release.bat` runs `git add -A` with the maintainer's own `commit-message.txt`
whenever the working tree is dirty. There is **no path allow-listing and no secrets
scan**. The intent is that only build output is present (the file is git-ignored, and
`.gitignore` covers `release/`, `dist/`, `node_modules/` and `.build-cache/`), but
anything else left in the working tree — a scratch script, a local config containing a
token — is committed with that message. Review `git status` before running it.

### 12.4 No test or lint steps in CI

The workflow builds and verifies output files. It never runs `cargo fmt --check`,
`cargo clippy`, `cargo test`, `npm run typecheck` or `npm test`. **The quality gates
in [`AGENTS.md`](#51-agentsmd-2-is-stale) §2 are local-only**, and §2 itself names
packages and a directory that do not exist (see [§5.1](#51-agentsmd-2-is-stale)). A
green CI run therefore says nothing about correctness or style.

### 12.5 `target/` reached 146 GB

Cargo never deletes old build output, so `src-tauri/target` and `src-tauri/target-test`
only grow. They reached **146 GB** on a real machine, which is what motivated two
mitigations:

- **`build-installer.bat` 8 GB cache guard.** Preflight measures both directories and
  runs `cargo clean` when they exceed `CACHE_LIMIT_GB` (default 8, overridable). The
  cost is a full ~10–15 minute rebuild next time.
- **The dev profile in `Cargo.toml`:**

  ```toml
  [profile.dev]
  debug = "line-tables-only"

  [profile.dev.package."*"]
  debug = false
  ```

  Full debug info produces a multi-hundred-megabyte `.pdb` **per binary**, and every
  `tests/*.rs` is its own binary. Line tables still give `file:line` in panics and
  backtraces, so the cost is low. `[profile.release]` is the opposite extreme —
  `codegen-units = 1`, `lto = true`, `opt-level = 3`, `panic = "abort"`, `strip = true`.

**Also note:** `.build-cache/` is not restored between CI runs, so a cold runner
re-downloads the sherpa-onnx archive even though `Swatinem/rust-cache` keeps
`src-tauri/target`.

### 12.6 Smaller items worth knowing

| Item | Detail |
| --- | --- |
| `@tauri-apps/plugin-autostart` is a declared but unused npm dependency | Autostart is implemented in Rust with `auto-launch`. Nothing in `src/` imports the plugin. It can be removed from `package.json`. |
| No `lint` script | There is no ESLint or Prettier anywhere. `tsc --noEmit` and `cargo clippy` are the linters. Do not add a `lint` script expectation to CI or to docs. |
| `capabilities/mod.rs` hardcodes `acceleration: "cpu"` | It misreports the accelerator whenever the GPU pack is active. `commands/memory.rs` computes the real value from `services::gpu::is_active()`. |
| `capabilities/mod.rs` counts MCP connections by exact string | `status.state == "connected"`. Renaming the state string would silently zero the count. |
| `privacy_status` does not recognise `172.16.0.0/12` | A LAN-hosted LM Studio on a `172.x` address reports as non-local. The rule is `localhost` / `127.0.0.1` / `::1` / `[::1]`, or a `192.168.` / `10.` prefix, or a `.local` suffix. |
| Three hand-mirrored tables | `PROVIDERS` in `settings/mod.rs` ↔ `src/app/providers.ts`; the accent palette in `desktop/icon.rs` ↔ `settings::sanitize` ↔ `ACCENTS` in `settingsStore.ts` ↔ the six `.swatch` gradients in `settings.css`; the modifier-token spellings in `desktop/shortcuts.rs` ↔ `settings::sanitize` ↔ the `GetAsyncKeyState` map. |
| Two copies of the sound-tag rule | `src/app/soundTags.ts` and `services/tts/speech_text.rs`. `soundTags.test.ts` asserts cross-language parity for the *instruction* constant, but the two regex implementations are independent. |
| `open_settings` is not `async` **on purpose** | Calling it directly on the worker thread avoids a self-deadlock: Tauri window operations self-dispatch to the main loop internally, so routing `build()` onto main first makes its inner rendezvous wait on a thread it already owns. The comment is load-bearing. |
| The settings window is never destroyed | Its `CloseRequested` handler calls `prevent_close()` and hides. Rebuilding can render blank, and destroying it mid-shortcut-recording would leave global shortcuts dead for the session. |
| `--minimized` is not parsed | `autostart` registers `--minimized`, but nothing reads it; visibility is gated purely on `settings.general.startMinimized`. The flag is forwarded verbatim by the GPU relaunch. |
| `is_ignored` unit tests exist | `services/audio/mod.rs` (an FFT cost benchmark) and `services/search/mod.rs` are `#[ignore]`d inside the crate. `cargo test --lib -- --ignored --nocapture` runs them. |
| `zip` and `anyhow` are barely used | `zip` is declared but not on the model path (`.tar.bz2` is); `anyhow` is startup context only. Both are legitimate, but do not assume they are load-bearing when removing them. |
| `conv_list` exposes `offset` in Rust only | The typed client never sends it, so `offset` is always `0`. `limit` defaults to **200** in `ipc.ts` and **100** in Rust, and is capped at 1000. |

---

## 13. Related specifications

| Document | Purpose |
| --- | --- |
| [`full_documentation.md`](./full_documentation.md) | Authoritative per-file analysis of all 168 source, config and doc files. |
| [`api_reference.md`](./api_reference.md) | The complete Tauri IPC contract: 93 commands, 18 events, 2 channels, 16 error codes. |
| [`architecture_overview.md`](./architecture_overview.md) | Architecture patterns, module boundaries, technology decisions. |
| [`catalog.txt`](./catalog.txt) | Every function, type, module and component, one line each. |
| [`relationships.txt`](./relationships.txt) | Machine-readable module dependency edges. |
| [`data_models.txt`](./data_models.txt) | Schemas, entities, interfaces and types. |
| [`runtime.md`](./runtime.md) | Pipeline state, file inventory, progress log. |
| [`AGENTS.md`](../AGENTS.md) | Binding contributor rules (binding over model defaults). |
| [`README.md`](../README.md) | User-facing documentation and the end-user build instructions. |
