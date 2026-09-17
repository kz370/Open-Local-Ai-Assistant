import { create } from "zustand";
import { ipc, on, toAppError } from "./ipc";
import type { AppErrorPayload, Settings } from "./types";

interface SettingsState {
  settings: Settings | null;
  error: AppErrorPayload | null;
  load: () => Promise<Settings>;
  /** Applies `mutate` to a copy of the current settings and persists it. */
  update: (mutate: (draft: Settings) => void) => Promise<void>;
  subscribe: () => Promise<() => void>;
}

export const useSettings = create<SettingsState>((set, get) => ({
  settings: null,
  error: null,
  load: async () => {
    const s = await ipc.getSettings();
    set({ settings: s });
    applyAppearance(s);
    return s;
  },
  update: async (mutate) => {
    const current = get().settings;
    if (!current) return;
    const draft: Settings = structuredClone(current);
    mutate(draft);
    set({ settings: draft }); // optimistic
    applyAppearance(draft);
    try {
      const saved = await ipc.saveSettings(draft);
      set({ settings: saved, error: null });
      applyAppearance(saved);
    } catch (e) {
      set({ settings: current, error: toAppError(e) });
      applyAppearance(current);
    }
  },
  subscribe: async () => {
    const un = await on<Settings>("settings://changed", (s) => {
      set({ settings: s });
      applyAppearance(s);
    });
    return un;
  },
}));

let mediaListener: ((e: MediaQueryListEvent) => void) | null = null;

export function applyAppearance(s: Settings) {
  const root = document.documentElement;
  const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
  const resolve = () => (s.general.theme === "system" ? (mq?.matches ? "dark" : "light") : s.general.theme);
  root.dataset.theme = resolve();
  root.dataset.contrast = s.general.highContrast ? "high" : "normal";
  root.style.setProperty("--font-scale", String(s.general.fontScale || 1));
  if (mq) {
    if (mediaListener) mq.removeEventListener?.("change", mediaListener);
    mediaListener = () => {
      root.dataset.theme = resolve();
    };
    mq.addEventListener?.("change", mediaListener);
  }
}
