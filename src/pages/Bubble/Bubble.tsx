import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ipc, on } from "../../app/ipc";
import { t } from "../../app/strings";
import type { DictationStateEvent, TtsEvent, VoiceEvent } from "../../app/types";

type Activity = "idle" | "listening" | "speaking";

/** Floating launcher bubble. Click opens the chat; drag moves it. */
export function Bubble() {
  const [activity, setActivity] = useState<Activity>("idle");
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(null);

  useEffect(() => {
    document.documentElement.classList.add("launcher-window");
    const subs = [
      on<VoiceEvent>("voice://event", (e) => {
        if (e.type === "state") setActivity(e.state === "idle" ? "idle" : "listening");
      }),
      on<DictationStateEvent>("dictation://state", (e) => {
        if (e.state === "listening") setActivity("listening");
        else if (["inserted", "empty", "cancelled", "error", "idle"].includes(e.state)) setActivity("idle");
      }),
      on<TtsEvent>("tts://event", (e) => {
        if (e.type === "speaking") setActivity("speaking");
        if (e.type === "idle") setActivity("idle");
      }),
    ];
    return () => subs.forEach((s) => void s.then((u) => u()));
  }, []);

  const label = activity === "listening" ? t("bubble.listening") : activity === "speaking" ? t("bubble.speaking") : t("bubble.open");

  return (
    <div className="launcher-stage">
      <button
        type="button"
        className={`launcher ${activity}`}
        aria-label={label}
        title={label}
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          press.current = { x: e.screenX, y: e.screenY, dragging: false };
        }}
        onPointerMove={(e) => {
          const p = press.current;
          if (!p || p.dragging || (e.buttons & 1) === 0) return;
          if (Math.hypot(e.screenX - p.x, e.screenY - p.y) > 4) {
            p.dragging = true;
            void getCurrentWindow().startDragging();
          }
        }}
        onPointerUp={() => {
          const p = press.current;
          press.current = null;
          if (p && !p.dragging) void ipc.bubbleOpenChat();
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") void ipc.bubbleOpenChat();
        }}
      >
        <span className="launcher-ring" aria-hidden />
        <svg className="launcher-glyph" viewBox="0 0 24 24" aria-hidden>
          <path d="M4 12h1.5M8 8.5v7M11.5 5.5v13M15 8.5v7M18.5 11v2" />
        </svg>
      </button>
    </div>
  );
}
