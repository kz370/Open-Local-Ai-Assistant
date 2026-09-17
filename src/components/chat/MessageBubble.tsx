import { memo, useState } from "react";
import { Check, Copy, RotateCcw, Volume2 } from "lucide-react";
import type { UiMessage } from "../../app/chatStore";
import { ipc } from "../../app/ipc";
import { t } from "../../app/strings";
import { ErrorNotice, textDir } from "../common/controls";
import { BrandMark } from "../common/BrandMark";
import { Markdown } from "./Markdown";
import { Sources } from "./Sources";
import { ToolActivity } from "./ToolActivity";

interface Props {
  message: UiMessage;
  developer: boolean;
  showReasoning: boolean;
  isLastAssistant: boolean;
  modelName?: string;
  onRetry?: () => void;
  onOpenSettings?: () => void;
}

export const MessageBubble = memo(function MessageBubble({ message: m, developer, showReasoning, isLastAssistant, modelName, onRetry, onOpenSettings }: Props) {
  const [copied, setCopied] = useState(false);
  const dir = textDir(m.content, m.language);
  const lang = m.language ?? undefined;

  if (m.role === "user") {
    return (
      <div className="msg user">
        <span className="sr-only">{t("chat.you")}</span>
        <div className="bubble" dir={dir} lang={lang}>
          {m.content}
        </div>
      </div>
    );
  }

  const waiting = m.streaming && !m.content && !m.tools.some((x) => x.status === "running" || x.status === "awaiting");
  return (
    <div className="msg assistant" aria-busy={m.streaming}>
      <div className="msg-avatar">
        <BrandMark size={24} />
      </div>
      <div className="msg-body">
      <div className="msg-label">
        {t("chat.assistant")}
        {modelName && <span className="msg-model">{modelName}</span>}
      </div>
      <ToolActivity tools={m.tools} developer={developer} />
      {showReasoning && m.reasoning && (
        <details className="reasoning">
          <summary>{m.streaming && !m.content ? t("chat.thinking") : t("chat.showReasoning")}</summary>
          <pre dir="auto">{m.reasoning}</pre>
        </details>
      )}
      {waiting && (
        <div className="typing" role="status" aria-label={t("chat.thinking")}>
          <span />
          <span />
          <span />
        </div>
      )}
      {m.content && (
        <div className="bubble" dir={dir} lang={lang}>
          <Markdown text={m.content} />
        </div>
      )}
      <Sources sources={m.sources} />
      {m.error && (
        <ErrorNotice
          error={m.error}
          actions={
            <>
              {onRetry && (
                <button className="btn btn-sm" onClick={onRetry}>
                  {t("app.retry")}
                </button>
              )}
              {onOpenSettings && (m.error.code === "lmstudio_unavailable" || m.error.code === "no_model") && (
                <button className="btn btn-sm" onClick={onOpenSettings}>
                  {t("app.openSettings")}
                </button>
              )}
            </>
          }
        />
      )}
      {!m.streaming && m.content && (
        <div className="msg-actions">
          <button
            className="icon-btn"
            aria-label={copied ? t("app.copied") : t("chat.copyMessage")}
            title={copied ? t("app.copied") : t("chat.copyMessage")}
            onClick={() => {
              void navigator.clipboard.writeText(m.content);
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
          >
            {copied ? <Check size={14} /> : <Copy size={14} />}
          </button>
          <button className="icon-btn" aria-label={t("chat.speak")} title={t("chat.speak")} onClick={() => void ipc.ttsSpeak(m.content, m.language)}>
            <Volume2 size={14} />
          </button>
          {isLastAssistant && (
            <button className="icon-btn" aria-label={t("chat.replay")} title={t("chat.replay")} onClick={() => void ipc.ttsReplayLast().then((ok) => { if (!ok) void ipc.ttsSpeak(m.content, m.language); })}>
              <RotateCcw size={14} />
            </button>
          )}
        </div>
      )}
      </div>
    </div>
  );
});
