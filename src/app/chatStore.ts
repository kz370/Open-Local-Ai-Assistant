import { create } from "zustand";
import { ipc, newId, toAppError } from "./ipc";
import type { ActivityRecord, AppErrorPayload, Attachment, ChatEvent, Message, Source, ToolCategory } from "./types";

export type ToolStatus = "running" | "awaiting" | "done" | "failed" | "denied";

export interface UiToolActivity {
  callId: string;
  serverName: string;
  toolName: string;
  category: ToolCategory;
  args: unknown;
  status: ToolStatus;
  durationMs?: number;
  resultPreview?: string;
  sources?: Source[];
}

export interface UiMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  attachments: Attachment[];
  language: string | null;
  reasoning: string;
  sources: Source[];
  tools: UiToolActivity[];
  streaming: boolean;
  error?: AppErrorPayload;
  model?: string;
  createdAt: string;
}

type SendOpts = { spokenLanguage?: string | null; voice?: boolean };

export interface QueuedMessage {
  id: string;
  text: string;
  attachments: Attachment[];
  opts?: SendOpts;
}

interface ChatState {
  conversationId: string | null;
  messages: UiMessage[];
  turnId: string | null;
  draft: string;
  /** Files staged in the composer, sent with the next message. */
  attachments: Attachment[];
  /** Files the backend refused, shown once above the composer. */
  attachmentErrors: string[];
  /** Messages typed while a reply is streaming, sent in order once it finishes. */
  queue: QueuedMessage[];
  confirmation: UiToolActivity | null;
  /** Set when the last turn failed because LM Studio is unreachable. */
  connectionError: AppErrorPayload | null;
  voiceNotice: string | null;
  setDraft: (d: string) => void;
  addAttachments: (added: Attachment[], failures?: string[]) => void;
  removeAttachment: (id: string) => void;
  clearAttachments: () => void;
  dismissAttachmentErrors: () => void;
  send: (text: string, opts?: SendOpts) => Promise<void>;
  removeQueued: (id: string) => void;
  retryLast: () => Promise<void>;
  stop: () => void;
  newConversation: () => void;
  loadConversation: (id: string) => Promise<void>;
  confirmTool: (approved: boolean) => void;
  setVoiceNotice: (n: string | null) => void;
  handleEvent: (ev: ChatEvent) => void;
}

export function activityToUi(a: ActivityRecord): UiToolActivity {
  return {
    callId: a.callId,
    serverName: a.serverName,
    toolName: a.toolName,
    category: a.category,
    args: a.args,
    status: a.denied ? "denied" : a.ok ? "done" : "failed",
    durationMs: a.durationMs,
    resultPreview: a.resultPreview,
  };
}

export function messageToUi(m: Message): UiMessage {
  return {
    id: m.id,
    role: m.role === "user" ? "user" : "assistant",
    content: m.content,
    attachments: m.attachments ?? [],
    language: m.language,
    reasoning: m.reasoning ?? "",
    sources: m.sources ?? [],
    tools: (m.toolActivity ?? []).map(activityToUi),
    streaming: false,
    createdAt: m.createdAt,
  };
}

const PENDING_ID = "pending-assistant";

export const useChat = create<ChatState>((set, get) => {
  const patchAssistant = (turnId: string, fn: (m: UiMessage) => UiMessage) => {
    if (get().turnId !== turnId) return;
    set((s) => ({ messages: s.messages.map((m) => (m.id === PENDING_ID ? fn(m) : m)) }));
  };

  const startTurn = async (text: string, attachments: Attachment[], opts?: SendOpts) => {
    const turnId = newId();
    const now = new Date().toISOString();
    set((s) => ({
      turnId,
      confirmation: null,
      connectionError: null,
      voiceNotice: null,
      messages: [
        ...s.messages.filter((m) => m.id !== PENDING_ID),
        { id: `local-${turnId}`, role: "user", content: text, attachments, language: opts?.spokenLanguage ?? null, reasoning: "", sources: [], tools: [], streaming: false, createdAt: now },
        { id: PENDING_ID, role: "assistant", content: "", attachments: [], language: null, reasoning: "", sources: [], tools: [], streaming: true, createdAt: now },
      ],
    }));
    try {
      await ipc.chatSend(
        {
          turnId,
          conversationId: get().conversationId,
          text,
          spokenLanguage: opts?.spokenLanguage ?? null,
          voice: !!opts?.voice,
          attachmentIds: attachments.map((a) => a.id),
        },
        (ev) => get().handleEvent(ev),
      );
    } catch (e) {
      get().handleEvent({ type: "error", turnId, ...toAppError(e), partialMessage: null });
    }
  };

  /** Puts queued messages back into the composer instead of sending them. */
  const restoreQueue = () => {
    const queue = get().queue;
    if (!queue.length) return;
    set((s) => ({
      queue: [],
      draft: [...queue.map((q) => q.text), s.draft].filter(Boolean).join("\n\n"),
      attachments: [...queue.flatMap((q) => q.attachments), ...s.attachments],
    }));
  };

  /** Starts the next queued message, if any. */
  const sendNextQueued = () => {
    const [next, ...rest] = get().queue;
    if (!next) return;
    set({ queue: rest });
    void startTurn(next.text, next.attachments, next.opts);
  };

  return {
    conversationId: null,
    messages: [],
    turnId: null,
    draft: "",
    attachments: [],
    attachmentErrors: [],
    queue: [],
    confirmation: null,
    connectionError: null,
    voiceNotice: null,

    setDraft: (draft) => set({ draft }),
    setVoiceNotice: (voiceNotice) => set({ voiceNotice }),

    addAttachments: (added, failures = []) =>
      set((s) => ({ attachments: [...s.attachments, ...added], attachmentErrors: failures })),

    removeAttachment: (id) => {
      void ipc.attachRemove(id).catch(() => undefined);
      set((s) => ({ attachments: s.attachments.filter((a) => a.id !== id) }));
    },

    clearAttachments: () => {
      // Drops the staged copies on disk as well.
      get().attachments.forEach((a) => void ipc.attachRemove(a.id).catch(() => undefined));
      set({ attachments: [], attachmentErrors: [] });
    },

    dismissAttachmentErrors: () => set({ attachmentErrors: [] }),

    send: async (text, opts) => {
      const trimmed = text.trim();
      const attachments = get().attachments;
      if (!trimmed && attachments.length === 0) return;
      if (get().turnId) {
        // Voice barges in on the current reply; typed messages wait their turn.
        if (!opts?.voice) {
          set((s) => ({ queue: [...s.queue, { id: newId(), text: trimmed, attachments, opts }], draft: "", attachments: [], attachmentErrors: [] }));
          return;
        }
      }
      set({ draft: "", attachments: [], attachmentErrors: [] });
      // Stopping after clearing the composer lets queued messages land back in it.
      if (get().turnId) get().stop();
      await startTurn(trimmed, attachments, opts);
    },

    removeQueued: (id) => {
      const item = get().queue.find((q) => q.id === id);
      item?.attachments.forEach((a) => void ipc.attachRemove(a.id).catch(() => undefined));
      set((s) => ({ queue: s.queue.filter((q) => q.id !== id) }));
    },

    retryLast: async () => {
      const lastUser = [...get().messages].reverse().find((m) => m.role === "user");
      if (!lastUser) return;
      // Remove the failed exchange from view; the backend keeps the stored user message.
      set((s) => ({ messages: s.messages.filter((m) => m.id !== lastUser.id && !(m.role === "assistant" && m.error)) }));
      // The attachments of that turn were already consumed by the backend, so a
      // retry resends the text only.
      await get().send(lastUser.content, { spokenLanguage: lastUser.language });
    },

    stop: () => {
      const id = get().turnId;
      if (id) void ipc.chatStop(id);
      void ipc.ttsStop();
      restoreQueue();
    },

    newConversation: () => {
      get().stop();
      get().clearAttachments();
      set({ conversationId: null, messages: [], turnId: null, confirmation: null, connectionError: null, draft: "" });
      void ipc.convSetLast(null);
    },

    loadConversation: async (id) => {
      get().stop();
      const [conv, msgs] = await ipc.convGet(id);
      set({ conversationId: conv.id, messages: msgs.map(messageToUi), turnId: null, confirmation: null, connectionError: null });
      void ipc.convSetLast(conv.id);
    },

    confirmTool: (approved) => {
      const c = get().confirmation;
      if (!c) return;
      void ipc.chatConfirmTool(c.callId, approved);
      set({ confirmation: null });
    },

    handleEvent: (ev) => {
      switch (ev.type) {
        case "started":
          if (get().turnId !== ev.turnId) return;
          set((s) => ({
            conversationId: ev.conversationId,
            messages: s.messages.map((m) =>
              m.id === `local-${ev.turnId}` ? messageToUi(ev.userMessage) : m.id === PENDING_ID ? { ...m, model: ev.model, language: ev.language } : m,
            ),
          }));
          break;
        case "delta":
          patchAssistant(ev.turnId, (m) => ({ ...m, content: m.content + ev.text }));
          break;
        case "reasoning":
          patchAssistant(ev.turnId, (m) => ({ ...m, reasoning: m.reasoning + ev.text }));
          break;
        case "toolAwaitingConfirmation": {
          const activity: UiToolActivity = { callId: ev.callId, serverName: ev.serverName, toolName: ev.toolName, category: ev.category, args: ev.args, status: "awaiting" };
          patchAssistant(ev.turnId, (m) => ({ ...m, tools: [...m.tools, activity] }));
          if (get().turnId === ev.turnId) set({ confirmation: activity });
          break;
        }
        case "toolStarted":
          patchAssistant(ev.turnId, (m) => {
            const exists = m.tools.some((t) => t.callId === ev.callId);
            const running: UiToolActivity = { callId: ev.callId, serverName: ev.serverName, toolName: ev.toolName, category: ev.category, args: ev.args, status: "running" };
            return { ...m, tools: exists ? m.tools.map((t) => (t.callId === ev.callId ? { ...t, status: "running" } : t)) : [...m.tools, running] };
          });
          break;
        case "toolFinished":
          patchAssistant(ev.turnId, (m) => ({
            ...m,
            sources: [...m.sources, ...ev.sources.filter((s) => !m.sources.some((x) => x.url === s.url))],
            tools: m.tools.map((t) =>
              t.callId === ev.callId
                ? { ...t, status: ev.denied ? "denied" : ev.ok ? "done" : "failed", durationMs: ev.durationMs, resultPreview: ev.resultPreview, sources: ev.sources }
                : t,
            ),
          }));
          if (get().confirmation?.callId === ev.callId) set({ confirmation: null });
          break;
        case "done":
          if (get().turnId !== ev.turnId) return;
          set((s) => ({
            turnId: null,
            confirmation: null,
            messages: s.messages.map((m) => {
              if (m.id !== PENDING_ID) return m;
              const done = messageToUi(ev.message);
              // Keep live tool details (args/results) gathered during streaming.
              return { ...done, tools: m.tools.length ? m.tools : done.tools, model: m.model };
            }),
          }));
          sendNextQueued();
          break;
        case "error": {
          if (get().turnId !== ev.turnId) return;
          const err = { code: ev.code, detail: ev.detail };
          const cancelled = ev.code === "cancelled";
          set((s) => ({
            turnId: null,
            confirmation: null,
            connectionError: ev.code === "lmstudio_unavailable" || ev.code === "no_model" ? err : null,
            messages: s.messages.flatMap((m) => {
              if (m.id !== PENDING_ID) return [m];
              if (ev.partialMessage) {
                const partial = messageToUi(ev.partialMessage);
                return [{ ...partial, tools: m.tools, error: cancelled ? undefined : err }];
              }
              if (cancelled && !m.content) return [];
              return [{ ...m, id: `failed-${ev.turnId}`, streaming: false, error: cancelled ? undefined : err }];
            }),
          }));
          // A failed turn would likely fail the queued ones too; hand them back.
          restoreQueue();
          break;
        }
      }
    },
  };
});
