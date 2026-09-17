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
  /// Tag of the audio currently playing (message id for "read aloud").
  speakingTag: string | null;
  paused: boolean;
  handsFree: boolean;
  /// Microphone the running session opened.
  device: string | null;
  /// Live text while the user is still speaking.
  partial: string;
  lastTranscript: string | null;
  error: AppErrorPayload | null;
  startPushToTalk: () => Promise<void>;
  stopPushToTalk: (submit: boolean) => Promise<void>;
  toggleHandsFree: () => Promise<void>;
  clearError: () => void;
  subscribe: () => Promise<() => void>;
}

const BARS = 24;
let subscription: Promise<void> | null = null;

export const useVoice = create<VoiceState>((set, get) => ({
  mode: null,
  phase: "idle",
  level: 0,
  levels: new Array(BARS).fill(0),
  speaking: false,
  speakingTag: null,
  paused: false,
  handsFree: false,
  device: null,
  partial: "",
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
    // Listeners are registered once per window and kept for its lifetime:
    // re-subscribing (React strict mode remounts) must never duplicate them,
    // and an unmount must never leave the window without any listener.
    if (subscription) {
      await subscription;
      return () => undefined;
    }
    subscription = (async () => {
      await on<VoiceEvent>("voice://event", (ev) => {
      if (ev.mode === "dictation" || ev.mode === "test") return;
      switch (ev.type) {
        case "state":
          if (ev.state === "idle") {
            set((s) => ({ phase: "idle", level: 0, mode: s.handsFree && ev.mode === "handsFree" ? null : s.mode === ev.mode ? null : s.mode, handsFree: ev.mode === "handsFree" ? false : s.handsFree }));
          } else {
            set({ phase: ev.state, mode: ev.mode, handsFree: ev.mode === "handsFree" ? true : get().handsFree, ...(ev.device ? { device: ev.device } : {}) });
          }
          break;
        case "level":
          set((s) => ({ level: ev.value, levels: [...s.levels.slice(1), ev.value] }));
          break;
        case "partial":
          set({ partial: ev.text });
          break;
        case "transcript": {
          const text = ev.text.trim();
          const chat = useChat.getState();
          if (!text) {
            if (ev.mode === "pushToTalk") chat.setVoiceNotice(t("voice.noSpeech"));
            return;
          }
          set({ lastTranscript: text, partial: "" });
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
      await on<TtsEvent>("tts://event", (ev) => {
      if (ev.type === "speaking") set({ speaking: true, speakingTag: ev.tag, paused: false });
      if (ev.type === "paused") set({ paused: true });
      if (ev.type === "resumed") set({ paused: false });
      if (ev.type === "idle") set({ speaking: false, speakingTag: null, paused: false });
      if (ev.type === "voiceUnavailable") useChat.getState().setVoiceNotice(t("chat.voiceUnavailable", { language: languageName(ev.language) }));
    });
      await on<AppErrorPayload>("voice://error", (e) => set({ error: e }));
    })();
    await subscription;
    return () => undefined;
  },
}));
