import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Check, Copy, Lightbulb, MessageSquare, Volume2, X } from "lucide-react";
import { useChat } from "../../app/chatStore";
import { ipc, newId } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import { textDir } from "../common/controls";
import { Markdown } from "./Markdown";

/** Text selected inside one assistant reply. */
interface Picked {
  text: string;
  /** The whole reply, as context for the explanation. */
  passage: string;
  language: string | null;
  messageId: string | null;
  rect: DOMRect;
}

interface Explanation extends Picked {
  id: string;
  content: string;
  status: "loading" | "streaming" | "done" | "error";
  error?: string;
}

const MARGIN = 8;

/** The reply bubble the current selection lies in, or null. */
function pickSelection(): Picked | null {
  const sel = window.getSelection();
  const text = sel?.toString().trim();
  if (!sel || !text || sel.rangeCount === 0) return null;
  const node = sel.anchorNode;
  const el = node instanceof Element ? node : node?.parentElement;
  const bubble = el?.closest<HTMLElement>(".msg.assistant .bubble");
  if (!bubble) return null;
  return {
    text,
    passage: bubble.innerText || bubble.textContent || "",
    language: bubble.getAttribute("lang"),
    messageId: bubble.dataset.messageId ?? null,
    rect: sel.getRangeAt(0).getBoundingClientRect(),
  };
}

function quote(text: string) {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .join("\n> ");
}

/** Keeps a fixed box of the given size inside the window. */
function clamp(x: number, y: number, w: number, h: number) {
  return {
    left: Math.max(MARGIN, Math.min(x, window.innerWidth - w - MARGIN)),
    top: Math.max(MARGIN, Math.min(y, window.innerHeight - h - MARGIN)),
  };
}

/**
 * Right-click menu for text selected in a reply: copy it, speak it, or have it
 * explained (as a follow-up in the chat or in a popup, per Settings → AI).
 * Without a selection the webview's own menu is left alone.
 */
export function SelectionMenu() {
  const [menu, setMenu] = useState<(Picked & { x: number; y: number }) | null>(null);
  const [explain, setExplain] = useState<Explanation | null>(null);
  const explainMode = useSettings((s) => s.settings?.ai.explainMode ?? "chat");

  useEffect(() => {
    const onContextMenu = (e: MouseEvent) => {
      if (!(e.target as Element | null)?.closest?.(".messages")) return;
      const picked = pickSelection();
      if (!picked) return;
      e.preventDefault();
      setMenu({ ...picked, x: e.clientX, y: e.clientY });
    };
    const onScroll = () => setMenu(null);
    document.addEventListener("contextmenu", onContextMenu);
    document.addEventListener("scroll", onScroll, { capture: true, passive: true });
    return () => {
      document.removeEventListener("contextmenu", onContextMenu);
      document.removeEventListener("scroll", onScroll, { capture: true });
    };
  }, []);

  // A running explanation is cancelled when its popup goes away.
  const running = explain && (explain.status === "loading" || explain.status === "streaming") ? explain.id : null;
  useEffect(() => {
    if (!running) return;
    return () => void ipc.chatStop(running).catch(() => undefined);
  }, [running]);

  const speak = (text: string, language: string | null, tag?: string) => void ipc.ttsSpeak(text, language, tag).catch(() => undefined);

  const askInChat = (p: Picked) => {
    void useChat.getState().send(t("chat.selection.explainPrompt", { text: quote(p.text) }), { spokenLanguage: p.language });
  };

  const startExplain = (p: Picked) => {
    if (explainMode === "chat") {
      askInChat(p);
      return;
    }
    const id = newId();
    setExplain({ ...p, id, content: "", status: "loading" });
    const update = (fn: (x: Explanation) => Explanation) => setExplain((cur) => (cur && cur.id === id ? fn(cur) : cur));
    ipc
      .chatExplain(id, p.text, p.passage, (ev) => {
        if (ev.type === "delta") update((x) => ({ ...x, status: "streaming", content: x.content + ev.text }));
        else if (ev.type === "done") update((x) => ({ ...x, status: "done" }));
        else update((x) => ({ ...x, status: "error", error: errorMessage(ev.code) }));
      })
      .catch(() => update((x) => ({ ...x, status: "error", error: errorMessage("other") })));
  };

  return createPortal(
    <>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { key: "copy", icon: <Copy size={14} />, label: t("chat.selection.copy"), run: () => void navigator.clipboard.writeText(menu.text) },
            { key: "speak", icon: <Volume2 size={14} />, label: t("chat.selection.speak"), run: () => speak(menu.text, menu.language, menu.messageId ?? undefined) },
            { key: "explain", icon: <Lightbulb size={14} />, label: t("chat.selection.explain"), run: () => startExplain(menu) },
          ]}
        />
      )}
      {explain && (
        <ExplainPopover
          explanation={explain}
          onSpeak={() => speak(explain.content, null)}
          onAskInChat={() => {
            askInChat(explain);
            setExplain(null);
          }}
          onClose={() => setExplain(null)}
        />
      )}
    </>,
    document.body,
  );
}

interface MenuItem {
  key: string;
  icon: ReactNode;
  label: string;
  run: () => void;
}

function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: MenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setPos(clamp(x, y, el.offsetWidth, el.offsetHeight));
    el.querySelector<HTMLButtonElement>("button")?.focus({ preventScroll: true });
  }, [x, y]);

  useEffect(() => {
    const outside = (e: Event) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", outside, true);
    document.addEventListener("contextmenu", outside, true);
    document.addEventListener("keydown", key);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    return () => {
      document.removeEventListener("mousedown", outside, true);
      document.removeEventListener("contextmenu", outside, true);
      document.removeEventListener("keydown", key);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  const onKeyDown = (e: ReactKeyboardEvent) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const buttons = [...(ref.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    buttons[(i + (e.key === "ArrowDown" ? 1 : buttons.length - 1)) % buttons.length]?.focus();
  };

  return (
    <div ref={ref} className="ctx-menu" role="menu" aria-label={t("chat.selection.menu")} style={pos} onKeyDown={onKeyDown}>
      {items.map((it) => (
        <button
          key={it.key}
          type="button"
          role="menuitem"
          className="ctx-item"
          // Keep the text selected while the menu is used.
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => {
            onClose();
            it.run();
          }}
        >
          {it.icon}
          {it.label}
        </button>
      ))}
    </div>
  );
}

function ExplainPopover({ explanation: x, onSpeak, onAskInChat, onClose }: { explanation: Explanation; onSpeak: () => void; onAskInChat: () => void; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  // Below the selection when it fits, otherwise above it.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { rect } = x;
    const h = el.offsetHeight;
    const below = rect.bottom + MARGIN;
    const top = below + h <= window.innerHeight - MARGIN ? below : rect.top - MARGIN - h;
    setPos(clamp(rect.left, top, el.offsetWidth, h));
  }, [x.rect, x.content.length > 0, x.status]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, [onClose]);

  const [copied, setCopied] = useState(false);
  const copy = () => {
    void navigator.clipboard.writeText(x.content.trim());
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  const done = x.status === "done" && x.content.trim().length > 0;
  return (
    <div
      ref={ref}
      className="explain-pop"
      role="dialog"
      aria-label={t("chat.selection.explainTitle")}
      aria-busy={x.status === "loading" || x.status === "streaming"}
      style={pos ?? { left: -9999, top: -9999 }}
    >
      <div className="explain-head">
        <Lightbulb size={13} aria-hidden />
        <span className="explain-title">{t("chat.selection.explainTitle")}</span>
        <button className="icon-btn" aria-label={t("chat.selection.close")} title={t("chat.selection.close")} onClick={onClose}>
          <X size={14} />
        </button>
      </div>
      <blockquote className="explain-quote" dir={textDir(x.text, x.language)}>
        {x.text}
      </blockquote>
      <div className="explain-body" dir={x.content ? textDir(x.content, null) : undefined}>
        {x.status === "loading" && (
          <div className="typing" role="status" aria-label={t("chat.thinking")}>
            <span />
            <span />
            <span />
          </div>
        )}
        {x.content && <Markdown text={x.content} />}
        {x.status === "error" && <p className="explain-error">{x.error}</p>}
      </div>
      {done && (
        <div className="explain-actions">
          <button className="btn btn-sm" onClick={copy}>
            {copied ? <Check size={12} /> : <Copy size={12} />} {copied ? t("app.copied") : t("chat.selection.copy")}
          </button>
          <button className="btn btn-sm" onClick={onSpeak}>
            <Volume2 size={12} /> {t("chat.selection.speak")}
          </button>
          <button className="btn btn-sm" onClick={onAskInChat}>
            <MessageSquare size={12} /> {t("chat.selection.askInChat")}
          </button>
        </div>
      )}
    </div>
  );
}
