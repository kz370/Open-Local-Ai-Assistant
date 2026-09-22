import { create } from "zustand";

/** Momentary highlight target set by settings search; consumed by Row and cleared after its pulse. */
interface HighlightState {
  term: string | null;
  set: (term: string) => void;
  clear: () => void;
}

export const useSettingsHighlight = create<HighlightState>((set) => ({
  term: null,
  set: (term) => set({ term }),
  clear: () => set({ term: null }),
}));
