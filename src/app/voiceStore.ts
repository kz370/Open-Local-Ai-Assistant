import { create } from "zustand";
import { useChat } from "./chatStore";
import { ipc, on, toAppError } from "./ipc";
import { useSettings } from "./settingsStore";
import { languageName, t } from "./strings";
import type { AppErrorPayload, ListenMode, TtsEvent, VoiceEvent } from "./types";

/** Bands per `level` event; matches `audio::SPECTRUM_BANDS` in the backend. */
export const SPECTRUM_BANDS = 24;

interface VoiceState {
  mode: ListenMode | null;
  phase: "idle" | "listening" | "transcribing";
  level: number;
  levels: number[];
  /** Latest voice spectrum, low to high pitch (0..1 each). */
  bands: number[];
  speaking: boolean;
  /// Tag of the audio currently playing (message id for "read aloud").
  speakingTag: string | null;
  paused: boolean;
  /** performance.now() of the pause, used to keep the spoken word in step. */
  pausedAt: number | null;
  handsFree: boolean;
  /// True while the user has muted the microphone from the call screen.
  muted: boolean;
  /// Microphone the running session opened.
  device: string | null;
  /// Live text while the user is still speaking.
  partial: string;
  /// The sentence the assistant is speaking right now, with the timing the UI
  /// needs to follow it word by word.
  spoken: SpokenSentence | null;
  lastTranscript: string | null;
  error: AppErrorPayload | null;
  startPushToTalk: () => Promise<void>;
  stopPushToTalk: (submit: boolean) => Promise<void>;
  toggleHandsFree: () => Promise<void>;
  toggleMuted: () => Promise<void>;
  /** Stops the answer being spoken (and generated) so the user can take over. */
  interrupt: () => void;
  clearError: () => void;
  subscribe: () => Promise<() => void>;
}

/** A sentence being played, and where its playback started on the UI clock. */
export interface SpokenSentence {
  tag: string;
  text: string;
  durationMs: number;
  /** performance.now() when playback started, shifted forward while paused. */
  startedAt: number;
}

const BARS = 24;
let subscription: Promise<void> | null = null;

export const useVoice = create<VoiceState>((set, get) => ({
  mode: null,
  phase: "idle",
  level: 0,
  levels: new Array(BARS).fill(0),
  bands: [],
  speaking: false,
  speakingTag: null,
  paused: false,
  pausedAt: null,
  handsFree: false,
  muted: false,
  device: null,
  partial: "",
  spoken: null,
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
      set({ handsFree: false, muted: false, mode: null, phase: "idle", level: 0 });
      return;
    }
    set({ error: null });
    try {
      await ipc.voiceStart("handsFree");
      set({ handsFree: true, muted: false, mode: "handsFree", phase: "listening" });
    } catch (e) {
      set({ error: toAppError(e), handsFree: false });
    }
  },

  toggleMuted: async () => {
    const next = !get().muted;
    set({ muted: next }); // optimistic: the button must react at once
    try {
      set({ muted: await ipc.voiceSetMuted(next) });
    } catch (e) {
      set({ muted: !next, error: toAppError(e) });
    }
  },

  interrupt: () => {
    void ipc.ttsStop();
    const chat = useChat.getState();
    if (chat.turnId) chat.stop();
    set({ speaking: false, speakingTag: null, paused: false, pausedAt: null, spoken: null });
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
            set((s) => ({ phase: "idle", level: 0, bands: [], mode: s.handsFree && ev.mode === "handsFree" ? null : s.mode === ev.mode ? null : s.mode, handsFree: ev.mode === "handsFree" ? false : s.handsFree }));
          } else {
            set({ phase: ev.state, mode: ev.mode, handsFree: ev.mode === "handsFree" ? true : get().handsFree, ...(ev.device ? { device: ev.device } : {}) });
          }
          break;
        case "level":
          set((s) => ({ level: ev.value, bands: ev.bands, levels: [...s.levels.slice(1), ev.value] }));
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
          const autoSubmit = useSettings.getState().settings?.stt.autoSubmit ?? false;
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
      if (ev.type === "sentence")
        set({ speaking: true, speakingTag: ev.tag, spoken: { tag: ev.tag, text: ev.text, durationMs: ev.durationMs, startedAt: performance.now() } });
      if (ev.type === "paused") set({ paused: true, pausedAt: performance.now() });
      if (ev.type === "resumed")
        set((s) => ({
          paused: false,
          pausedAt: null,
          // Skip the pause, so the highlighted word stays on the spoken one.
          spoken: s.spoken && s.pausedAt !== null ? { ...s.spoken, startedAt: s.spoken.startedAt + (performance.now() - s.pausedAt) } : s.spoken,
        }));
      if (ev.type === "idle") set({ speaking: false, speakingTag: null, paused: false, pausedAt: null, spoken: null });
      if (ev.type === "voiceUnavailable") useChat.getState().setVoiceNotice(t("chat.voiceUnavailable", { language: languageName(ev.language) }));
    });
      await on<AppErrorPayload>("voice://error", (e) => set({ error: e }));
    })();
    await subscription;
    return () => undefined;
  },
}));
