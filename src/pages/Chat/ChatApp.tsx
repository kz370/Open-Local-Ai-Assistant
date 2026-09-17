import { useCallback, useEffect, useRef, useState } from "react";
import { History, Minus, Settings2, SquarePen, X } from "lucide-react";
import { useChat } from "../../app/chatStore";
import { ipc, on } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { modelLabel, t } from "../../app/strings";
import { useVoice } from "../../app/voiceStore";
import { Composer, type ComposerHandle } from "../../components/chat/Composer";
import { MessageBubble } from "../../components/chat/MessageBubble";
import { ToolConfirmDialog } from "../../components/chat/ToolConfirmDialog";
import { BrandMark } from "../../components/common/BrandMark";
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
  const composerRef = useRef<ComposerHandle>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);

  const developer = settings?.general.developerMode ?? false;
  const compact = settings?.general.compact ?? false;
  const aliases = settings?.ai.modelAliases;
  const activeModel = settings?.ai.modelMode === "manual" ? settings.ai.model : autoModel;

  const checkLm = useCallback(async () => {
    try {
      await ipc.lmstudioTest();
      setLm("connected");
      setAutoModel((await ipc.lmstudioAutoSelection())?.modelId ?? null);
    } catch {
      setLm("unavailable");
    }
  }, []);

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
  }, [checkLm, settings?.ai.serverUrl]);

  useEffect(() => {
    if (connectionError) setLm("unavailable");
  }, [connectionError]);

  useEffect(() => {
    const subs = [
      on("app://focus-input", () => {
        setShowHistory(false);
        composerRef.current?.focus();
      }),
      on("app://new-conversation", () => {
        setShowHistory(false);
        newConversation();
        composerRef.current?.focus();
      }),
      on("app://toggle-hands-free", () => void useVoice.getState().toggleHandsFree()),
    ];
    return () => subs.forEach((p) => void p.then((un) => un()));
  }, [newConversation]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
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
        void ipc.openSettings();
      } else if (e.key === "Escape" && !busy && !showHistory && !document.querySelector(".dialog, .picker-menu")) {
        void ipc.minimizeWindow();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, showHistory, newConversation]);

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
        {t("chat.offline")}
      </>
    );

  return (
    <div className={`app-shell${compact ? " compact" : ""}`}>
      <header className="header">
        <div className="header-drag" data-tauri-drag-region>
          <BrandMark size={30} />
          <div className="header-titles" data-tauri-drag-region>
            <span className="header-title" data-tauri-drag-region>
              {t("app.name")}
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
          <button className="icon-btn" aria-label={t("app.settings")} title={`${t("app.settings")} (Ctrl+,)`} onClick={() => void ipc.openSettings()}>
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

      {compact ? (
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
      <ToolConfirmDialog />
      {showHistory && <HistoryPanel onClose={() => setShowHistory(false)} />}
    </div>
  );
}
