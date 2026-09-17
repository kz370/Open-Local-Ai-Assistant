import { create } from "zustand";
import { useChat } from "./chatStore";
import { ipc, on, toAppError } from "./ipc";
import { useSettings } from "./settingsStore";
import { languageName, t } from "./strings";
import type { AppErrorPayload, ListenMode, TtsEvent, VoiceEvent } from "./types";

interface VoiceState {
  mode: ListenMode | null;
  phase: "idle" | "listening" | "transcribing";
  level: number;
  levels: number[];
  speaking: boolean;
  handsFree: boolean;
  lastTranscript: string | null;
  error: AppErrorPayload | null;
  startPushToTalk: () => Promise<void>;
  stopPushToTalk: (submit: boolean) => Promise<void>;
  toggleHandsFree: () => Promise<void>;
  clearError: () => void;
  subscribe: () => Promise<() => void>;
}

const BARS = 24;

export const useVoice = create<VoiceState>((set, get) => ({
  mode: null,
  phase: "idle",
  level: 0,
  levels: new Array(BARS).fill(0),
  speaking: false,
  handsFree: false,
  lastTranscript: null,
  error: null,

  clearError: () => set({ error: null }),

  startPushToTalk: async () => {
    set({ error: null, lastTranscript: null });
    try {
      await ipc.voiceStart("pushToTalk");
      set({ mode: "pushToTalk", phase: "listening", handsFree: false });
    } catch (e) {
      set({ error: toAppError(e), mode: null, phase: "idle" });
    }
  },

  stopPushToTalk: async (submit) => {
    await ipc.voiceStop(!submit);
    if (!submit) set({ mode: null, phase: "idle", level: 0 });
  },

  toggleHandsFree: async () => {
    if (get().handsFree) {
      await ipc.voiceStop(true);
      set({ handsFree: false, mode: null, phase: "idle", level: 0 });
      return;
    }
    set({ error: null });
    try {
      await ipc.voiceStart("handsFree");
      set({ handsFree: true, mode: "handsFree", phase: "listening" });
    } catch (e) {
      set({ error: toAppError(e), handsFree: false });
    }
  },

  subscribe: async () => {
    const un1 = await on<VoiceEvent>("voice://event", (ev) => {
      if (ev.mode === "dictation" || ev.mode === "test") return;
      switch (ev.type) {
        case "state":
          if (ev.state === "idle") {
            set((s) => ({ phase: "idle", level: 0, mode: s.handsFree && ev.mode === "handsFree" ? null : s.mode === ev.mode ? null : s.mode, handsFree: ev.mode === "handsFree" ? false : s.handsFree }));
          } else {
            set({ phase: ev.state, mode: ev.mode, handsFree: ev.mode === "handsFree" ? true : get().handsFree });
          }
          break;
        case "level":
          set((s) => ({ level: ev.value, levels: [...s.levels.slice(1), ev.value] }));
          break;
        case "transcript": {
          const text = ev.text.trim();
          const chat = useChat.getState();
          if (!text) {
            if (ev.mode === "pushToTalk") chat.setVoiceNotice(t("voice.noSpeech"));
            return;
          }
          set({ lastTranscript: text });
          const autoSubmit = useSettings.getState().settings?.stt.autoSubmit ?? true;
          if (ev.mode === "handsFree" || autoSubmit) {
            void chat.send(text, { spokenLanguage: ev.language, voice: true });
          } else {
            chat.setDraft(chat.draft ? `${chat.draft} ${text}` : text);
          }
          break;
        }
        case "error":
          set({ error: { code: ev.code, detail: ev.detail } });
          break;
      }
    });
    const un2 = await on<TtsEvent>("tts://event", (ev) => {
      if (ev.type === "speaking") set({ speaking: true });
      if (ev.type === "idle") set({ speaking: false });
      if (ev.type === "voiceUnavailable") useChat.getState().setVoiceNotice(t("chat.voiceUnavailable", { language: languageName(ev.language) }));
    });
    const un3 = await on<AppErrorPayload>("voice://error", (e) => set({ error: e }));
    return () => {
      un1();
      un2();
      un3();
    };
  },
}));
