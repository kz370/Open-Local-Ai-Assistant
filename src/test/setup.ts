import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Tauri APIs are unavailable in jsdom; tests override `invoke` per case.
vi.mock("@tauri-apps/api/core", () => {
  class Channel<T> {
    onmessage: (m: T) => void = () => {};
  }
  return { invoke: vi.fn(async () => undefined), Channel };
});
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => undefined) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string) => ({ matches: false, media: query, addEventListener: () => {}, removeEventListener: () => {} }),
});
