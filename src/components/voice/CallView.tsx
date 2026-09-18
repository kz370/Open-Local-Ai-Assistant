import { useEffect, useMemo, useState } from "react";
import { Mic, Pause, PhoneOff, Play } from "lucide-react";
import { useChat } from "../../app/chatStore";
import { ipc } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { t } from "../../app/strings";
import { useVoice } from "../../app/voiceStore";
import { BrandMark } from "../common/BrandMark";
import { textDir } from "../common/controls";
import { LevelMeter } from "./LevelMeter";
import { SpeechTicker } from "./SpeechTicker";

function elapsed(startedAt: number): string {
  const s = Math.max(0, Math.floor((Date.now() - startedAt) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Phone-call style screen for hands-free conversation. */
export function CallView() {
  const voice = useVoice();
  const messages = useChat((s) => s.messages);
  const busy = useChat((s) => s.turnId !== null);
  const stop = useChat((s) => s.stop);
  const [startedAt] = useState(() => Date.now());
  const micName = useVoice((s) => s.device);
  const silent = useVoice((s) => s.levels.every((v) => v < 0.03));
  const openSpeechSettings = () => void ipc.openSettings("speech");
  useSettings((s) => s.settings?.stt.microphone); // re-render when the device changes
  const [, tick] = useState(0);

  useEffect(() => {
    const id = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, []);

  const lastAssistant = useMemo(() => [...messages].reverse().find((m) => m.role === "assistant" && m.content), [messages]);
  const lastUser = useMemo(() => [...messages].reverse().find((m) => m.role === "user" && m.content), [messages]);
  const voiceNotice = useChat((s) => s.voiceNotice);
  const state = voice.speaking ? "speaking" : busy ? "thinking" : voice.phase === "transcribing" ? "transcribing" : "listening";
  const label = t(`call.${state}`);
  const caption = voice.partial || (state === "speaking" || state === "thinking" ? lastAssistant?.content ?? "" : voice.lastTranscript ?? "");

  return (
    <div className={`call ${state}`} role="region" aria-label={t("call.title")}>
      <div className="call-top">
        <span className="call-timer">{elapsed(startedAt)}</span>
      </div>
      <div className="call-avatar-wrap">
        <span className={`call-pulse ${state}`} aria-hidden />
        <BrandMark size={96} />
      </div>
      <div className="call-state" aria-live="polite">
        {label}
      </div>
      <LevelMeter levels={voice.levels} max={44} label={t("voice.level")} />
      {voice.spoken && state === "speaking" ? (
        <SpeechTicker sentence={voice.spoken} paused={voice.paused} />
      ) : (
        <div className="call-caption" dir={caption ? textDir(caption) : "auto"}>
          {caption || t("call.saySomething")}
        </div>
      )}
      {voiceNotice && (
        <div className="notice warn" style={{ maxWidth: 340 }} role="alert">
          <span style={{ flex: 1 }}>{voiceNotice}</span>
          <button className="btn btn-sm" onClick={() => void ipc.openSettings("voice")}>
            {t("app.openSettings")}
          </button>
        </div>
      )}
      {lastUser && state !== "listening" && (
        <div className="call-you" dir={textDir(lastUser.content, lastUser.language)}>
          {t("chat.youSaid")} “{lastUser.content}”
        </div>
      )}
      <div className="call-device">
        {micName ? t("call.usingMic", { device: micName }) : ""}
        {silent && (
          <button className="btn btn-sm" onClick={openSpeechSettings}>
            {t("call.changeMic")}
          </button>
        )}
      </div>
      <div className="call-actions">
        <button
          className="call-btn"
          aria-label={voice.paused ? t("chat.resume") : t("call.interrupt")}
          title={voice.paused ? t("chat.resume") : t("call.interrupt")}
          onClick={() => {
            if (voice.paused) {
              void ipc.ttsSetPaused(false);
            } else if (voice.speaking) {
              void ipc.ttsSetPaused(true);
            } else {
              void ipc.ttsStop();
              if (busy) stop();
            }
          }}
        >
          {voice.paused ? <Play size={20} /> : <Pause size={20} />}
        </button>
        <button className="call-btn end" aria-label={t("call.end")} title={t("call.end")} onClick={() => void voice.toggleHandsFree()}>
          <PhoneOff size={22} />
        </button>
        <button className="call-btn" aria-label={t("call.listening")} title={t("call.listening")} disabled>
          <Mic size={20} />
        </button>
      </div>
    </div>
  );
}
