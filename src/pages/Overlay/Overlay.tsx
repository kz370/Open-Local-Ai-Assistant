import { useEffect, useRef, useState } from "react";
import { Check, CheckCircle2, ChevronDown, Globe, Mic, Pencil, RotateCcw, X, XCircle } from "lucide-react";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { ipc, on } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import type { DictationStateEvent, LanguageEntry, VoiceEvent } from "../../app/types";
import { textDir } from "../../components/common/controls";
import { LevelMeter } from "../../components/voice/LevelMeter";

/** Free drag on the overlay's header strip; Shift locks to horizontal-only,
 * Alt locks to vertical-only. Uses manual `setPosition` (not Tauri's
 * `startDragging`, which hands off to the OS and can't be axis-constrained
 * mid-drag), throttled to one IPC call per frame. */
function useOverlayDrag() {
  const drag = useRef<{ startX: number; startY: number; winX: number; winY: number; frame: number | null; pending: { x: number; y: number } | null } | null>(null);

  const flush = () => {
    const d = drag.current;
    if (!d) return;
    d.frame = null;
    if (!d.pending) return;
    const { x, y } = d.pending;
    d.pending = null;
    void getCurrentWindow().setPosition(new PhysicalPosition(x, y));
  };

  return {
    onPointerDown: (e: React.PointerEvent) => {
      if (e.button !== 0) return;
      // Without capture, the browser stops delivering move events to this
      // element the instant the (async) window move puts the cursor even
      // slightly outside its bounds — the drag would visually never start.
      e.currentTarget.setPointerCapture(e.pointerId);
      void getCurrentWindow()
        .outerPosition()
        .then((pos) => {
          drag.current = { startX: e.screenX, startY: e.screenY, winX: pos.x, winY: pos.y, frame: null, pending: null };
        });
    },
    onPointerMove: (e: React.PointerEvent) => {
      const d = drag.current;
      if (!d || (e.buttons & 1) === 0) return;
      // screenX/screenY are logical (CSS) pixels; PhysicalPosition wants
      // real screen pixels, so scale by the monitor's DPI factor.
      const scale = window.devicePixelRatio || 1;
      let dx = Math.round((e.screenX - d.startX) * scale);
      let dy = Math.round((e.screenY - d.startY) * scale);
      if (e.shiftKey) dy = 0;
      if (e.altKey) dx = 0;
      d.pending = { x: d.winX + dx, y: d.winY + dy };
      if (d.frame == null) d.frame = requestAnimationFrame(flush);
    },
    onPointerUp: (e: React.PointerEvent) => {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
      drag.current = null;
    },
  };
}

/** Small header button. Stops the pointer so pressing it never starts a drag. */
function HeadBtn(props: { label: string; onClick: () => void; children: React.ReactNode; disabled?: boolean }) {
  return (
    <button
      type="button"
      className="icon-btn"
      style={{ width: 24, height: 24 }}
      aria-label={props.label}
      title={props.label}
      disabled={props.disabled}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={props.onClick}
    >
      {props.children}
    </button>
  );
}

const quiet = () => undefined;

/** Language pill with a scrolling list that drops over the text. A native
 *  select's popup is clipped by the small overlay window and can't be styled. */
function LanguageMenu(props: { value: string; languages: LanguageEntry[]; onChange: (code: string) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    menuRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
    const onDown = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const nameOf = (e: LanguageEntry) => (e.builtIn ? t(`languages.${e.code}`) : e.displayName);
  const options = [{ code: "auto", short: t("overlay.auto"), name: t("app.automatic") }, ...props.languages.map((e) => ({ code: e.code, short: e.code.toUpperCase(), name: nameOf(e) }))];
  const current = options.find((o) => o.code === props.value) ?? options[0];

  return (
    <div ref={ref} className="overlay-lang-wrap" onPointerDown={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}>
      <button
        type="button"
        className={`overlay-lang${open ? " open" : ""}`}
        title={`${t("overlay.language")}: ${current.name}`}
        aria-label={`${t("overlay.language")}: ${current.name}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <Globe size={12} aria-hidden />
        {current.short}
        <ChevronDown size={12} aria-hidden className="overlay-lang-chevron" />
      </button>
      {open && (
        <div ref={menuRef} className="overlay-lang-menu" role="listbox" aria-label={t("overlay.language")}>
          {options.map((o) => (
            <button
              key={o.code}
              type="button"
              role="option"
              aria-selected={o.code === props.value}
              className="overlay-lang-option"
              title={o.name}
              onClick={() => {
                setOpen(false);
                if (o.code !== props.value) props.onChange(o.code);
              }}
            >
              <span className="overlay-lang-check" aria-hidden>
                {o.code === props.value && <Check size={11} />}
              </span>
              <span className="overlay-lang-code">{o.short}</span>
              <span className="overlay-lang-name">{o.name}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** Window shown while dictating into another application. Displays what you
 *  say live, then confirms the insertion. In review mode it holds the result
 *  as editable text until you insert or discard it. */
export function Overlay() {
  const [state, setState] = useState<DictationStateEvent["state"]>("idle");
  const [error, setError] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [draft, setDraft] = useState("");
  const [levels, setLevels] = useState<number[]>(new Array(18).fill(0));
  const mode = useSettings((s) => s.settings?.dictation.mode ?? "hold");
  const textRef = useRef<HTMLDivElement>(null);
  const editRef = useRef<HTMLTextAreaElement>(null);
  const drag = useOverlayDrag();

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
        if (e.state === "review") setDraft(e.result?.inserted ?? "");
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

  const reviewing = state === "review";
  useEffect(() => {
    if (!reviewing) return;
    // The window only just became focusable; give it a beat before typing focus.
    const id = setTimeout(() => {
      const el = editRef.current;
      if (!el) return;
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    }, 60);
    return () => clearTimeout(id);
  }, [reviewing]);

  const listening = state === "listening" || state === "idle";
  let icon = (
    <span className="overlay-rec" aria-hidden>
      <Mic size={12} />
    </span>
  );
  let label = t("overlay.listeningLabel");
  switch (state) {
    case "transcribing":
      icon = <span className="spinner" />;
      label = t("overlay.transcribing");
      break;
    case "correcting":
      icon = <span className="spinner" />;
      label = t("overlay.correcting");
      break;
    case "review":
      icon = <Pencil size={16} style={{ color: "var(--accent)" }} />;
      label = t("overlay.review");
      break;
    case "inserted":
      icon = <CheckCircle2 size={16} style={{ color: "var(--success)" }} />;
      label = t("overlay.inserted");
      break;
    case "empty":
      icon = <XCircle size={16} style={{ color: "var(--text-faint)" }} />;
      label = t("overlay.empty");
      break;
    case "cancelled":
      icon = <XCircle size={16} style={{ color: "var(--text-faint)" }} />;
      label = t("overlay.cancelled");
      break;
    case "error":
      icon = <XCircle size={16} style={{ color: "var(--danger)" }} />;
      label = error ?? t("overlay.error");
      break;
  }

  const keysHint = mode === "toggle" ? t("overlay.hintToggle") : t("overlay.hint");
  const language = useSettings((st) => st.settings?.dictation.language || st.settings?.stt.language || "auto");
  const languages = useSettings((st) => st.settings?.language.entries ?? []);
  const changeLanguage = (code: string) => {
    void ipc
      .dictationSetLanguage(code)
      // Mid-recording the recognizer is already built: restart on the new language.
      .then(() => (listening ? ipc.dictationRetry() : undefined))
      .catch(quiet);
  };
  const confirm = () => void ipc.dictationConfirm(draft).catch(quiet);

  return (
    <div className={`overlay-card${state === "inserted" ? " done" : ""}`} role="status" aria-live="polite">
      {/* Header: what is happening, the language, and close. Drag to move;
          double-click puts the overlay back in its default spot. */}
      <div
        className="overlay-head"
        title={t("overlay.dragHint")}
        onPointerDown={drag.onPointerDown}
        onPointerMove={drag.onPointerMove}
        onPointerUp={drag.onPointerUp}
        onDoubleClick={() => void ipc.dictationResetOverlayPosition().catch(quiet)}
      >
        <span className="overlay-icon">{icon}</span>
        <span className="overlay-label">{label}</span>
        {listening && <LevelMeter levels={levels} max={14} label={t("voice.level")} />}
        <span style={{ flex: 1 }} />
        {(listening || reviewing) && <LanguageMenu value={language} languages={languages} onChange={changeLanguage} />}
        <HeadBtn label={`${t("voice.cancel")} (Esc)`} onClick={() => void ipc.dictationCancel().catch(quiet)}>
          <X size={14} />
        </HeadBtn>
      </div>
      {reviewing ? (
        <>
          <textarea
            ref={editRef}
            className="overlay-edit"
            value={draft}
            dir={draft ? textDir(draft) : "auto"}
            spellCheck
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                confirm();
              } else if (e.key === "Escape") {
                e.preventDefault();
                void ipc.dictationCancel().catch(quiet);
              }
            }}
          />
          <div className="overlay-actions">
            <span className="overlay-hint">{t("overlay.insertHint")}</span>
            <button type="button" className="btn btn-sm btn-ghost" title={t("overlay.retryHint")} onClick={() => void ipc.dictationRetry().catch(quiet)}>
              <RotateCcw size={13} /> {t("overlay.retry")}
            </button>
            <button type="button" className="btn btn-sm btn-primary" onClick={confirm}>
              <Check size={13} /> {t("overlay.insert")}
            </button>
          </div>
        </>
      ) : (
        <>
          <div className={`overlay-text${text ? "" : " empty"}`} ref={textRef} dir={text ? textDir(text) : "auto"}>
            {text || t("overlay.speakNow")}
          </div>
          {listening && (
            <div className="overlay-actions">
              <span className="overlay-hint">{keysHint}</span>
              <button type="button" className="btn btn-sm btn-ghost" title={t("overlay.retryHint")} onClick={() => void ipc.dictationRetry().catch(quiet)}>
                <RotateCcw size={13} /> {t("overlay.restart")}
              </button>
              <button type="button" className="btn btn-sm btn-primary" title={t("overlay.insertNowHint")} onClick={() => void ipc.dictationInsertNow().catch(quiet)}>
                <Check size={13} /> {t("overlay.insert")}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
