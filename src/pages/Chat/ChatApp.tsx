import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { History, Minus, Settings2, SquarePen, Upload, X } from "lucide-react";
import { attachPaths } from "../../app/attach";
import { useChat } from "../../app/chatStore";
import { ipc, on } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { modelLabel, t } from "../../app/strings";
import type { DictationStateEvent } from "../../app/types";
import { useVoice } from "../../app/voiceStore";
import { Composer, type ComposerHandle } from "../../components/chat/Composer";
import { MessageBubble } from "../../components/chat/MessageBubble";
import { ToolConfirmDialog } from "../../components/chat/ToolConfirmDialog";
import { BrandMark } from "../../components/common/BrandMark";
import { CallView } from "../../components/voice/CallView";
import { textDir } from "../../components/common/controls";
import { HistoryPanel } from "../../components/history/HistoryPanel";

type LmState = "checking" | "connected" | "unavailable";

const SUGGESTIONS = ["chat.suggestion1", "chat.suggestion2", "chat.suggestion3", "chat.suggestion4"];

export function ChatApp() {
  const settings = useSettings((s) => s.settings);
  const messages = useChat((s) => s.messages);
  const busy = useChat((s) => s.turnId !== null);
  const connectionError = useChat((s) => s.connectionError);
  const newConversation = useChat((s) => s.newConversation);
  const retryLast = useChat((s) => s.retryLast);
  const send = useChat((s) => s.send);
  const [showHistory, setShowHistory] = useState(false);
  const [lm, setLm] = useState<LmState>("checking");
  const [autoModel, setAutoModel] = useState<string | null>(null);
  // Morph veil: bubble <-> chat continuity. Starts closed (veiled) so first
  // paint never flashes content before window-shown event.
  const [morph, setMorph] = useState<{ phase: "closed" | "opening" | "idle" | "closing"; fx: number; fy: number }>({ phase: "closed", fx: 396, fy: 616 });
  const morphTimer = useRef(0);
  const lastOpenRef = useRef(0);
  // voiceStore ignores dictation events, so track it here for Esc-cancel.
  const [dictating, setDictating] = useState(false);
  const [dropping, setDropping] = useState(false);
  const composerRef = useRef<ComposerHandle>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);

  const handsFree = useVoice((s) => s.handsFree);
  const voiceMode = useVoice((s) => s.mode);
  const voicePhase = useVoice((s) => s.phase);
  const stopPtt = useVoice((s) => s.stopPushToTalk);
  const recording = voiceMode === "pushToTalk" && voicePhase !== "idle";
  const callView = (settings?.stt.callView ?? true) && handsFree;
  const developer = settings?.general.developerMode ?? false;
  const compact = settings?.general.compact ?? false;
  const aliases = settings?.ai.modelAliases;
  const activeModel = settings?.ai.modelMode === "manual" ? settings.ai.model : autoModel;

  const provider = settings?.ai.provider;
  const hasKey = !!settings?.ai.apiKey?.trim();
  const needsKey = !!provider && provider !== "lmstudio" && !hasKey;

  const checkLm = useCallback(async () => {
    if (needsKey) {
      setLm("unavailable");
      return;
    }
    try {
      await ipc.lmstudioTest();
      setLm("connected");
      setAutoModel((await ipc.lmstudioAutoSelection())?.modelId ?? null);
    } catch {
      setLm("unavailable");
    }
  }, [needsKey]);

  useEffect(() => {
    void checkLm();
    const id = setInterval(() => {
      if (document.visibilityState === "visible") void checkLm();
    }, 20000);
    const onFocus = () => void checkLm();
    window.addEventListener("focus", onFocus);
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", onFocus);
    };
  }, [checkLm, settings?.ai.serverUrl, provider]);

  useEffect(() => {
    if (connectionError) setLm("unavailable");
  }, [connectionError]);

  // A preset window position is locked; only "custom" can be dragged. Tauri
  // starts a window drag from a document-level mousedown on any
  // [data-tauri-drag-region]; a window capture listener runs first, so
  // stopping the event there keeps the window where it is.
  const locked = (settings?.general.windowPosition ?? "custom") !== "custom";
  useEffect(() => {
    if (!locked) return;
    const block = (e: MouseEvent) => {
      if (e.target instanceof Element && e.target.hasAttribute("data-tauri-drag-region")) e.stopPropagation();
    };
    window.addEventListener("mousedown", block, true);
    return () => window.removeEventListener("mousedown", block, true);
  }, [locked]);

  useEffect(() => {
    // Self-heal: missed window-shown (first mount race) leaves veil stuck.
    getCurrentWindow()
      .isVisible()
      .then((v) => {
        if (v) setMorph((m) => (m.phase === "closed" ? { ...m, phase: "idle" } : m));
      })
      .catch(() => undefined);
    const later = (ms: number, fn: () => void) => {
      window.clearTimeout(morphTimer.current);
      morphTimer.current = window.setTimeout(fn, ms);
    };
    const subs = [
      on("app://focus-input", () => {
        setShowHistory(false);
        composerRef.current?.focus();
      }),
      on<{ origin?: string; animated?: boolean; fx?: number; fy?: number }>("app://window-shown", (p) => {
        // GPU shell zoom in webview; native frame already snapped to anchor.
        // Dedupe: rapid double events replay zoom (reads as happening twice).
        if (p?.animated && Number.isFinite(p.fx) && Number.isFinite(p.fy)) {
          const now = Date.now();
          if (now - lastOpenRef.current < 350) return;
          lastOpenRef.current = now;
          setMorph({ phase: "opening", fx: p.fx as number, fy: p.fy as number });
          later(260, () => setMorph((m) => ({ ...m, phase: "idle" })));
        } else {
          // Plain reveal (first-run, custom pos, tray): quick fade.
          setMorph((m) => ({ ...m, phase: "idle" }));
        }
      }),
      on<{ fx?: number; fy?: number }>("app://window-closing", (p) => {
        setMorph((m) => ({
          phase: "closing",
          fx: Number.isFinite(p?.fx) && (p?.fx as number) !== 0 ? (p?.fx as number) : m.fx,
          fy: Number.isFinite(p?.fy) && (p?.fy as number) !== 0 ? (p?.fy as number) : m.fy,
        }));
        later(240, () => setMorph((m) => ({ ...m, phase: "closed" })));
      }),
      on("app://new-conversation", () => {
        setShowHistory(false);
        newConversation();
        composerRef.current?.focus();
      }),
      on("app://toggle-hands-free", () => void useVoice.getState().toggleHandsFree()),
      on<DictationStateEvent>("dictation://state", (e) => {
        setDictating(e.state === "listening" || e.state === "transcribing" || e.state === "correcting");
      }),
    ];
    return () => {
      window.clearTimeout(morphTimer.current);
      subs.forEach((p) => void p.then((un) => un()));
    };
  }, [newConversation]);

  useEffect(() => {
    // Files dropped anywhere on the window are attached to the next message.
    const sub = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "over") {
        setDropping(true);
      } else if (e.payload.type === "drop") {
        setDropping(false);
        void attachPaths(e.payload.paths);
        composerRef.current?.focus();
      } else {
        setDropping(false);
      }
    });
    return () => void sub.then((un) => un());
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Esc cancels push-to-talk record (listening AND transcribing),
      // discarding audio. Takes priority over minimize.
      if (e.key === "Escape" && recording) {
        e.preventDefault();
        e.stopPropagation();
        void stopPtt(false);
        return;
      }
      // Esc cancels dictation too (overlay can't take focus).
      if (e.key === "Escape" && dictating) {
        e.preventDefault();
        e.stopPropagation();
        void ipc.dictationCancel().catch(() => undefined);
        return;
      }
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === "n") {
        e.preventDefault();
        newConversation();
        composerRef.current?.focus();
      } else if (mod && e.key.toLowerCase() === "h") {
        e.preventDefault();
        setShowHistory((v) => !v);
      } else if (mod && e.key === ",") {
        e.preventDefault();
        void ipc.openSettings().catch((err) => console.error("open settings failed", err));
      } else if (e.key === "Escape" && !busy && !showHistory && !document.querySelector(".dialog, .picker-menu")) {
        void ipc.minimizeWindow();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, showHistory, newConversation, recording, stopPtt, dictating]);

  useEffect(() => {
    const el = listRef.current;
    if (el && stickToBottom.current) el.scrollTo({ top: el.scrollHeight });
  }, [messages]);

  const lastAssistantIdx = messages.map((m) => m.role).lastIndexOf("assistant");
  const lastAssistant = lastAssistantIdx >= 0 ? messages[lastAssistantIdx] : null;

  const status =
    lm === "connected" ? (
      <>
        <span className="dot ok" aria-hidden />
        <span className="header-status-text">{activeModel ? modelLabel(activeModel, aliases, 26) : t("chat.online")}</span>
      </>
    ) : lm === "checking" ? (
      <>
        <span className="dot busy" aria-hidden />
        {t("status.checking")}
      </>
    ) : (
      <>
        <span className="dot err" aria-hidden />
        {needsKey ? t("chat.needsKey") : t("chat.offline")}
      </>
    );

  return (
    <div
      className={`app-shell${compact ? " compact" : ""}`}
      data-morph={morph.phase}
      style={{ "--zx": `${morph.fx}px`, "--zy": `${morph.fy}px` } as React.CSSProperties}
    >
      <div className="morph-content">
      <header className="header">
        <div className="header-drag" data-tauri-drag-region>
          <BrandMark size={30} />
          <div className="header-titles" data-tauri-drag-region>
            <span className="header-title" data-tauri-drag-region>
              {settings?.general.assistantName || t("app.name")}
            </span>
            <span className="header-status" data-tauri-drag-region title={activeModel ?? undefined}>
              {status}
            </span>
          </div>
        </div>
        <div className="header-actions">
          <button
            className="icon-btn"
            aria-label={t("app.newConversation")}
            title={`${t("app.newConversation")} (Ctrl+N)`}
            onClick={() => {
              setShowHistory(false);
              newConversation();
              composerRef.current?.focus();
            }}
          >
            <SquarePen size={16} />
          </button>
          <button className={`icon-btn${showHistory ? " active" : ""}`} aria-label={t("app.history")} title={`${t("app.history")} (Ctrl+H)`} aria-pressed={showHistory} onClick={() => setShowHistory((v) => !v)}>
            <History size={16} />
          </button>
          <button className="icon-btn" aria-label={t("app.settings")} title={`${t("app.settings")} (Ctrl+,)`} onClick={() => void ipc.openSettings().catch((err) => console.error("open settings failed", err))}>
            <Settings2 size={16} />
          </button>
          <span className="header-sep" aria-hidden />
          <button className="icon-btn" aria-label={t("app.minimize")} title={t("app.minimize")} onClick={() => void ipc.minimizeWindow()}>
            <Minus size={16} />
          </button>
          <button className="icon-btn danger" aria-label={t("app.close")} title={`${t("app.close")} — ${t("chat.closeHint")}`} onClick={() => void ipc.hideWindow()}>
            <X size={16} />
          </button>
        </div>
      </header>

      {lm === "unavailable" && (
        <div className="banner" role="alert">
          <div>
            <strong>{t("status.lmUnavailableTitle")}</strong> {t("status.lmUnavailableBody")}
          </div>
          <div className="banner-actions">
            <button className="btn btn-sm" onClick={() => void checkLm()}>
              {t("app.retry")}
            </button>
            <button className="btn btn-sm btn-ghost" onClick={() => void ipc.openSettings("ai")}>
              {t("app.openSettings")}
            </button>
          </div>
        </div>
      )}

      {callView ? (
        <CallView />
      ) : compact ? (
        <div className="compact-last" dir={lastAssistant ? textDir(lastAssistant.content, lastAssistant.language) : undefined}>
          {lastAssistant?.content || (busy ? t("chat.thinking") : t("chat.emptyHint"))}
        </div>
      ) : (
        <div
          className="messages"
          ref={listRef}
          role="log"
          aria-live="polite"
          aria-label={t("app.name")}
          onScroll={(e) => {
            const el = e.currentTarget;
            stickToBottom.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
          }}
        >
          {messages.length === 0 ? (
            <div className="empty-state">
              <div className="empty-orb" aria-hidden>
                <BrandMark size={52} />
              </div>
              <h1>{t("chat.emptyTitle")}</h1>
              <p>{t("chat.emptyHint")}</p>
              <div className="suggestions">
                {SUGGESTIONS.map((k) => (
                  <button key={k} className="suggestion" dir="auto" onClick={() => void send(t(k))} disabled={lm !== "connected"}>
                    {t(k)}
                  </button>
                ))}
              </div>
            </div>
          ) : (
            messages.map((m, i) => (
              <MessageBubble
                key={m.id}
                message={m}
                developer={developer}
                showReasoning={settings?.ai.showReasoning ?? false}
                isLastAssistant={i === lastAssistantIdx}
                modelName={m.model ? modelLabel(m.model, aliases) : undefined}
                onRetry={m.error && i === messages.length - 1 ? () => void retryLast() : undefined}
                onOpenSettings={() => void ipc.openSettings("ai")}
              />
            ))
          )}
        </div>
      )}

      <Composer ref={composerRef} autoModel={autoModel} onVoiceSetup={() => void ipc.openSettings("speech")} />
      </div>
      {dropping && (
        <div className="drop-veil" role="status">
          <div className="drop-card">
            <Upload size={22} aria-hidden />
            <span>{t("attach.dropHere")}</span>
          </div>
        </div>
      )}
      <div className="morph-veil" aria-hidden />
      <ToolConfirmDialog />
      {showHistory && <HistoryPanel onClose={() => setShowHistory(false)} />}
    </div>
  );
}
