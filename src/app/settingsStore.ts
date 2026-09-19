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

/** Accent palettes (no purple). Values are [accent, hover, soft, softText]. */
const ACCENTS: Record<string, { light: string[]; dark: string[]; grad: string }> = {
  teal: { light: ["#0f766e", "#0b5f58", "#dcf1ec", "#0b4f49"], dark: ["#2dd4bf", "#5eead4", "#15332f", "#99f6e4"], grad: "linear-gradient(145deg, #2dd4bf, #0d9488)" },
  blue: { light: ["#1d4ed8", "#1e40af", "#dde7fd", "#152f6d"], dark: ["#60a5fa", "#93c5fd", "#16233d", "#bfdbfe"], grad: "linear-gradient(145deg, #60a5fa, #2563eb)" },
  green: { light: ["#15803d", "#166534", "#ddf3e3", "#0f4c25"], dark: ["#4ade80", "#86efac", "#14301f", "#bbf7d0"], grad: "linear-gradient(145deg, #4ade80, #16a34a)" },
  amber: { light: ["#b45309", "#92400e", "#fdeed6", "#7c3a06"], dark: ["#fbbf24", "#fcd34d", "#3a2a10", "#fde68a"], grad: "linear-gradient(145deg, #fbbf24, #d97706)" },
  rose: { light: ["#be123c", "#9f1239", "#fde3e8", "#851032"], dark: ["#fb7185", "#fda4af", "#3a1a22", "#fecdd3"], grad: "linear-gradient(145deg, #fb7185, #e11d48)" },
  slate: { light: ["#334155", "#1e293b", "#e4e8ee", "#1f2937"], dark: ["#94a3b8", "#cbd5e1", "#232a35", "#e2e8f0"], grad: "linear-gradient(145deg, #94a3b8, #475569)" },
};

const ACCENT_PROPS = ["--accent", "--accent-hover", "--accent-soft", "--accent-soft-text", "--accent-contrast", "--focus", "--brand-gradient", "--user-bubble", "--user-bubble-text", "--user-bubble-shadow", "--glow-1", "--glow-strong", "--focus-ring"];

function applyAccent(root: HTMLElement, name: string, dark: boolean, highContrast: boolean) {
  // High contrast has its own fixed palette in tokens.css; inline values would override it.
  if (highContrast) {
    ACCENT_PROPS.forEach((p) => root.style.removeProperty(p));
    return;
  }
  const accent = ACCENTS[name] ?? ACCENTS.teal;
  const [main, hover, soft, softText] = dark ? accent.dark : accent.light;
  // The user bubble carries white text, so it needs a deep fill in both themes:
  // the light palette's darker shades, or the dark palette's bright accent pushed toward black.
  const [lightMain, lightHover] = accent.light;
  const bubble = dark
    ? `linear-gradient(145deg, color-mix(in srgb, ${main} 55%, #000), color-mix(in srgb, ${main} 40%, #000))`
    : `linear-gradient(145deg, ${lightMain}, ${lightHover})`;
  root.style.setProperty("--accent", main);
  root.style.setProperty("--accent-hover", hover);
  root.style.setProperty("--accent-soft", soft);
  root.style.setProperty("--accent-soft-text", softText);
  root.style.setProperty("--accent-contrast", dark ? "#05201c" : "#ffffff");
  root.style.setProperty("--focus", dark ? hover : main);
  root.style.setProperty("--brand-gradient", accent.grad);
  root.style.setProperty("--user-bubble", bubble);
  root.style.setProperty("--user-bubble-text", "#ffffff");
  root.style.setProperty("--user-bubble-shadow", dark ? "rgba(0, 0, 0, 0.25)" : `color-mix(in srgb, ${main} 22%, transparent)`);
  root.style.setProperty("--glow-1", `color-mix(in srgb, ${main} ${dark ? "12%" : "16%"}, transparent)`);
  root.style.setProperty("--glow-strong", `color-mix(in srgb, ${main} 28%, transparent)`);
  root.style.setProperty("--focus-ring", `color-mix(in srgb, ${main} 16%, transparent)`);
}

export function applyAppearance(s: Settings) {
  const root = document.documentElement;
  const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
  const resolve = () => (s.general.theme === "system" ? (mq?.matches ? "dark" : "light") : s.general.theme);
  root.dataset.theme = resolve();
  root.dataset.contrast = s.general.highContrast ? "high" : "normal";
  root.style.setProperty("--font-scale", String(s.general.fontScale || 1));
  applyAccent(root, s.general.accent, resolve() === "dark", s.general.highContrast);
  if (mq) {
    if (mediaListener) mq.removeEventListener?.("change", mediaListener);
    mediaListener = () => {
      root.dataset.theme = resolve();
      applyAccent(root, s.general.accent, resolve() === "dark", s.general.highContrast);
    };
    mq.addEventListener?.("change", mediaListener);
  }
}
