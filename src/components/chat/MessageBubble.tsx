import { memo, useState } from "react";
import { Check, Copy, Pause, Play, Square, Volume2 } from "lucide-react";
import type { UiMessage } from "../../app/chatStore";
import { ipc } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { useVoice } from "../../app/voiceStore";
import { t } from "../../app/strings";
import { ErrorNotice, textDir } from "../common/controls";
import { BrandMark } from "../common/BrandMark";
import { AttachmentList } from "./Attachments";
import { Markdown } from "./Markdown";
import { Sources } from "./Sources";
import { ToolActivity } from "./ToolActivity";

/** Sounds an Orpheus voice performs (`<laugh>` …), shown as stage directions. */
const PERFORMED: Record<string, string> = {
  laugh: "laughs",
  chuckle: "chuckles",
  sigh: "sighs",
  cough: "coughs",
  sniffle: "sniffles",
  groan: "groans",
  yawn: "yawns",
  gasp: "gasps",
};
const EXPRESSIVE_TAG = new RegExp(`<(${Object.keys(PERFORMED).join("|")})>`, "gi");
const showTags = (text: string, wrap = "*") => text.replace(EXPRESSIVE_TAG, (_, tag: string) => `${wrap}(${PERFORMED[tag.toLowerCase()]})${wrap}`);

interface Props {
  message: UiMessage;
  developer: boolean;
  showReasoning: boolean;
  /** Kept for layout decisions by the caller. */
  isLastAssistant?: boolean;
  modelName?: string;
  onRetry?: () => void;
  onOpenSettings?: () => void;
}

export const MessageBubble = memo(function MessageBubble({ message: m, developer, showReasoning, modelName, onRetry, onOpenSettings }: Props) {
  const [copied, setCopied] = useState(false);
  const assistantName = useSettings((s) => s.settings?.general.assistantName) || t("chat.assistant");
  const speakingTag = useVoice((s) => s.speakingTag);
  const paused = useVoice((s) => s.paused);
  const isThisPlaying = speakingTag === m.id;
  const dir = textDir(m.content, m.language);
  const lang = m.language ?? undefined;

  if (m.role === "user") {
    return (
      <div className="msg user">
        <span className="sr-only">{t("chat.you")}</span>
        {m.attachments.length > 0 && <AttachmentList items={m.attachments} />}
        {m.content && (
          <div className="bubble" dir={dir} lang={lang}>
            {m.content}
          </div>
        )}
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
        {assistantName}
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
          <Markdown text={showTags(m.content)} />
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
              void navigator.clipboard.writeText(showTags(m.content, ""));
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
          >
            {copied ? <Check size={14} /> : <Copy size={14} />}
          </button>
          {isThisPlaying ? (
            <>
              <button
                className="icon-btn active"
                aria-label={paused ? t("chat.resume") : t("chat.pause")}
                title={paused ? t("chat.resume") : t("chat.pause")}
                onClick={() => void ipc.ttsSetPaused(!paused)}
              >
                {paused ? <Play size={14} /> : <Pause size={14} />}
              </button>
              <button className="icon-btn" aria-label={t("chat.stopSpeaking")} title={t("chat.stopSpeaking")} onClick={() => void ipc.ttsStop()}>
                <Square size={12} fill="currentColor" />
              </button>
            </>
          ) : (
            <button className="icon-btn" aria-label={t("chat.speak")} title={t("chat.speak")} onClick={() => void ipc.ttsSpeak(m.content, m.language, m.id)}>
              <Volume2 size={14} />
            </button>
          )}
        </div>
      )}
      </div>
    </div>
  );
});
