import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useChat } from "../app/chatStore";
import { useSettings } from "../app/settingsStore";
import type { ExplainEvent, Settings } from "../app/types";
import { SelectionMenu } from "../components/chat/SelectionMenu";

// jsdom has no layout; a selection still needs a rectangle to anchor to.
Range.prototype.getBoundingClientRect = () => new DOMRect(40, 40, 80, 16);

function renderReply() {
  render(
    <>
      <div className="messages">
        <div className="msg assistant">
          <div className="bubble" lang="de" data-message-id="m7">
            <p>Pflanzen nutzen Photosynthese, um Energie zu gewinnen.</p>
          </div>
        </div>
      </div>
      <SelectionMenu />
    </>,
  );
}

function selectWord(word: string) {
  const p = document.querySelector(".bubble p")!;
  const text = p.firstChild!;
  const start = text.textContent!.indexOf(word);
  const range = document.createRange();
  range.setStart(text, start);
  range.setEnd(text, start + word.length);
  const sel = window.getSelection()!;
  sel.removeAllRanges();
  sel.addRange(range);
  fireEvent.contextMenu(p, { clientX: 50, clientY: 60 });
}

function useExplainMode(mode: "chat" | "popup") {
  useSettings.setState({ settings: { ai: { explainMode: mode } } as unknown as Settings });
}

describe("selected text menu", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
    window.getSelection()?.removeAllRanges();
  });

  it("leaves the normal menu alone without a selection", () => {
    renderReply();
    const ev = fireEvent.contextMenu(document.querySelector(".bubble p")!);
    expect(ev).toBe(true); // not prevented
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("speaks the selected words in the reply's language", () => {
    renderReply();
    selectWord("Photosynthese");
    fireEvent.click(screen.getByRole("menuitem", { name: "Speak" }));
    expect(invoke).toHaveBeenCalledWith("tts_speak", { text: "Photosynthese", language: "de", tag: "m7" });
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("explains as a follow-up in the chat", () => {
    useExplainMode("chat");
    const send = vi.fn(async () => undefined);
    useChat.setState({ send });
    renderReply();
    selectWord("Photosynthese");
    fireEvent.click(screen.getByRole("menuitem", { name: "Explain" }));
    expect(send).toHaveBeenCalledWith("Explain this part of your answer:\n\n> Photosynthese", { spokenLanguage: "de" });
    expect(invoke).not.toHaveBeenCalledWith("chat_explain", expect.anything());
  });

  it("explains in a popup, streaming the answer", async () => {
    useExplainMode("popup");
    let onEvent: ((e: ExplainEvent) => void) | undefined;
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "chat_explain") onEvent = (args as { onEvent: { onmessage: (e: ExplainEvent) => void } }).onEvent.onmessage;
      return undefined;
    });
    renderReply();
    selectWord("Photosynthese");
    fireEvent.click(screen.getByRole("menuitem", { name: "Explain" }));
    const call = vi.mocked(invoke).mock.calls.find(([c]) => c === "chat_explain")!;
    expect(call[1]).toMatchObject({ selection: "Photosynthese", passage: "Pflanzen nutzen Photosynthese, um Energie zu gewinnen." });

    act(() => {
      onEvent!({ type: "delta", text: "Plants turn light " });
      onEvent!({ type: "delta", text: "into energy." });
      onEvent!({ type: "done" });
    });
    const dialog = screen.getByRole("dialog", { name: "Explanation" });
    expect(dialog).toHaveTextContent("Plants turn light into energy.");

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("stops an explanation that is still running when its popup closes", () => {
    useExplainMode("popup");
    renderReply();
    selectWord("Energie");
    fireEvent.click(screen.getByRole("menuitem", { name: "Explain" }));
    const id = (vi.mocked(invoke).mock.calls.find(([c]) => c === "chat_explain")![1] as { id: string }).id;
    fireEvent.keyDown(document, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith("chat_stop", { turnId: id });
  });
});
