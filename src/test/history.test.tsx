import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { HistoryPanel } from "../components/history/HistoryPanel";

const CONVS = [
  { conversation: { id: "c1", title: "PHP versions", createdAt: "2026-09-17T10:00:00Z", updatedAt: new Date().toISOString(), model: "qwen", language: "en" }, snippet: "latest [PHP] release" },
  { conversation: { id: "c2", title: "الترجمة", createdAt: "2026-09-10T10:00:00Z", updatedAt: "2026-09-10T11:00:00Z", model: null, language: "ar" }, snippet: "" },
];

describe("history panel", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async (cmd: string) => (cmd === "conv_search" ? CONVS : undefined));
  });

  it("lists conversations grouped by recency", async () => {
    render(<HistoryPanel onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText("PHP versions")).toBeInTheDocument());
    expect(screen.getByText("الترجمة")).toBeInTheDocument();
    expect(screen.getByText("Today")).toBeInTheDocument();
    expect(screen.getByText("Earlier")).toBeInTheDocument();
    expect(screen.queryByText("No conversations yet.")).toBeNull();
  });

  it("shows the empty state only when there is nothing", async () => {
    vi.mocked(invoke).mockImplementation(async () => []);
    render(<HistoryPanel onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText("No conversations yet.")).toBeInTheDocument());
  });

  it("surfaces a failing search instead of looking empty", async () => {
    vi.mocked(invoke).mockImplementation(async () => {
      throw { code: "database", detail: "locked" };
    });
    render(<HistoryPanel onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText(/Local storage error/)).toBeInTheDocument());
  });
});
