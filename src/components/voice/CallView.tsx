import { useEffect, useMemo, useRef, useState } from "react";
import { Mic, MicOff, Pause, PhoneOff, Play, Square } from "lucide-react";
import { useChat } from "../../app/chatStore";
import { ipc } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { t } from "../../app/strings";
import { useVoice, type SpokenSentence } from "../../app/voiceStore";
import { BrandMark } from "../common/BrandMark";
import { textDir } from "../common/controls";
import { LevelMeter } from "./LevelMeter";
import { SpokenText } from "./SpokenText";
import { stripSoundTags } from "../../app/soundTags";

function elapsed(startedAt: number): string {
  const s = Math.max(0, Math.floor((Date.now() - startedAt) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** How long the last sentence stays up after playback goes idle once the
 *  reply has finished generating (the remaining sentences may still be
 *  synthesizing). */
const HOLD_AFTER_REPLY_MS = 2500;

/** The last spoken sentence, kept while playback waits for the next one. */
function useHeldSentence(spoken: SpokenSentence | null, turnId: string | null, userTalking: boolean): SpokenSentence | null {
  const busy = turnId !== null;
  const last = useRef<SpokenSentence | null>(null);
  const [expired, setExpired] = useState(false);
  if (spoken) last.current = spoken;
  useEffect(() => {
    setExpired(false);
    if (spoken || busy) return;
    const id = setTimeout(() => setExpired(true), HOLD_AFTER_REPLY_MS);
    return () => clearTimeout(id);
  }, [spoken, busy]);
  if (spoken || userTalking || expired) return null;
  // A new turn must not show the previous turn's last sentence while it thinks.
  if (busy && last.current?.tag !== turnId) return null;
  return last.current;
}

/** Phone-call style screen for hands-free conversation. */
export function CallView() {
  const voice = useVoice();
  const messages = useChat((s) => s.messages);
  const turnId = useChat((s) => s.turnId);
  const busy = turnId !== null;
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

  // A new call starts with a clean screen: messages from before it, and the
  // previous call's transcript, are not this call's captions.
  const [before] = useState(() => new Set(useChat.getState().messages.map((m) => m.id)));
  useEffect(() => useVoice.setState({ lastTranscript: null, partial: "" }), []);
  const callMessages = useMemo(() => messages.filter((m) => !before.has(m.id)), [messages, before]);
  const lastUser = useMemo(() => [...callMessages].reverse().find((m) => m.role === "user" && m.content), [callMessages]);
  const voiceNotice = useChat((s) => s.voiceNotice);
  // Playback goes idle between sentences whenever the next one is not ready
  // yet (the model is still writing it, or it is still being synthesized).
  // Keep the last sentence on screen through that gap instead of flashing the
  // whole reply, until this turn ends or the user starts talking.
  const held = useHeldSentence(voice.spoken, turnId, !!voice.partial || voice.phase === "transcribing");
  const sentence = voice.spoken ?? held;
  const state = voice.speaking || sentence ? "speaking" : busy ? "thinking" : voice.phase === "transcribing" ? "transcribing" : "listening";
  // The assistant can be cut off whenever it is talking or about to talk.
  const canInterrupt = voice.speaking || voice.paused || busy;
  // The reply being spoken (its sentences are tagged with its turn), or the
  // one being written right now.
  const reply = useMemo(
    () => [...callMessages].reverse().find((m) => m.role === "assistant" && m.content && (m.turnId === (sentence?.tag ?? turnId) || !m.turnId)),
    [callMessages, sentence?.tag, turnId],
  );
  const showReply = !!reply && (state === "speaking" || (busy && reply.turnId === turnId));
  const thinking = state === "thinking" || state === "transcribing";
  const label = voice.muted ? t("call.muted") : t(`call.${state}`);
  // The reply itself is shown by SpokenText; never dump it here in full.
  const caption = voice.partial || (voice.lastTranscript ?? "");

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
        {thinking && !voice.muted && (
          <span className="thinking-dots" aria-hidden>
            <i />
            <i />
            <i />
          </span>
        )}
      </div>
      {/* While the assistant works the microphone level means nothing: the
          glow and the dots show progress instead, in the same space. */}
      {thinking ? <div className="call-meter-gap" aria-hidden /> : <LevelMeter levels={voice.levels} max={44} label={t("voice.level")} />}
      {showReply && reply ? (
        <SpokenText text={stripSoundTags(reply.content)} sentence={sentence && sentence.tag === reply.turnId ? sentence : null} paused={voice.paused} />
      ) : (
        <div className="call-caption" dir={caption ? textDir(caption) : "auto"} lang={caption && textDir(caption) === "rtl" ? "ar" : undefined}>
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
          aria-label={t("call.interrupt")}
          title={t("call.interrupt")}
          disabled={!canInterrupt}
          onClick={() => voice.interrupt()}
        >
          <Square size={17} fill="currentColor" />
        </button>
        {/* Pause keeps the rest of the answer queued; while paused the
            microphone listens again, so speaking up still takes over. */}
        <button
          className={`call-btn${voice.paused ? " muted" : ""}`}
          aria-label={voice.paused ? t("chat.resume") : t("chat.pause")}
          title={voice.paused ? t("chat.resume") : t("chat.pause")}
          aria-pressed={voice.paused}
          disabled={!voice.speaking && !voice.paused}
          onClick={() => void ipc.ttsSetPaused(!voice.paused)}
        >
          {voice.paused ? <Play size={20} /> : <Pause size={20} />}
        </button>
        <button className="call-btn end" aria-label={t("call.end")} title={t("call.end")} onClick={() => void voice.toggleHandsFree()}>
          <PhoneOff size={22} />
        </button>
        <button
          className={`call-btn${voice.muted ? " muted" : ""}`}
          aria-label={voice.muted ? t("call.unmute") : t("call.mute")}
          title={voice.muted ? t("call.unmute") : t("call.mute")}
          aria-pressed={voice.muted}
          onClick={() => void voice.toggleMuted()}
        >
          {voice.muted ? <MicOff size={20} /> : <Mic size={20} />}
        </button>
      </div>
    </div>
  );
}
