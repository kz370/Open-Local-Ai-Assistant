import { useEffect, useState } from "react";
import { CheckCircle2, Mic, XCircle } from "lucide-react";
import { on } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import type { DictationStateEvent, VoiceEvent } from "../../app/types";
import { LevelMeter } from "../../components/voice/LevelMeter";

/** Non-focusable pill shown while dictating into another application. */
export function Overlay() {
  const [state, setState] = useState<DictationStateEvent>({ state: "idle" });
  const [levels, setLevels] = useState<number[]>(new Array(20).fill(0));
  const mode = useSettings((s) => s.settings?.dictation.mode ?? "hold");

  useEffect(() => {
    document.documentElement.classList.add("overlay");
    const subs = [
      on<DictationStateEvent>("dictation://state", (e) => {
        setState(e);
        if (e.state === "listening") setLevels(new Array(20).fill(0));
      }),
      on<VoiceEvent>("voice://event", (e) => {
        if (e.type === "level" && e.mode === "dictation") setLevels((l) => [...l.slice(1), e.value]);
      }),
    ];
    return () => subs.forEach((s) => void s.then((u) => u()));
  }, []);

  let icon = <Mic size={16} style={{ color: "var(--danger)" }} />;
  let text = mode === "hold" ? t("overlay.listening") : t("overlay.listeningToggle");
  switch (state.state) {
    case "transcribing":
      icon = <span className="spinner" />;
      text = t("overlay.transcribing");
      break;
    case "correcting":
      icon = <span className="spinner" />;
      text = t("overlay.correcting");
      break;
    case "inserted":
      icon = <CheckCircle2 size={16} style={{ color: "var(--success)" }} />;
      text = t("overlay.inserted");
      break;
    case "empty":
      icon = <XCircle size={16} style={{ color: "var(--text-faint)" }} />;
      text = t("overlay.empty");
      break;
    case "error":
      icon = <XCircle size={16} style={{ color: "var(--danger)" }} />;
      text = state.error ? errorMessage(state.error.code) : t("overlay.error");
      break;
  }

  return (
    <div className="overlay-pill" role="status" aria-live="assertive">
      {icon}
      <span style={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{text}</span>
      {(state.state === "listening" || state.state === "idle") && <LevelMeter levels={levels} max={22} label={t("voice.level")} />}
    </div>
  );
}
