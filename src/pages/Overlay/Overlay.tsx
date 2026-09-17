import { useEffect, useRef, useState } from "react";
import { CheckCircle2, Mic, XCircle } from "lucide-react";
import { on } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import type { DictationStateEvent, VoiceEvent } from "../../app/types";
import { textDir } from "../../components/common/controls";
import { LevelMeter } from "../../components/voice/LevelMeter";

/** Non-focusable window shown while dictating into another application.
 *  Displays what you say live, then confirms the insertion. */
export function Overlay() {
  const [state, setState] = useState<DictationStateEvent["state"]>("idle");
  const [error, setError] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [levels, setLevels] = useState<number[]>(new Array(18).fill(0));
  const mode = useSettings((s) => s.settings?.dictation.mode ?? "hold");
  const textRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    document.documentElement.classList.add("overlay");
    const subs = [
      on<DictationStateEvent>("dictation://state", (e) => {
        setState(e.state);
        if (e.state === "listening") {
          setText("");
          setError(null);
          setLevels(new Array(18).fill(0));
        }
        if (e.error) setError(errorMessage(e.error.code));
        if (e.result?.inserted) setText(e.result.inserted);
      }),
      on<VoiceEvent>("voice://event", (e) => {
        if (e.mode !== "dictation") return;
        if (e.type === "level") setLevels((l) => [...l.slice(1), e.value]);
        if (e.type === "partial") setText(e.text);
        if (e.type === "transcript" && e.text) setText(e.text);
      }),
    ];
    return () => subs.forEach((s) => void s.then((u) => u()));
  }, []);

  useEffect(() => {
    textRef.current?.scrollTo({ top: textRef.current.scrollHeight });
  }, [text]);

  const listening = state === "listening" || state === "idle";
  let icon = <Mic size={16} className="ic-listening" />;
  let label = t("overlay.speakNow");
  let hint = mode === "hold" ? t("overlay.hint") : t("overlay.hintToggle");
  switch (state) {
    case "transcribing":
      icon = <span className="spinner" />;
      label = t("overlay.transcribing");
      hint = "";
      break;
    case "correcting":
      icon = <span className="spinner" />;
      label = t("overlay.correcting");
      hint = "";
      break;
    case "inserted":
      icon = <CheckCircle2 size={16} style={{ color: "var(--success)" }} />;
      label = t("overlay.inserted");
      hint = "";
      break;
    case "empty":
      icon = <XCircle size={16} style={{ color: "var(--text-faint)" }} />;
      label = t("overlay.empty");
      hint = "";
      break;
    case "error":
      icon = <XCircle size={16} style={{ color: "var(--danger)" }} />;
      label = error ?? t("overlay.error");
      hint = "";
      break;
  }

  return (
    <div className={`overlay-card${state === "inserted" ? " done" : ""}`} role="status" aria-live="polite">
      <div className="overlay-head">
        {icon}
        <span className="overlay-label">{label}</span>
        {listening && <LevelMeter levels={levels} max={18} label={t("voice.level")} />}
        {hint && <span className="overlay-hint">{hint}</span>}
      </div>
      <div className={`overlay-text${text ? "" : " empty"}`} ref={textRef} dir={text ? textDir(text) : "auto"}>
        {text || t("overlay.listening")}
      </div>
    </div>
  );
}
