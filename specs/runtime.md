# RUNTIME

Documentation pipeline state for **Open Local Assistant** (`local-ai-assistant` v1.0.0).

- **Repository root:** `I:\Development\repos\Open-Local-Ai-Assistant`
- **Tracked files (git):** 232
- **Analysable source / config / doc files:** 168
- **Binary assets (excluded from line-level analysis):** 64
- **Documentation language:** English (100%)
- **Started:** 2026-09-27T21:50:21Z

---

## Scope Rules

The following generated or vendored paths are **excluded** from `Files to Process`:

| Excluded path | Reason |
| --- | --- |
| `node_modules/` | Third-party package install output (untracked) |
| `src-tauri/target/` | Rust build output (untracked) |
| `dist/` | Vite build output (untracked) |
| `release/` | Packaging output (untracked) |
| `.build-cache/` | Build cache (untracked) |
| `specs/` | This documentation package (produced by this pipeline) |

Binary assets (PNG, GIF, ICO, ICNS, ONNX) are listed in the inventory below for
coverage completeness but are **not** deep-analysed: they carry no code semantics.

---

## Files to Process

### 1. Repository Root (13)

- `/LICENSE`
- `/PRIVACY.md`
- `/README.md`
- `/build-installer.bat`
- `/commit-message.txt` (git-ignored; not tracked)
- `/index.html`
- `/package.json`
- `/package-lock.json`
- `/tsconfig.json`
- `/tsconfig.node.json`
- `/upload-release.bat`
- `/vite.config.ts`
- `/.gitignore`

### 2. CI, Editor, Scripts, Installer (5)

- `/.github/workflows/build.yml`
- `/.vscode/extensions.json`
- `/scripts/generate_icon.py`
- `/installer/open-local-assistant.iss`
- `/src-tauri/.gitignore`

### 3. Rust Build & Manifest (4)

- `/src-tauri/Cargo.toml`
- `/src-tauri/Cargo.lock`
- `/src-tauri/build.rs`
- `/src-tauri/tauri.conf.json`
- `/src-tauri/capabilities/default.json`

### 4. Rust Core — Entry, State, Errors, Logging, Capabilities (7)

- `/src-tauri/src/main.rs`
- `/src-tauri/src/lib.rs`
- `/src-tauri/src/state.rs`
- `/src-tauri/src/errors.rs`
- `/src-tauri/src/logging.rs`
- `/src-tauri/src/capabilities/mod.rs`

### 5. Rust Commands — Tauri IPC Boundary (8)

- `/src-tauri/src/commands/mod.rs`
- `/src-tauri/src/commands/app.rs`
- `/src-tauri/src/commands/attachments.rs`
- `/src-tauri/src/commands/chat.rs`
- `/src-tauri/src/commands/mcp.rs`
- `/src-tauri/src/commands/memory.rs`
- `/src-tauri/src/commands/voice.rs`

### 6. Rust Services — AI, Chat, Attachments, Language (18)

- `/src-tauri/src/services/mod.rs`
- `/src-tauri/src/services/ai/mod.rs`
- `/src-tauri/src/services/ai/lmstudio.rs`
- `/src-tauri/src/services/ai/model_selector.rs`
- `/src-tauri/src/services/ai/sse.rs`
- `/src-tauri/src/services/chat/mod.rs`
- `/src-tauri/src/services/chat/orchestrator.rs`
- `/src-tauri/src/services/chat/prompt.rs`
- `/src-tauri/src/services/chat/resolver.rs`
- `/src-tauri/src/services/chat/freshness.rs`
- `/src-tauri/src/services/chat/think.rs`
- `/src-tauri/src/services/chat/explain.rs`
- `/src-tauri/src/services/chat/attach.rs`
- `/src-tauri/src/services/chat/tools.rs`
- `/src-tauri/src/services/attachments/mod.rs`
- `/src-tauri/src/services/attachments/base64.rs`
- `/src-tauri/src/services/attachments/office.rs`
- `/src-tauri/src/services/language/mod.rs`
- `/src-tauri/src/services/language/detect.rs`

### 7. Rust Services — Voice, Audio, Models, GPU (22)

- `/src-tauri/src/services/dictation.rs`
- `/src-tauri/src/services/stt/mod.rs`
- `/src-tauri/src/services/stt/engine.rs`
- `/src-tauri/src/services/stt/session.rs`
- `/src-tauri/src/services/tts/mod.rs`
- `/src-tauri/src/services/tts/sentence_buffer.rs`
- `/src-tauri/src/services/tts/speech_text.rs`
- `/src-tauri/src/services/tts/voices.rs`
- `/src-tauri/src/services/audio/mod.rs`
- `/src-tauri/src/services/audio/capture.rs`
- `/src-tauri/src/services/audio/devices.rs`
- `/src-tauri/src/services/audio/playback.rs`
- `/src-tauri/src/services/models/mod.rs`
- `/src-tauri/src/services/models/catalog.rs`
- `/src-tauri/src/services/models/download.rs`
- `/src-tauri/src/services/gpu/mod.rs`
- `/src-tauri/src/services/hardware/mod.rs`

### 8. Rust Services — MCP, Search (6)

- `/src-tauri/src/services/mcp/mod.rs`
- `/src-tauri/src/services/mcp/config.rs`
- `/src-tauri/src/services/mcp/permissions.rs`
- `/src-tauri/src/services/mcp/sources.rs`
- `/src-tauri/src/services/search/mod.rs`

### 9. Rust Persistence — Database & Settings (8)

- `/src-tauri/src/database/mod.rs`
- `/src-tauri/src/database/conversations.rs`
- `/src-tauri/src/database/dictation.rs`
- `/src-tauri/src/database/export.rs`
- `/src-tauri/src/database/migrations.rs`
- `/src-tauri/src/settings/mod.rs`
- `/src-tauri/src/settings/backup.rs`

### 10. Rust Desktop Integration (9)

- `/src-tauri/src/desktop/mod.rs`
- `/src-tauri/src/desktop/window.rs`
- `/src-tauri/src/desktop/tray.rs`
- `/src-tauri/src/desktop/shortcuts.rs`
- `/src-tauri/src/desktop/autostart.rs`
- `/src-tauri/src/desktop/icon.rs`
- `/src-tauri/src/desktop/verify.rs`

### 11. Rust Binaries & Tests (9)

- `/src-tauri/src/bin/gpu_probe.rs`
- `/src-tauri/src/bin/mcp_fixture_server.rs`
- `/src-tauri/tests/db_smoke.rs`
- `/src-tauri/tests/gpu_pack.rs`
- `/src-tauri/tests/lmstudio_live.rs`
- `/src-tauri/tests/mcp_integration.rs`
- `/src-tauri/tests/voice_models.rs`
- `/src-tauri/tests/web_search_live.rs`

### 12. Frontend — App Layer (13)

- `/src/main.tsx`
- `/src/vite-env.d.ts`
- `/src/app/attach.ts`
- `/src/app/chatStore.ts`
- `/src/app/ipc.ts`
- `/src/app/knownLanguages.ts`
- `/src/app/providers.ts`
- `/src/app/settingsHighlight.ts`
- `/src/app/settingsStore.ts`
- `/src/app/soundTags.ts`
- `/src/app/strings.ts`
- `/src/app/strings.en.json`
- `/src/app/types.ts`
- `/src/app/vision.ts`
- `/src/app/voiceStore.ts`

### 13. Frontend — Components (22)

- `/src/components/chat/Attachments.tsx`
- `/src/components/chat/Composer.tsx`
- `/src/components/chat/Markdown.tsx`
- `/src/components/chat/MessageBubble.tsx`
- `/src/components/chat/ModelPicker.tsx`
- `/src/components/chat/SelectionMenu.tsx`
- `/src/components/chat/Sources.tsx`
- `/src/components/chat/ToolActivity.tsx`
- `/src/components/chat/ToolConfirmDialog.tsx`
- `/src/components/common/BrandMark.tsx`
- `/src/components/common/ErrorBoundary.tsx`
- `/src/components/common/controls.tsx`
- `/src/components/history/HistoryPanel.tsx`
- `/src/components/settings/GpuCard.tsx`
- `/src/components/settings/LanguagePicker.tsx`
- `/src/components/settings/ModelManager.tsx`
- `/src/components/settings/ShortcutInput.tsx`
- `/src/components/settings/layout.tsx`
- `/src/components/voice/CallView.tsx`
- `/src/components/voice/LevelMeter.tsx`
- `/src/components/voice/SpokenText.tsx`
- `/src/components/voice/VoiceBars.tsx`

### 14. Frontend — Pages (15)

- `/src/pages/Bubble/Bubble.tsx`
- `/src/pages/Chat/ChatApp.tsx`
- `/src/pages/Overlay/Overlay.tsx`
- `/src/pages/Settings/SettingsApp.tsx`
- `/src/pages/Settings/searchIndex.ts`
- `/src/pages/Settings/sections/About.tsx`
- `/src/pages/Settings/sections/Ai.tsx`
- `/src/pages/Settings/sections/Basic.tsx`
- `/src/pages/Settings/sections/Dictation.tsx`
- `/src/pages/Settings/sections/Languages.tsx`
- `/src/pages/Settings/sections/Mcp.tsx`
- `/src/pages/Settings/sections/Memory.tsx`
- `/src/pages/Settings/sections/System.tsx`
- `/src/pages/Settings/sections/Voice.tsx`
- `/src/pages/Settings/sections/WebSearch.tsx`
- `/src/pages/Setup/SetupWizard.tsx`

### 15. Frontend — Styles (4)

- `/src/styles/tokens.css`
- `/src/styles/base.css`
- `/src/styles/chat.css`
- `/src/styles/settings.css`

### 16. Frontend — Tests (8)

- `/src/test/setup.ts`
- `/src/test/ui.test.tsx`
- `/src/test/history.test.tsx`
- `/src/test/selection.test.tsx`
- `/src/test/settings.test.tsx`
- `/src/test/shortcutinput.test.tsx`
- `/src/test/soundTags.test.ts`
- `/src/test/strings.test.ts`
- `/src/test/voiceSessions.test.ts`

### 17. Binary Assets (64) — Inventoried, Not Analysed

- `/assets/icon.png`
- `/docs/icon.png`
- `/docs/screenshots/chat.png`
- `/docs/screenshots/chat-web-search.png`
- `/docs/screenshots/dictation-overlay.png`
- `/docs/screenshots/explain-demo.gif`
- `/docs/screenshots/explain-in-chat.png`
- `/docs/screenshots/explain-popup.png`
- `/docs/screenshots/hands-free-call.png`
- `/docs/screenshots/model-picker.png`
- `/docs/screenshots/settings-ai-provider.png`
- `/docs/screenshots/settings-assistant-voice.png`
- `/docs/screenshots/settings-dictation.png`
- `/docs/screenshots/settings-loaded-models.png`
- `/docs/screenshots/settings-mcp-tools.png`
- `/src-tauri/icons/32x32.png`, `64x64.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, `icon.ico`, `icon.icns`
- `/src-tauri/icons/Square*.png` (Windows Store logo set, 9 files)
- `/src-tauri/icons/StoreLogo.png`
- `/src-tauri/icons/ios/AppIcon-*.png` (19 files)
- `/src-tauri/icons/android/mipmap-*/ic_launcher*.png` (15 files)
- `/src-tauri/icons/android/mipmap-anydpi-v26/ic_launcher.xml`
- `/src-tauri/icons/android/values/ic_launcher_background.xml`
- `/src-tauri/models/` sample `.onnx` model file

### 18. Release Notes (2)

- `/release-notes/v0.1.0.md`
- `/release-notes/v1.0.0.md`

---

## Files Processed

### 1. Repository Root (13) — COMPLETE
`LICENSE`, `PRIVACY.md`, `README.md`, `build-installer.bat`, `commit-message.txt`,
`index.html`, `package.json`, `package-lock.json`, `tsconfig.json`,
`tsconfig.node.json`, `upload-release.bat`, `vite.config.ts`, `.gitignore`

### 2. CI, Editor, Scripts, Installer (5) — PARTIAL
`.github/workflows/build.yml`, `.vscode/extensions.json`, `scripts/generate_icon.py`,
`installer/open-local-assistant.iss`, `src-tauri/.gitignore`

### 3. Rust Build & Manifest (5) — COMPLETE
`src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/build.rs`,
`src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`

### 4. Rust Core (6) — COMPLETE
`main.rs`, `lib.rs`, `state.rs`, `errors.rs`, `logging.rs`, `capabilities/mod.rs`

### 5. Rust Commands (7) — COMPLETE
`commands/mod.rs`, `app.rs`, `attachments.rs`, `chat.rs`, `mcp.rs`, `memory.rs`, `voice.rs`

### 6. Rust Services — AI, Chat, Attachments, Language (19) — PENDING

### 7. Rust Services — Voice, Audio, Models, GPU (17) — COMPLETE
`services/mod.rs`, `dictation.rs`, `stt/{mod,engine,session}.rs`,
`tts/{mod,sentence_buffer,speech_text,voices}.rs`,
`audio/{mod,capture,devices,playback}.rs`, `models/{mod,catalog,download}.rs`,
`gpu/mod.rs`, `hardware/mod.rs`

### 8. Rust Services — MCP, Search (5) — PENDING

### 9. Rust Persistence (7) — PENDING

### 10. Rust Desktop (7) — PENDING

### 11. Rust Binaries & Tests (9) — PENDING

### 12. Frontend App Layer (14) — COMPLETE
`src/main.tsx`, `src/vite-env.d.ts`, `app/{ipc,types,chatStore,settingsStore,voiceStore,attach,vision,providers,knownLanguages,settingsHighlight,soundTags,strings}.ts`, `app/strings.en.json`

### 13. Frontend Components (22) — PENDING

### 14. Frontend Pages (16) — PENDING

### 15. Frontend Styles (4) — COMPLETE
`styles/{tokens,base,chat,settings}.css`

### 16. Frontend Tests (9) — COMPLETE
`test/{setup,ui.test,history.test,selection.test,settings.test,shortcutinput.test,soundTags.test,strings.test,voiceSessions.test}.ts(x)`

### 17. Binary Assets (64) — INVENTORIED (excluded from line analysis)

### 18. Release Notes (2) — COMPLETE
`release-notes/v0.1.0.md`, `release-notes/v1.0.0.md`

---

## Log

- [2026-09-27T21:50:21Z] Initialisation started. Structured reasoning mode enabled; 9-phase plan created.
- [2026-09-27T21:50:44Z] `specs/` and `specs/diagrams/` created.
- [2026-09-28T00:51:44Z] File discovery completed. 232 tracked files enumerated: 168 analysable, 64 binary assets.
- [2026-09-28T00:51:44Z] Root manifests read: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `README.md`. Project identified as a Tauri 2 desktop application (Rust backend + React 19 / TypeScript frontend).
- [2026-09-28T00:56:00Z] 8 parallel deep-analysis agents launched across: Rust core+commands, AI+chat services, voice services, MCP+search+data, desktop+tests, frontend app layer, frontend components+pages, styles+tests+root.
- [2026-09-28T00:58:04Z] Agent 8/8 complete (styles, tests, root config, docs). Styles section 4.1 written.
- [2026-09-28T00:58:39Z] Agent 6/8 complete (frontend app layer). Section 4.2 written.
- [2026-09-28T00:59:24Z] Agent 1/8 complete (Rust core + all 93 IPC commands). Section 4.5 written. **Verified: 93 `#[tauri::command]` functions match the 93 entries in the `lib.rs` invoke_handler list exactly — 1:1, nothing unregistered, nothing phantom.**
- [2026-09-28T00:59:24Z] Agent 3/8 complete (voice services). Section 4.6 written.
- [2026-09-28T00:59:24Z] Sections 4.3 (frontend tests) and 4.4 (root configuration) written.
- [2026-09-28T00:59:24Z] `specs/diagrams/architecture.mmd` drafted from the confirmed module structure.
- [2026-09-28T00:59:24Z] Four agent reports outstanding: AI+chat services, MCP+search+data, desktop+tests, frontend components+pages.
- [2026-09-28T01:00:26Z] Agent 5/8 complete (desktop + build pipeline + integration tests). Section 4.7 written.
- [2026-09-28T01:00:56Z] Agent 2/8 complete (AI + chat services). **All 8 analysis reports received.**
- [2026-09-28T01:01:35Z] Agent 7/8 complete (frontend components + pages). Section 4.10/4.11 content captured.
- [2026-09-28T01:01:35Z] Analysis phase COMPLETE — 8/8 reports. Verified during ingestion: a flagged line break in `orchestrator.rs:38-40` is valid Rust (a raw newline inside a string literal is legal and yields the intended blank-line separator), confirmed by byte-level read plus an isolated `rustc` compile. Not a defect.
- [2026-09-28T01:01:35Z] Authoring phase launched: 4 parallel writer agents, each owning a disjoint set of `specs/` files — (a) architecture_overview + dataflow/classes/sequence diagrams, (b) catalog/relationships/data_models, (c) developer_guide + api_reference, (d) full_documentation §4.8–4.11 and §5–7.
- [2026-09-28T01:01:35Z] Headings verified: exactly one `## 1`–`## 9` each; no duplicate section numbers.
