import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { ArrowUp, AudioLines, ChevronDown, Clock, Globe, Mic, Paperclip, Plus, Square, Volume2, VolumeX, X } from "lucide-react";
import { attachFromPaste, pickFiles } from "../../app/attach";
import { useChat } from "../../app/chatStore";
import { ipc } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import { useVoice } from "../../app/voiceStore";
import type { ServerStatus } from "../../app/types";
import { textDir } from "../common/controls";
import { LevelMeter } from "../voice/LevelMeter";
import { VoiceBars } from "../voice/VoiceBars";
import { AttachmentList } from "./Attachments";
import { ModelPicker } from "./ModelPicker";

export interface ComposerHandle {
  focus: () => void;
}

export const Composer = forwardRef<ComposerHandle, { onVoiceSetup: () => void; autoModel?: string | null }>(function Composer({ onVoiceSetup, autoModel = null }, ref) {
  const draft = useChat((s) => s.draft);
  const setDraft = useChat((s) => s.setDraft);
  const send = useChat((s) => s.send);
  const stop = useChat((s) => s.stop);
  const busy = useChat((s) => s.turnId !== null);
  const queue = useChat((s) => s.queue);
  const removeQueued = useChat((s) => s.removeQueued);
  const voiceNotice = useChat((s) => s.voiceNotice);
  const setVoiceNotice = useChat((s) => s.setVoiceNotice);
  const attachments = useChat((s) => s.attachments);
  const attachmentErrors = useChat((s) => s.attachmentErrors);
  const removeAttachment = useChat((s) => s.removeAttachment);
  const dismissAttachmentErrors = useChat((s) => s.dismissAttachmentErrors);
  const sessionWebSearch = useChat((s) => s.sessionWebSearch);
  const setSessionWebSearch = useChat((s) => s.setSessionWebSearch);
  const sessionMcpEnabled = useChat((s) => s.sessionMcpEnabled);
  const setSessionMcpEnabled = useChat((s) => s.setSessionMcpEnabled);
  const resetSessionTools = useChat((s) => s.resetSessionTools);
  const voice = useVoice();
  const speak = useSettings((s) => s.settings?.tts.speakResponses ?? false);
  const pasteAsFileChars = useSettings((s) => s.settings?.ai.pasteAsFileChars ?? 0);
  const updateSettings = useSettings((s) => s.update);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const [mcpServers, setMcpServers] = useState<ServerStatus[]>([]);
  const [mcpOpen, setMcpOpen] = useState(false);
  const mcpRootRef = useRef<HTMLDivElement>(null);

  useImperativeHandle(ref, () => ({ focus: () => taRef.current?.focus() }));

  const recording = voice.mode === "pushToTalk" && voice.phase !== "idle";

  useEffect(() => {
    let cancelled = false;
    void ipc.mcpList().then((servers) => {
      if (cancelled) return;
      setMcpServers(servers);
      const mcpDefaults: Record<string, boolean> = {};
      servers.forEach((s) => { mcpDefaults[s.config.id] = s.config.enabled; });
      resetSessionTools({
        webSearch: true,
        mcp: mcpDefaults,
      });
    }).catch(() => setMcpServers([]));
    return () => { cancelled = true; };
  }, [resetSessionTools]);

  useEffect(() => {
    if (!mcpOpen) return;
    const onDown = (e: MouseEvent) => {
      if (mcpRootRef.current && !mcpRootRef.current.contains(e.target as Node)) setMcpOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        setMcpOpen(false);
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [mcpOpen]);

  // Also re-measure when recording ends: an empty textarea is hidden while
  // recording, so a transcript lands in the draft while it has no height.
  useEffect(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${Math.min(ta.scrollHeight, 180)}px`;
  }, [draft, recording]);
  const canSend = (draft.trim().length > 0 || attachments.length > 0) && !recording;
  const submit = () => {
    if (canSend) void send(draft);
  };

  const micError = voice.error;
  return (
    <div className="composer-wrap">
      {voice.handsFree && (
        <div className="handsfree-bar" role="status">
          <span className={voice.phase === "transcribing" ? "spinner" : "rec-dot"} aria-hidden />
          <span>{voice.phase === "transcribing" ? t("voice.transcribing") : t("chat.handsFreeOn")}</span>
          <LevelMeter levels={voice.levels.slice(-16)} max={16} label={t("voice.level")} />
        </div>
      )}
      {micError && (
        <div className="voice-notice" role="alert">
          <span>{micError.code === "stt_unavailable" ? t("voice.setupNeeded") : errorMessage(micError.code)}</span>
          <span style={{ display: "flex", gap: 4 }}>
            {micError.code === "stt_unavailable" && (
              <button className="btn btn-sm" onClick={onVoiceSetup}>
                {t("voice.setupAction")}
              </button>
            )}
            <button className="btn btn-sm btn-ghost" onClick={voice.clearError}>
              {t("app.dismiss")}
            </button>
          </span>
        </div>
      )}
      {voice.lastTranscript && !micError && busy && (
        <div className="voice-notice">
          <span dir="auto">
            {t("chat.youSaid")} “{voice.lastTranscript}”
          </span>
        </div>
      )}
      {attachmentErrors.length > 0 && (
        <div className="voice-notice" role="alert">
          <span>{attachmentErrors.join(" · ")}</span>
          <button className="btn btn-sm btn-ghost" onClick={dismissAttachmentErrors}>
            {t("app.dismiss")}
          </button>
        </div>
      )}
      {voiceNotice && !micError && (
        <div className="voice-notice" role="status">
          <span>{voiceNotice}</span>
          <button className="btn btn-sm btn-ghost" onClick={() => setVoiceNotice(null)}>
            {t("app.dismiss")}
          </button>
        </div>
      )}
      {queue.map((q) => (
        <div key={q.id} className="voice-notice queued" role="status">
          <span className="queued-text" dir="auto">
            <Clock size={12} aria-hidden /> {t("chat.queued")}: {q.text || q.attachments.map((a) => a.name).join(", ")}
          </span>
          <button className="btn btn-sm btn-ghost" aria-label={t("chat.unqueue")} title={t("chat.unqueue")} onClick={() => removeQueued(q.id)}>
            <X size={12} />
          </button>
        </div>
      ))}
      <div className="composer">
        <AttachmentList items={attachments} onRemove={removeAttachment} />
        <label className="sr-only" htmlFor="composer-input">
          {t("chat.placeholder")}
        </label>
        <textarea
          id="composer-input"
          ref={taRef}
          rows={1}
          value={draft}
          hidden={recording && !draft}
          readOnly={recording}
          dir={draft ? textDir(draft) : "auto"}
          placeholder={t("chat.placeholder")}
          onChange={(e) => setDraft(e.target.value)}
          onPaste={(e) => {
            // Pasted files, and very long pasted text, become attachments.
            const data = e.clipboardData;
            const hasFiles = data.files.length > 0;
            const longText = pasteAsFileChars > 0 && data.getData("text/plain").length > pasteAsFileChars;
            if (!hasFiles && !longText) return;
            e.preventDefault();
            void attachFromPaste(data, pasteAsFileChars);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              submit();
            }
            if (e.key === "Escape" && busy) {
              e.stopPropagation();
              stop();
            }
          }}
        />
        {recording && (
          <div className="recording-row" role="status" aria-live="polite" title={voice.device ? t("call.usingMic", { device: voice.device }) : undefined}>
            {voice.phase === "listening" ? (
              <>
                <span className="rec-dot" aria-hidden />
                <span className="sr-only">{t("voice.listening")}</span>
                <VoiceBars bands={voice.bands} level={voice.level} label={t("voice.level")} />
                <button className="round-btn mic" aria-label={t("voice.cancel")} title={`${t("voice.cancel")} (Esc)`} onClick={() => void voice.stopPushToTalk(false)}>
                  <X size={16} />
                </button>
                <button className="round-btn send" aria-label={t("voice.stop")} title={t("voice.stop")} onClick={() => void voice.stopPushToTalk(true)} autoFocus>
                  <Square size={12} fill="currentColor" />
                </button>
              </>
            ) : (
              <>
                <span className="spinner" aria-hidden />
                <span className="recording-label">{t("voice.transcribing")}</span>
              </>
            )}
          </div>
        )}
        <div className="composer-bar">
          <ModelPicker autoModel={autoModel} />
          <button
            type="button"
            className="chip icon-chip"
            aria-label={t("chat.attach")}
            title={t("chat.attach")}
            onClick={() => void pickFiles().catch(() => undefined)}
          >
            <Paperclip size={14} />
          </button>
          <button
            type="button"
            className={`chip icon-chip${speak ? " on" : ""}`}
            aria-pressed={speak}
            aria-label={t("chat.voiceReplies")}
            title={t("chat.voiceReplies")}
            onClick={() => {
              if (speak) void ipc.ttsStop();
              void updateSettings((s) => void (s.tts.speakResponses = !speak));
            }}
          >
            {speak ? <Volume2 size={14} /> : <VolumeX size={14} />}
          </button>
          <button
            type="button"
            className={`chip icon-chip${voice.handsFree ? " on" : ""}`}
            aria-pressed={voice.handsFree}
            aria-label={t("chat.handsFree")}
            title={t("chat.handsFree")}
            onClick={() => void voice.toggleHandsFree()}
          >
            <AudioLines size={14} />
          </button>
          <button
            type="button"
            className={`chip icon-chip${sessionWebSearch ? " on" : ""}`}
            aria-pressed={sessionWebSearch}
            aria-label={t("chat.webSearch")}
            title={sessionWebSearch ? t("chat.webSearchOn") : t("chat.webSearchOff")}
            onClick={() => setSessionWebSearch(!sessionWebSearch)}
          >
            <Globe size={14} />
          </button>
          <span className="composer-sep" aria-hidden />
          <div className="picker" ref={mcpRootRef}>
            <button type="button" className={`chip icon-chip mcp-chip${mcpOpen ? " on" : ""}`} aria-haspopup="menu" aria-expanded={mcpOpen} aria-label={t("chat.mcpTools")} title={t("chat.mcpTools")} onClick={() => setMcpOpen((v) => !v)}>
              <Plus size={14} />
              {(() => {
                const total = mcpServers.length;
                const on = mcpServers.filter((s) => sessionMcpEnabled[s.config.id] ?? s.config.enabled).length;
                return on > 0 && total > 1 ? <span className="mcp-count">{on}/{total}</span> : null;
              })()}
              <ChevronDown size={10} aria-hidden className={mcpOpen ? "flip" : ""} />
            </button>
            {mcpOpen && (
              <div className="picker-menu narrow" role="menu" aria-label={t("chat.mcpTools")}>
                <div className="picker-head">
                  <span>{t("chat.mcpTools")}</span>
                </div>
                {mcpServers.length === 0 && <div className="picker-empty">{t("status.notInstalled")}</div>}
                {mcpServers.map((s) => {
                  const on = sessionMcpEnabled[s.config.id] ?? s.config.enabled;
                  return (
                    <button
                      type="button"
                      key={s.config.id}
                      role="menuitemcheckbox"
                      aria-checked={on}
                      className="picker-item"
                      onClick={() => setSessionMcpEnabled(s.config.id, !on)}
                    >
                      <span className={`picker-dot${on ? " on" : ""}`} aria-hidden />
                      <span className="picker-text">
                        <span className="picker-name">{s.config.name}</span>
                        <span className="picker-sub">{s.state}</span>
                      </span>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
          <span className="composer-spacer" />
          {!recording && (
            <button className="round-btn mic" aria-label={t("chat.microphone")} title={t("chat.microphone")} onClick={() => void voice.startPushToTalk()} disabled={voice.handsFree}>
              <Mic size={16} />
            </button>
          )}
          {busy && canSend && (
            <button className="round-btn send" aria-label={t("chat.queue")} title={t("chat.queue")} onClick={submit}>
              <ArrowUp size={17} strokeWidth={2.4} />
            </button>
          )}
          {recording ? null : busy ? (
            <button className="round-btn send stop" aria-label={t("chat.stop")} title={t("chat.stop")} onClick={stop}>
              <Square size={12} fill="currentColor" />
            </button>
          ) : (
            <button className="round-btn send" aria-label={t("chat.send")} title={t("chat.send")} onClick={submit} disabled={!canSend}>
              <ArrowUp size={17} strokeWidth={2.4} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
});
