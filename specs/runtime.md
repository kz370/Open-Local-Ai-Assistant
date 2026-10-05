# RUNTIME

Documentation pipeline state for **Open Local Assistant** (`local-ai-assistant` v1.1.0).

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

### 16. Frontend — Tests (9)

- `/src/test/setup.ts`
- `/src/test/ui.test.tsx`
- `/src/test/mcpDropdown.test.tsx`
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

### 18. Release Notes (3)

- `/release-notes/v0.1.0.md`
- `/release-notes/v1.0.0.md`
- `/release-notes/v1.1.0.md`

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

### 16. Frontend Tests (10) — COMPLETE
`test/{setup,ui.test,mcpDropdown.test,history.test,selection.test,settings.test,shortcutinput.test,soundTags.test,strings.test,voiceSessions.test}.ts(x)`

### 17. Binary Assets (64) — INVENTORIED (excluded from line analysis)

### 18. Release Notes (3) — COMPLETE
`release-notes/v0.1.0.md`, `release-notes/v1.0.0.md`, `release-notes/v1.1.0.md`

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
- [2026-09-28T23:42:00Z] Session-scoped tool toggles added: `SendInput` extended with `web_search_enabled` and `mcp_enabled`; `chatStore` gains `sessionWebSearch`/`sessionMcpEnabled`; UI adds websearch icon toggle and MCP server dropdown in header; specs updated.
- [2026-09-28T23:58:00Z] Chat MCP control rebuilt around the composer dropdown and made independent of the Settings form. The chip now lives in the composer bar, lists **only servers enabled in Settings** and is hidden entirely when none are enabled; every switch is **off by default** and per-chat only (`mcpEnabled` is now always sent, and `newConversation` no longer clears `sessionMcpEnabled`, which had been silently handing the next turn every enabled server). The dropdown follows `mcp://changed`, so enabling a server in Settings shows up live; its popover is anchored to the chip's end edge because a start-anchored menu was clipped by the window. New store action `syncSessionMcp` reconciles a refreshed server list without discarding the user's switches. `src/test/mcpDropdown.test.tsx` added (4 cases). Sections 13/14 remain PENDING as before, so `Files Processed` still does not equal `Files to Process` for those two pre-existing sections.
- [2026-09-29T00:44:00Z] Follow-up polish: a gear in the dropdown header opens `open_settings_window { section: "mcp" }` (the chat window still writes no server state); the MCP row rules were moved after `.picker-item` and given a compound selector, because `.picker-item`'s own `padding`/`border-radius` in the same file was overriding them and the earlier sizing edits had no effect. Dropdown: 240 px wide, 18 px radius, 10 px padding, rows 10/12 px with an 11 px radius growing to 14 px plus a hairline ring on hover/focus. `src/test/mcpDropdown.test.tsx` grown to 6 cases, including a server enabled mid-conversation and the gear. Full frontend suite: 134 passed.
- [2026-09-29T00:46:00Z] Dropdown hint restyled: `.picker-hint` moved off the 10 px faint token to `--text-xs` / `--text-muted` with 1.45 line-height, since the "off until you switch them on" line was barely legible at the previous size.
- [2026-09-29T00:48:00Z] Fixed squared-off window corners while a dialog was open in the chat route (Clear history, tool confirmation). `.dialog-backdrop` is `position: fixed`, so the shell's `overflow: hidden` never clipped it and its opaque background covered the transparent rounded corners. `html[data-route="chat"] .dialog-backdrop` now carries the shell's 25 px radius, dropped to 0 under `.app-shell.maximized`. Affects every full-screen dialog in that route, not just the history one.
- [2026-09-29T00:55:00Z] History panel turned into a compact table list. `HistoryPanel` gained a `stamp()` helper (day/month/year built from the date parts, plus the locale's 12/24-hour time) rendered **under** the conversation title inside the same button, so a row is title + stamp (+ search snippet) with the rename/export/delete group pinned to a fixed 78 px right-hand track. Title is `--text-sm`/`--text`, the stamp `--text-xs`/`--text-faint` with tabular figures. The row separator is a short centred 36 px rule drawn in the 3 px gap below each row (a `::before` pseudo-element), fading out on hover — the earlier full-width `border-bottom` read as part of the rounded hover card. **Cascade fix:** the whole history block is now scoped `html[data-route="chat"] …` because `main.tsx` imports `settings.css` *after* `chat.css`, and the settings window's own `.history-item` rule (`display:flex`, `padding: 10px 0`) was overriding the chat one — that is why every earlier sizing change appeared to do nothing. Test `history panel › renders each row with its last-message stamp under the title` added.
- [2026-09-29T02:07:00Z] Fixed "(no content generated)" in Explain popup when models do not stream in chunks or answer in one piece. In `LmStudioService::chat`, detect non-SSE responses (`!is_sse`) when `req.stream == true` and parse them as full JSON completions, support alternative SSE chunk formats (`delta.text`, `choice.text`, `message.content`, `thought`), and fall back to raw JSON completion parsing if no SSE chunks were parsed. In `orchestrator.rs::explain`, respect `settings.ai.streaming` and `settings.ai.max_tokens`, buffer reasoning chunks during streaming, and fall back to `done.content` / reasoning text before emitting `(no content generated)`. In `explain::request`, accept `stream` and `max_tokens` (defaulting to 2048 with a floor of 1024 to support reasoning models). Added tests in `lmstudio.rs` and `orchestrator.rs`.
- [2026-09-29T02:14:00Z] Excluded thinking/reasoning process from Explain popup. In `LmStudioService::chat`, ensured reasoning tokens in SSE chunks are strictly separated and never fall back to `choice["text"]` or `delta["text"]` as content. In `orchestrator.rs::explain`, removed reasoning fallback from the popup delta emitter so the popup renders only the final explanation text, filtering out internal thinking scratchpads. Unit tests updated and verified.
- [2026-09-29T02:27:00Z] Resolved `(no content generated)` recurrence for reasoning models in Explain popup. In `explain.rs`, removed artificial `max_tokens` cap (which prematurely cut off reasoning models mid-thought before producing visible content) and added instruction in `EXPLAIN_PROMPT` to keep thinking brief. In `orchestrator.rs::explain`, clarified length-limit diagnostic if token limit is reached, maintaining clean visible-only content streaming for the popup. Unit tests verified (171 passed).
- [2026-09-29T02:40:00Z] Fixed `upload-release.bat` to force-update the release tag to the pushed `HEAD` before creating/updating the release, ensuring GitHub's source-code archives track the published source. Updated release instructions in `README.md`, `specs/developer_guide.md`, and `specs/full_documentation.md`.
- [2026-09-29T03:20:00Z] Explain popup: modernised the Copy / Speak / Ask in chat actions and gave Speak real playback feedback. The three `.btn.btn-sm` became `.xact` pills (28px, fully rounded, tinted footer, hover lift + press scale, accent-tinted primary action pinned right). Speak is now tagged with the explanation's own id, so the popover tracks its own audio via `useVoice.speaking`/`speakingTag`: **Speak → "Preparing…" (spinner) → "Speaking…" (animated 4-bar `.xact-wave` + accent ring) → Speak** when TTS emits `idle`; clicking while preparing or speaking calls `tts_stop`, and `PREPARE_TIMEOUT_MS` (90s) clears the spinner if no voice ever starts. New strings `chat.selection.speaking` / `chat.selection.preparingSpeech`. Test `shows the explanation is being spoken, and stops it on click` added (142 UI tests pass).
- [2026-10-02T21:22:00Z] Dictation review now opens an editable preview immediately when Review is clicked, seeded with the latest live partial while final transcription continues. Review-aware labels/hints, a transcribing spinner/status hint, disabled Insert/Retry until the final result is ready, and preservation of user edits improve responsiveness without inserting incomplete text. Updated the IPC/type/docs contracts; frontend tests cover preview-to-final state and edit preservation.
- [2026-10-02T21:41:00Z] Dictation correction output now collapses whitespace runs after removing model wrappers, preventing tabs/multiple spaces from leaking into review text or insertion. Added a regression test for repeated spaces and tabs.
- [2026-10-03T01:03:00Z] Follow-up Notepad reproduction confirmed Windows Typing still corrupted reviewed text after restoring Enigo's bulk text path. Windows Typing now routes individual characters through Enigo's Unicode sender with a 4 ms gap, preserving Enigo event metadata; Pasting behavior is unchanged.
- [2026-10-03T02:30:00Z] Dictation **Typing** no longer accumulates stray characters and mid-word typos (Pasting was always clean). Two defects fixed in `src-tauri/src/services/dictation.rs`. (1) **Injection-failure guard:** the live-typing worker advanced `current` even when `apply_delta_raw` returned `Err`, so one dropped `SendInput` chunk (UIPI, focus change, overlay) left the model claiming text the screen never received — every later `reconcile` then computed backspaces against fiction and leftover runs accumulated on screen, surviving into the final text. `current` now advances only on `Ok`; on failure the new `desynced` flag is set, and the next apply (partial or `finish`) erases the last confirmed length and retypes the goal instead of diffing. `desynced` survives `reset` because the stray text is still on screen. (2) **Word-committed coalescing:** new `committed_prefix` cuts every partial at its last word boundary (`is_word_char` treats alphanumerics and the apostrophe forms `'` `’` `ʼ` `＇` as word characters), so a word the recognizer is still revising never reaches the screen and never needs backspacing; the trailing word is typed once by `finish`. Also extracted `plain_keyboard()` so `apply_delta_raw` and the retype path no longer build `Enigo` inline. New test `live_typing_only_commits_whole_words` covers ASCII, punctuation, apostrophes and multi-byte text; `cargo test -p local-ai-assistant --lib` = 172 passed, `cargo clippy --all-targets` clean for this module. Specs: `full_documentation.md` (Summary, Technical Details, Business Logic — text insertion), `catalog.txt`, this log. Sections 13/14 remain PENDING as before, so `Files Processed` still does not equal `Files to Process` for those two pre-existing sections.
- [2026-10-03T03:10:00Z] **Root cause of the dictation typos found and fixed: `KEYEVENTF_UNICODE` injection, not recognition.** Evidence chain: (a) the newest `dictation_history` row for a garbled take stores a perfectly clean `raw`/`text` pair (`"No, I don't think this is going to work at all. It might take me an entire year to land a single job."`, `corrected=0`), and the log has no `live-typing dictation partial failed` warnings, so the recognizer and the diff engine were never at fault; (b) the active settings are `insertMethod=type`, `reviewBeforeInsert=true`, `mode=toggle`, which means live typing is disabled and every garbled line came from the one-shot `insert_text` → `type_text` path; (c) raw `SendInput` measurements reproduced the exact symptom in classic Win32 edit controls — per-character `KEYEVENTF_UNICODE` at 4 ms produced `"...to work at all. It might take    nn entire aaar      a nnngle job."` from text that decodes perfectly, Enigo's bulk single-call form dropped everything except the tail (`"land a single job."`), while `KEYEVENTF_SCANCODE` events came through byte-exact in Notepad and in Chromium (WebView2). Fix: new `#[cfg(windows)] mod key_events` resolves each character against the **target window's** keyboard layout (`VkKeyScanExW` → `MapVirtualKeyExW(MAPVK_VK_TO_VSC_EX)`) into a scancode plus optional left-shift and injects `KEYEVENTF_SCANCODE` down/up pairs, falling back to Enigo's Unicode path for single characters the layout cannot produce (emoji, or Arabic under an English layout). The per-character gap moved from 4 ms to **16 ms**: at 4-8 ms the receiving application silently drops characters while `SendInput` still reports every event accepted (visible as a stuck shift, `unicode` → `Unicode`), at 16 ms and above nothing was lost across all measured cases. `insert_text`, `LiveTyper` and `reconcile` are unchanged in signature — both the one-shot and the live-typing path go through the new `type_text`. Verified by replicating the exact algorithm outside the app against Notepad: the dictation sentence, a mixed-case/digits/symbols line, a multiple-spaces line and an Arabic line all arrived byte-exact. Specs: `full_documentation.md` (Business Logic — text insertion, with the measurements and the reason for both choices), `catalog.txt`, this log.
- [2026-10-03T03:40:00Z] Fixed dictation failing with the generic "the audio device could not be used". The new logging named the real error: **`A buffer underrun or overrun occurred.`** — cpal/WASAPI `AUDCLNT_E_BUFFER_ERROR`, raised by the Realtek driver ~80 ms after `stream.play()`, i.e. the mic *was* opened (`microphone opened device=Microphone (Realtek High Definition Audio) 44100/2ch F32`) and then the driver killed the fresh stream. WASAPI cannot revive a dead client, so `Capture::start` now rebuilds the stream: the capture thread waits `START_SETTLE` = 250 ms for the stream to stay alive before reporting success, and on an early death drops it, waits `RETRY_PAUSE` = 300 ms and builds a fresh one, up to `START_ATTEMPTS` = 4. The caller waits up to 10 s for that handshake (was 5 s) and only sees an error if all four attempts fail, in which case it gets the real cpal message instead of a generic one. After startup the same thread drains the stream's error channel, so a later failure reaches the session as `CaptureEvent::Error` instead of vanishing. Ruled out along the way: device selection (stream was built), our own leaks (`Capture` implements `Drop` → `stop()` → join), and other mic clients (only one app instance; Windows' mic history showed no other user since hours earlier). Also corrected a spec error: cpal 0.18's fifth `build_input_stream` argument is the **activation timeout**, not a buffer size. `cargo test -p local-ai-assistant --lib` = 172 passed. Specs: `full_documentation.md` (Summary line count, Technical Details, startup handshake, Concurrency), `catalog.txt`, this log.
- [2026-10-03T03:55:00Z] Dictation overlay now animates with the microphone. The overlay header drew a rolling LevelMeter capped at max={14} inside a 14px-tall .overlay-head .meter box: heights were Math.max(3, Math.round(v * 14)), so a healthy speaking level of ~0.5 produced 7px and any level below 0.107 produced the same 3px as silence - the bars moved at most a pixel or two and read as frozen even though voice://event level events were arriving (confirmed live: the debug log once-per-second microphone level reads ~0.5, and live partial text streams in, which shares the exact same emit_to(window::OVERLAY, "voice://event") route at lib.rs:230). Fix is display-only, no Rust change: the overlay now renders VoiceBars from the bands array that the same level event already carried and the overlay was discarding, so bars are scaled with transform: scaleY() on a 0.12 floor and move continuously instead of snapping to integer pixels. src/pages/Overlay/Overlay.tsx keeps bands + level state instead of an 18-slot rolling array; .overlay-head .meter was replaced by .overlay-head .voice-bars (148x18px, 2px bars) in src/styles/settings.css. npm run typecheck and npm run build clean. Specs: full_documentation.md (settings.css overlay live meter), catalog.txt (Overlay entry), relationships.txt (Overlay uses VoiceBars), this log. Not touched here: the separate "A buffer underrun or overrun occurred" storm still visible in the log after startup, where duplicate error lines suggest two capture streams on one device; that is its own defect and unchanged.
- [2026-10-03T04:25:00Z] Dictation overlay meter, verified end to end instead of assumed. A temporary `#[ignore]`d probe opened the live microphone through `Capture::start` for 8 s: `level` sat at 0.39-0.44 (room tone, only +-0.02 of movement) while the spectrum moved hard - `peak_band` between 0.67 and 0.75, individual bands across 0.20-0.74. That is the whole original bug: the old `LevelMeter` drew `Math.max(3, Math.round(v * 14))` inside a 14px box, so a +-0.02 level wobble produced byte-identical pixel heights and the header read as frozen while recognition worked. Bands carry ~40x more dynamic range than the RMS scalar, which is why the overlay now renders `VoiceBars`. Two permanent tests lock the chain: `level_carries_the_spectrum_and_a_mute_flattens_it` (session.rs - `emit_level` forwards all 24 bands and zeroes value and bands only when the microphone is muted) and `moves the spectrum bars while the microphone is picked up` (src/test/ui.test.tsx - a dictation `level` event with new bands re-renders the 48 mirrored bars). `cargo test -p local-ai-assistant --lib` = 173 passed, `cargo clippy --all-targets` adds no new warnings, `cargo fmt --all` clean, `npm test` = 147 passed. Note: the report that the change made things worse came from a binary built before it (target/debug/local-ai-assistant.exe was rebuilt at 02:23 local, the complaint was at 02:16), so that observation says nothing about this fix. Specs: this log, `catalog.txt`, `relationships.txt`, `full_documentation.md` (unchanged in this pass).
- [2026-10-03T04:55:00Z] Dictation **profiles**: a user-authored list of (title, prompt) pairs that reshape a take for the topic being spoken about (programming, imaging, a report). The overlay header gained a second pill next to the language pill; `LanguageMenu` was generalised into `PillMenu` and both pills share **one** window clip rectangle (`MENU_RECT`), tracked by a module-level `menuOwner` so only the pill that opened it may take it away. Rust: `DictationProfile { id, title, prompt }` plus `profiles` / `profileSearchThreshold` (default 10, clamped 2..=100) / `activeProfile` on `DictationSettings`; `sanitize` drops empty-id/empty-title and duplicate-id rows, trims id/title/prompt to 40/60/4000, **caps no list length**, and clears a dangling `activeProfile`; new `dictation_set_profile` command. `correct_text` takes an optional profile prompt, **appends** it to `CORRECTION_PROMPT` (the built-in grammar/emoji rules always run), and **skips the length-ratio guard** plus **keeps line structure** in `sanitize_correction` for a profile - both guards exist to catch a model that answers instead of correcting, which is exactly what a profile may ask for; without a profile the output is still forced onto one line. `run_dictation` resolves the prompt at take time and a non-empty profile **forces the model pass on even when `correctionEnabled` is false** (no model => raw transcript + `NoModel`, as before). UI: profiles sub-page at `#/settings/dictation/profiles` (`DictationProfiles.tsx`) reached from a card on the Dictation page; `SettingsApp` now splits the hash into section + sub-page. The page is a **draft with an explicit Save button** - nothing reaches the store until pressed, and an emptied title marks the row invalid (`aria-invalid`, Save disabled, inline error) instead of letting the store silently drop it; `Saved` is only reported once the store actually holds the draft, so a failed save leaves the edits in place. From `profileSearchThreshold` profiles (default 10) the profile dropdown grows a filter row that counts as a menu row for the clip height. Two layout defects fixed in the same pass: the header no longer overflows the fixed 480px card (`.overlay-pills` is the only shrinking wrapper, pills truncate, the spectrum sheds bars to a 56px floor, the empty profile pill is icon-only) and the temporary level/peak/band debug readout was removed from the header. Tests: `sanitize_cleans_dictation_profiles`, `profile_prompt_is_appended_and_skips_the_length_guard`, `offers a profile pill only when profiles exist, and saves the pick`, `searches the profile list once it reaches the threshold`, plus three `dictation profiles` settings cases; `cargo test -p local-ai-assistant --lib` = 175 passed, `npm test` = 152 passed, `npm run typecheck` and `npm run build` clean, `cargo clippy --all-targets` adds no new warnings. Specs: `full_documentation.md` (settings.css header budget, dictation.rs Business Logic - dictation profiles, DictationSettings fields, voice.rs command count), `catalog.txt`, `relationships.txt`, `api_reference.md` (renumbered 52-93 to 53-94, 93 -> 94 commands), `data_models.txt`, this log. Sections 13/14 remain PENDING as before.
- [2026-10-03T00:10:00Z] Version bumped to **1.1.0** in every place that drives a build: `package.json`, `package-lock.json` (both the root and `packages[""]` entries), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (rewritten by cargo itself), and `src-tauri/tauri.conf.json` — the last is what the About page and the NSIS installer read. `build-installer.bat` and `upload-release.bat` were **not** touched: the first parses the `version` line out of `Cargo.toml`, the second derives the tag from the setup exe name, so both follow automatically. Spec headers updated to `local-ai-assistant` v1.1.0 in `api_reference.md`, `architecture_overview.md`, `developer_guide.md`, `full_documentation.md` and this file. Added `release-notes/v1.1.0.md` (33 lines): leads with dictation profiles, lists the Windows dictation fixes (scancode key injection, the live-typing desync, the capture restart handshake, the frozen spectrum meter, the header overflow) under **Fixed**, notes that profiles need a correction model, and repeats the SmartScreen plus `Get-FileHash` verification guidance and the two download artefact names. Docs pipeline rebalanced for the new file: "Files to Process" section 18 (Release Notes) 2 -> 3 with `/release-notes/v1.1.0.md` listed, and "Files Processed" section 18 2 -> 3 marked COMPLETE, so the two counters still agree. `full_documentation.md`'s release-notes file entry rewritten to cover all three notes. Verified: `npm run build` clean, `npm run typecheck` clean, `cargo test -p local-ai-assistant --lib` = 175 passed. Sections 13/14 remain PENDING as before.
- [2026-10-03T00:30:00Z] Build output now lands in **one folder per release**: `release\v<version>\` (v1.1.0 → `release\v1.1.0\`), instead of every build dropping its portable exe, the four speech DLLs and the setup exe into a flat `release\` where rebuilding an old version overwrote the current release's files. `build-installer.bat` parses `VERSION` from `src-tauri\Cargo.toml` **first** and then derives `DIST = %ROOT%release\v%VERSION%` from it (the `DIST` assignment moved below the version parse — that ordering is load-bearing). `upload-release.bat` was moved in lockstep: with no tag it finds the newest setup exe anywhere under `release\` (a `dir` glob across subfolders is invalid on Windows, so it is a PowerShell `Get-ChildItem -Recurse | Sort-Object LastWriteTime -Descending | Select-Object -First 1` — `dir /o-d /s` sorts per directory, not globally, and was verified to pick the *older* build), then splits that full path into folder (`%%~dpP`) and file name (`%%~nxP`); with an explicit tag it reads `release\v<version>\` and falls back to the flat `release\` so artifacts built before this change still publish. `.github/workflows/build.yml` step 8 had the same stale assumption and would have thrown on every CI run: it now picks the newest `release` subfolder containing `Open Local Assistant.exe` and asserts the five portable files plus at least one setup exe inside it (`upload-artifact`'s `path: release/` stays valid — it walks recursively). `.gitignore`'s `release/` already covers the nested folders. No `CoreCommand`, IPC command or type change. Specs: `developer_guide.md` §11.2 (rewritten layout tree), §12.1 (the quoted `Get-ChildItem` line), `full_documentation.md` (build-installer variables, `upload-release.bat` Purpose, CI step 8), `catalog.txt` (`DIST`, `upload-release.bat`, `build.yml` steps), this log. File counts unchanged, so `Files Processed` still equals `Files to Process`.
- [2026-10-05T18:14:00Z] Dictation review exposes Esc / Ctrl+Shift+Backspace to cancel, Ctrl+Enter to insert, and Ctrl+Shift+Enter to retry; Alt+Enter is deliberately not intercepted. Configured global app shortcuts are temporarily replaced by review-specific global registrations while review is open, making actions work even when the dictated-into app is focused. The review shows the remembered destination window title when available and always offers Copy. Added UI coverage for review actions, destination title, Copy and Alt+Enter non-conflict. Specs updated: `api_reference.md`, `full_documentation.md`, `catalog.txt`, `relationships.txt`, this log.
- [2026-10-05T18:25:00Z] Improved the review destination label's contrast and fixed a delayed-hide race: each dictation start advances a session generation, and the UI-thread hide rechecks that generation and idle state before hiding. A stale linger timer can no longer dismiss the overlay belonging to a newer take. Specs updated: `full_documentation.md`, `catalog.txt`, this log.
- [2026-10-05T18:40:00Z] Compact chat now defaults to 440×170 logical px and can be resized horizontally from 440 to 760 px while its height stays fixed. The old 380 px exact lock squeezed the composer toolbar and caused the MCP and microphone controls to overlap. Specs updated: `full_documentation.md`, `catalog.txt`, this log.
