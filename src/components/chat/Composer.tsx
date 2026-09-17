import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { ArrowUp, AudioLines, Mic, Square, Volume2, VolumeX } from "lucide-react";
import { useChat } from "../../app/chatStore";
import { ipc } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import { useVoice } from "../../app/voiceStore";
import { textDir } from "../common/controls";
import { LevelMeter } from "../voice/LevelMeter";
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
  const voiceNotice = useChat((s) => s.voiceNotice);
  const setVoiceNotice = useChat((s) => s.setVoiceNotice);
  const voice = useVoice();
  const speak = useSettings((s) => s.settings?.tts.speakResponses ?? false);
  const updateSettings = useSettings((s) => s.update);
  const taRef = useRef<HTMLTextAreaElement>(null);

  useImperativeHandle(ref, () => ({ focus: () => taRef.current?.focus() }));

  useEffect(() => {
    const ta = taRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = `${Math.min(ta.scrollHeight, 180)}px`;
  }, [draft]);

  const recording = voice.mode === "pushToTalk" && voice.phase !== "idle";
  const submit = () => {
    if (draft.trim() && !recording) void send(draft);
  };

  if (recording) {
    return (
      <div className="composer-wrap">
        <div className="recording" role="status" aria-live="polite">
          <div className="recording-state">
            {voice.phase === "listening" ? (
              <>
                <span className="rec-dot" aria-hidden /> {t("voice.listening")}
              </>
            ) : (
              <>
                <span className="spinner" aria-hidden /> {t("voice.transcribing")}
              </>
            )}
          </div>
          <LevelMeter levels={voice.levels} label={t("voice.level")} />
          {voice.phase === "listening" && (
            <div className="recording-actions">
              <button className="btn btn-primary" onClick={() => void voice.stopPushToTalk(true)} autoFocus>
                <Square size={12} fill="currentColor" /> {t("voice.stop")}
              </button>
              <button className="btn" onClick={() => void voice.stopPushToTalk(false)}>
                {t("voice.cancel")}
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

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
              {t("app.close")}
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
      {voiceNotice && !micError && (
        <div className="voice-notice" role="status">
          <span>{voiceNotice}</span>
          <button className="btn btn-sm btn-ghost" onClick={() => setVoiceNotice(null)}>
            {t("app.close")}
          </button>
        </div>
      )}
      <div className="composer">
        <label className="sr-only" htmlFor="composer-input">
          {t("chat.placeholder")}
        </label>
        <textarea
          id="composer-input"
          ref={taRef}
          rows={1}
          value={draft}
          dir={draft ? textDir(draft) : "auto"}
          placeholder={t("chat.placeholder")}
          onChange={(e) => setDraft(e.target.value)}
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
        <div className="composer-bar">
          <ModelPicker autoModel={autoModel} />
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
          <span className="composer-spacer" />
          <button className="round-btn mic" aria-label={t("chat.microphone")} title={t("chat.microphone")} onClick={() => void voice.startPushToTalk()} disabled={voice.handsFree}>
            <Mic size={16} />
          </button>
          {busy ? (
            <button className="round-btn send stop" aria-label={t("chat.stop")} title={t("chat.stop")} onClick={stop}>
              <Square size={12} fill="currentColor" />
            </button>
          ) : (
            <button className="round-btn send" aria-label={t("chat.send")} title={t("chat.send")} onClick={submit} disabled={!draft.trim()}>
              <ArrowUp size={17} strokeWidth={2.4} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
});
