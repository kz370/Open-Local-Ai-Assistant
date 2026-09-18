import { useEffect, useRef, useState, type CSSProperties, type MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { GripVertical, Settings2, X } from "lucide-react";
import { ipc, on } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import type { CaptionEvent, CaptionSettings } from "../../app/types";
import { textDir } from "../../components/common/controls";

/** Lines older than this are dropped once nothing new has been said. */
const CLEAR_AFTER_MS = 7000;
const KEEP_LINES = 12;

/** "#rrggbb" + opacity -> rgba(). */
function rgba(hex: string, alpha: number): string {
  const n = parseInt(hex.slice(1), 16);
  if (Number.isNaN(n)) return `rgba(0, 0, 0, ${alpha})`;
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

export function captionStyle(c: CaptionSettings): CSSProperties {
  return {
    color: c.textColor,
    background: rgba(c.backgroundColor, c.backgroundOpacity),
    fontSize: c.fontSize,
    fontWeight: c.fontWeight,
    fontStyle: c.italic ? "italic" : "normal",
    ["--caption-lines" as string]: c.maxLines,
  };
}

/** Always-on-top subtitle bar for whatever the PC is playing. */
export function Captions() {
  const captions = useSettings((s) => s.settings!.captions);
  const [lines, setLines] = useState<string[]>([]);
  const [partial, setPartial] = useState("");
  const [status, setStatus] = useState<"starting" | "listening" | "idle" | "error">("starting");
  const [error, setError] = useState<string | null>(null);
  const clearTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    document.documentElement.classList.add("captions");
    // Fade out stale text a while after the audio went quiet.
    const touch = () => {
      window.clearTimeout(clearTimer.current);
      clearTimer.current = window.setTimeout(() => {
        setLines([]);
        setPartial("");
      }, CLEAR_AFTER_MS);
    };
    const sub = on<CaptionEvent>("captions://event", (e) => {
      switch (e.type) {
        case "state":
          if (e.state === "starting") {
            setLines([]);
            setPartial("");
            setError(null);
          }
          // An error stays on screen after its session ends.
          setStatus((prev) => (e.state === "idle" && prev === "error" ? prev : e.state));
          break;
        case "partial":
          setPartial(e.text);
          touch();
          break;
        case "line":
          setLines((l) => [...l, e.text].slice(-KEEP_LINES));
          setPartial("");
          touch();
          break;
        case "error":
          setStatus("error");
          setError(errorMessage(e.code));
          break;
      }
    });
    void ipc.captionsStatus().then((active) => {
      if (!active) setStatus((prev) => (prev === "starting" ? "idle" : prev));
    });
    return () => {
      window.clearTimeout(clearTimer.current);
      void sub.then((u) => u());
    };
  }, []);

  const shown = partial ? [...lines, partial] : lines;
  let placeholder = t("captions.waiting");
  if (status === "starting") placeholder = t("captions.starting");
  if (status === "idle") placeholder = t("captions.off");
  if (status === "error") placeholder = error ?? t("captions.error");

  const drag = (e: MouseEvent) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    void getCurrentWindow().startDragging();
  };

  return (
    <div className="caption-bar" style={captionStyle(captions)} onMouseDown={drag}>
      <div className="caption-tools">
        <GripVertical size={14} aria-label={t("captions.move")} />
        <span style={{ flex: 1 }} />
        <button type="button" className="caption-btn" aria-label={t("captions.settings")} title={t("captions.settings")} onClick={() => void ipc.openSettings("captions")}>
          <Settings2 size={14} />
        </button>
        <button type="button" className="caption-btn" aria-label={t("captions.close")} title={t("captions.close")} onClick={() => void ipc.captionsStop()}>
          <X size={14} />
        </button>
      </div>
      <div className="caption-text" role="log" aria-live="polite">
        {shown.length === 0 ? (
          <p className={`caption-line placeholder${status === "error" ? " err" : ""}`}>{placeholder}</p>
        ) : (
          shown.map((line, i) => (
            <p key={`${i}-${line.length}`} className={`caption-line${partial && i === shown.length - 1 ? " partial" : ""}`} dir={textDir(line)}>
              {line}
            </p>
          ))
        )}
      </div>
      <button
        type="button"
        className="caption-resize"
        aria-label={t("captions.resize")}
        title={t("captions.resize")}
        onMouseDown={(e) => {
          e.stopPropagation();
          void getCurrentWindow().startResizeDragging("SouthEast");
        }}
      />
    </div>
  );
}
