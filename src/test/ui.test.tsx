import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen, type Event, type EventCallback } from "@tauri-apps/api/event";
import { useChat, type UiMessage } from "../app/chatStore";
import { useSettings } from "../app/settingsStore";
import type { Attachment } from "../app/types";
import { MessageBubble } from "../components/chat/MessageBubble";
import { Composer } from "../components/chat/Composer";
import { CallView } from "../components/voice/CallView";
import { useVoice } from "../app/voiceStore";
import { SpokenText } from "../components/voice/SpokenText";
import { textDir } from "../components/common/controls";
import { acceleratorFromEvent, prettyAccelerator } from "../components/settings/ShortcutInput";
import { Overlay } from "../pages/Overlay/Overlay";

const base: UiMessage = { id: "m1", role: "assistant", content: "", attachments: [], language: null, reasoning: "", sources: [], tools: [], streaming: false, createdAt: "" };

describe("text direction", () => {
  it("detects Arabic, English, German and mixed text", () => {
    expect(textDir("كيف حالك اليوم؟")).toBe("rtl");
    expect(textDir("How are you today?")).toBe("ltr");
    expect(textDir("Wie geht es dir heute?")).toBe("ltr");
    expect(textDir("ما هو آخر إصدار من PHP؟")).toBe("rtl");
    expect(textDir("PHP ما هو")).toBe("ltr"); // first strong character decides
    expect(textDir("PHP ما هو", "ar")).toBe("rtl"); // detected language wins
    expect(textDir("123")).toBe("auto");
  });
});

describe("SpokenText", () => {
  const on = () => document.querySelector(".spoken-word.on")?.textContent;

  it("shows the whole reply and highlights the word being spoken", async () => {
    const reply = "Hello there. **One** two three four. Goodbye now.";
    const sentence = { tag: "t1", text: "One two three four.", durationMs: 1000, startedAt: performance.now() };
    const { rerender } = render(<SpokenText text={reply} sentence={sentence} paused={false} />);
    // Markdown is not shown, the rest of the reply is.
    expect(document.querySelector(".spoken-text")?.textContent).toBe("Hello there. One two three four. Goodbye now.");
    await waitFor(() => expect(on()).toBe("One"));
    expect(document.querySelectorAll(".spoken-word.done")).toHaveLength(2); // "Hello there." came before

    // Three quarters through the sentence: the third word is spoken.
    rerender(<SpokenText text={reply} sentence={{ ...sentence, startedAt: performance.now() - 700 }} paused={false} />);
    await waitFor(() => expect(on()).toBe("three"));
  });

  it("finds a spoken sentence despite added diacritics", async () => {
    const sentence = { tag: "t1", text: "مَرْحَبًا بِكَ", durationMs: 1000, startedAt: performance.now() };
    render(<SpokenText text="أهلا. مرحبا بك" sentence={sentence} paused={false} />);
    await waitFor(() => expect(on()).toBe("مرحبا"));
  });

  it("holds the highlight while speech is paused or between sentences", async () => {
    const sentence = { tag: "t1", text: "alpha beta gamma", durationMs: 600, startedAt: performance.now() - 590 };
    const { rerender } = render(<SpokenText text="alpha beta gamma delta" sentence={sentence} paused={true} />);
    await new Promise((r) => setTimeout(r, 50));
    expect(on()).toBe("gamma");
    rerender(<SpokenText text="alpha beta gamma delta" sentence={null} paused={false} />);
    expect(on()).toBe("gamma");
  });
});

describe("MessageBubble", () => {
  it("renders Arabic assistant messages RTL while keeping code and URLs LTR", () => {
    const m = { ...base, language: "ar", content: "هذا مثال:\n\n```php\necho 'hi';\n```\n\nراجع [الموقع](https://php.net)" };
    const { container } = render(<MessageBubble message={m} developer={false} showReasoning={false} isLastAssistant />);
    expect(container.querySelector(".bubble")).toHaveAttribute("dir", "rtl");
    expect(container.querySelector(".bubble")).toHaveAttribute("lang", "ar");
    expect(container.querySelector(".codeblock")).toHaveAttribute("dir", "ltr");
    expect(screen.getByText("الموقع").closest("a")).toHaveAttribute("dir", "ltr");
  });

  it("renders user English messages LTR and never renders raw HTML", () => {
    const { container } = render(<MessageBubble message={{ ...base, role: "user", content: "Hello <b>world</b>" }} developer={false} showReasoning={false} isLastAssistant={false} />);
    expect(container.querySelector(".bubble")).toHaveAttribute("dir", "ltr");
    expect(container.querySelector("b")).toBeNull();
    const a = render(<MessageBubble message={{ ...base, content: "Hi <img src=x onerror=alert(1)> there" }} developer={false} showReasoning={false} isLastAssistant={false} />);
    expect(a.container.querySelector("img")).toBeNull();
  });

  it("shows friendly tool status, sources, and developer details", () => {
    const m: UiMessage = {
      ...base,
      content: "PHP 8.5 is the latest.",
      sources: [{ url: "https://www.php.net/releases/", title: "PHP releases" }],
      tools: [{ callId: "c1", serverName: "Web Search", toolName: "searxng_web_search", category: "search", args: { query: "latest php" }, status: "done", durationMs: 812, resultPreview: "..." }],
    };
    const { rerender } = render(<MessageBubble message={m} developer={false} showReasoning={false} isLastAssistant />);
    expect(screen.getByText("Web search completed")).toBeInTheDocument();
    expect(screen.getByText("PHP releases")).toBeInTheDocument();
    expect(screen.queryByText("Arguments")).toBeNull();
    rerender(<MessageBubble message={m} developer showReasoning={false} isLastAssistant />);
    expect(screen.getByText("Arguments")).toBeInTheDocument();
    expect(screen.getByText(/812 ms/, { selector: "dd" })).toBeInTheDocument();
  });

  it("shows running search status", () => {
    const m: UiMessage = { ...base, streaming: true, tools: [{ callId: "c1", serverName: "S", toolName: "search", category: "search", args: {}, status: "running" }] };
    render(<MessageBubble message={m} developer={false} showReasoning={false} isLastAssistant />);
    expect(screen.getByText("Searching the web…")).toBeInTheDocument();
  });

  it("shows LM Studio unavailable error with retry", () => {
    const onRetry = vi.fn();
    render(<MessageBubble message={{ ...base, error: { code: "lmstudio_unavailable", detail: "ECONNREFUSED 127.0.0.1:1234" } }} developer={false} showReasoning={false} isLastAssistant onRetry={onRetry} onOpenSettings={() => {}} />);
    expect(screen.getByText(/Unable to connect to LM Studio/)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Retry"));
    expect(onRetry).toHaveBeenCalled();
    expect(screen.getByText("Open Settings")).toBeInTheDocument();
  });

  it("shows a spinner after Read aloud until the audio starts", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    render(<MessageBubble message={{ ...base, id: "m9", content: "Hello there." }} developer={false} showReasoning={false} />);
    fireEvent.click(screen.getByLabelText("Read aloud"));
    expect(document.querySelector(".msg-actions .spinner")).not.toBeNull();
    useVoice.setState({ speakingTag: "m9", paused: false });
    await waitFor(() => expect(document.querySelector(".msg-actions .spinner")).toBeNull());
    expect(screen.getByLabelText("Pause")).toBeInTheDocument();
    useVoice.setState({ speakingTag: null });
  });

  it("offers Send again on a user message that got no answer", () => {
    const retryLast = vi.fn(async () => {});
    useChat.setState({ retryLast });
    render(<MessageBubble message={{ ...base, role: "user", content: "hello?" }} developer={false} showReasoning={false} onResend={() => void retryLast()} onOpenSettings={() => {}} />);
    fireEvent.click(screen.getByLabelText("Send again"));
    expect(retryLast).toHaveBeenCalled();
  });
});

describe("chat store", () => {
  beforeEach(() => {
    useChat.setState({ conversationId: null, messages: [], turnId: null, draft: "", attachments: [], queue: [], confirmation: null, connectionError: null, voiceNotice: null });
    vi.mocked(invoke).mockReset();
  });

  it("streams a turn: started → deltas → tool → done", async () => {
    let turnId = "";
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "chat_send") turnId = (args as { input: { turnId: string } }).input.turnId;
      return undefined;
    });
    await useChat.getState().send("What is the latest PHP version?");
    const h = useChat.getState().handleEvent;
    const userMessage = { id: "u1", conversationId: "c1", role: "user" as const, content: "What is the latest PHP version?", language: "en", createdAt: "" };
    h({ type: "started", turnId, conversationId: "c1", userMessage, model: "qwen", language: "en", newConversation: true });
    h({ type: "toolStarted", turnId, callId: "k", serverName: "Web", toolName: "search", category: "search", args: {} });
    h({ type: "toolFinished", turnId, callId: "k", ok: true, durationMs: 5, resultPreview: "", sources: [{ url: "https://php.net" }], denied: false });
    h({ type: "delta", turnId, text: "PHP " });
    h({ type: "delta", turnId, text: "8.5" });
    // Deltas are applied in batches.
    await new Promise((r) => setTimeout(r, 80));
    let s = useChat.getState();
    expect(s.conversationId).toBe("c1");
    expect(s.messages[1].content).toBe("PHP 8.5");
    expect(s.messages[1].sources).toHaveLength(1);
    expect(s.messages[1].tools[0].status).toBe("done");
    h({ type: "done", turnId, message: { id: "a1", conversationId: "c1", role: "assistant", content: "PHP 8.5", language: "en", sources: [{ url: "https://php.net" }], createdAt: "" } });
    s = useChat.getState();
    expect(s.turnId).toBeNull();
    expect(s.messages.map((m) => m.id)).toEqual(["u1", "a1"]);
    expect(s.messages[1].tools).toHaveLength(1);
  });

  it("surfaces LM Studio unavailability and ignores stale turns", async () => {
    let turnId = "";
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "chat_send") turnId = (args as { input: { turnId: string } }).input.turnId;
    });
    await useChat.getState().send("hi");
    useChat.getState().handleEvent({ type: "delta", turnId: "old-turn", text: "stale" });
    expect(useChat.getState().messages[1].content).toBe("");
    useChat.getState().handleEvent({ type: "error", turnId, code: "lmstudio_unavailable", detail: "refused", partialMessage: null });
    const s = useChat.getState();
    expect(s.connectionError?.code).toBe("lmstudio_unavailable");
    expect(s.messages[1].error?.code).toBe("lmstudio_unavailable");
  });

  it("tracks tool confirmation requests", async () => {
    let turnId = "";
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "chat_send") turnId = (args as { input: { turnId: string } }).input.turnId;
    });
    await useChat.getState().send("write a file");
    useChat.getState().handleEvent({ type: "toolAwaitingConfirmation", turnId, callId: "w1", serverName: "FS", toolName: "write_file", category: "write", args: { path: "a.txt" } });
    expect(useChat.getState().confirmation?.toolName).toBe("write_file");
    useChat.getState().confirmTool(false);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("chat_confirm_tool", { callId: "w1", approved: false });
    expect(useChat.getState().confirmation).toBeNull();
  });

  it("queues typed messages while a reply streams and sends them after", async () => {
    const turns: string[] = [];
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "chat_send") turns.push((args as { input: { turnId: string } }).input.turnId);
    });
    await useChat.getState().send("first");
    await useChat.getState().send("second");
    expect(turns).toHaveLength(1);
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith("chat_stop", expect.anything());
    expect(useChat.getState().queue.map((q) => q.text)).toEqual(["second"]);
    useChat.getState().handleEvent({ type: "done", turnId: turns[0], message: { id: "a1", conversationId: "c1", role: "assistant", content: "ok", language: "en", sources: [], createdAt: "" } });
    await Promise.resolve();
    expect(turns).toHaveLength(2);
    expect(useChat.getState().queue).toHaveLength(0);
    expect(useChat.getState().messages.at(-2)?.content).toBe("second");
  });

  it("returns queued messages to the composer when stopped", async () => {
    vi.mocked(invoke).mockImplementation(async () => undefined);
    await useChat.getState().send("first");
    await useChat.getState().send("second");
    useChat.getState().stop();
    const s = useChat.getState();
    expect(s.queue).toHaveLength(0);
    expect(s.draft).toBe("second");
  });
});

function attachment(): Attachment {
  return { id: "a1", name: "report.pdf", mime: "application/pdf", kind: "binary", sizeBytes: 2048, textChars: 0, truncated: false, note: null, createdAt: "" };
}

describe("Composer", () => {
  it("explains microphone startup failures and exposes the backend detail", () => {
    useVoice.setState({
      error: { code: "audio", detail: "Audio device error: could not open microphone: device unavailable" },
    });
    render(<Composer onVoiceSetup={() => {}} />);
    expect(screen.getByText("The audio device could not be used.")).toBeInTheDocument();
    expect(screen.getByText(/failed to start a capture stream/)).toBeInTheDocument();
    expect(screen.getByText(/allow desktop apps to access the microphone/)).toBeInTheDocument();
    const detail = screen.getByText(/device unavailable/).closest("details");
    expect(detail).not.toHaveAttribute("open");
    fireEvent.click(screen.getByText("Technical details"));
    expect(detail).toHaveAttribute("open");
    useVoice.setState({ error: null });
  });

  it("sends on Enter, keeps Shift+Enter for newlines, and supports RTL input", () => {
    const send = vi.fn(async () => {});
    useChat.setState({ draft: "", turnId: null, send });
    render(<Composer onVoiceSetup={() => {}} />);
    const ta = screen.getByLabelText("Type a message…") as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: "مرحبا" } });
    expect(ta).toHaveAttribute("dir", "rtl");
    fireEvent.keyDown(ta, { key: "Enter", shiftKey: true });
    expect(send).not.toHaveBeenCalled();
    fireEvent.keyDown(ta, { key: "Enter" });
    expect(send).toHaveBeenCalledWith("مرحبا");
  });

  it("shows stop button while generating", () => {
    useChat.setState({ draft: "", turnId: "t" });
    render(<Composer onVoiceSetup={() => {}} />);
    expect(screen.getByLabelText("Stop generating")).toBeInTheDocument();
  });

  it("sends an attachment-only message and can drop a staged file", () => {
    const send = vi.fn(async () => {});
    const removeAttachment = vi.fn();
    useChat.setState({ draft: "", turnId: null, send, attachments: [attachment()], attachmentErrors: [], removeAttachment });
    render(<Composer onVoiceSetup={() => {}} />);
    expect(screen.getByText("report.pdf")).toBeInTheDocument();

    const sendButton = screen.getByLabelText("Send");
    expect(sendButton).toBeEnabled();
    fireEvent.click(sendButton);
    expect(send).toHaveBeenCalledWith("");

    fireEvent.click(screen.getByLabelText("Remove report.pdf"));
    expect(removeAttachment).toHaveBeenCalledWith("a1");
  });

  it("attaches pasted text that is longer than the configured limit", async () => {
    const staged = { ...attachment(), id: "a2", name: "pasted-text.txt", kind: "text" as const };
    vi.mocked(invoke).mockImplementation(async (cmd: string) => (cmd === "attach_text" ? staged : undefined));
    useSettings.setState({ settings: { ai: { pasteAsFileChars: 10 }, tts: {} } as never });
    const addAttachments = vi.fn();
    useChat.setState({ draft: "", turnId: null, attachments: [], attachmentErrors: [], addAttachments });
    render(<Composer onVoiceSetup={() => {}} />);

    const paste = (text: string) => {
      const data = { files: [] as File[], getData: () => text } as unknown as DataTransfer;
      fireEvent.paste(screen.getByLabelText("Type a message…"), { clipboardData: data });
    };
    paste("short");
    expect(invoke).not.toHaveBeenCalledWith("attach_text", expect.anything());

    paste("x".repeat(40));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("attach_text", { name: "pasted-text.txt", text: "x".repeat(40) }));
    await waitFor(() => expect(addAttachments).toHaveBeenCalledWith([staged]));
    useSettings.setState({ settings: null });
  });
});

describe("CallView", () => {
  beforeEach(() => {
    useSettings.setState({ settings: { stt: {}, tts: {} } as never });
    useChat.setState({ messages: [], turnId: null, voiceNotice: null });
  });

  it("mutes the microphone without ending the call", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd, args) => (cmd === "voice_set_muted" ? (args as { muted: boolean }).muted : undefined));
    useVoice.setState({ handsFree: true, muted: false, speaking: false, phase: "listening", levels: [0.5] });
    render(<CallView />);

    fireEvent.click(screen.getByLabelText("Mute microphone"));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("voice_set_muted", { muted: true }));
    await waitFor(() => expect(screen.getByLabelText("Unmute microphone")).toBeInTheDocument());
    expect(useVoice.getState().handsFree).toBe(true);
  });

  it("interrupts the answer only while there is one to interrupt", () => {
    useVoice.setState({ handsFree: true, muted: false, speaking: false, paused: false, phase: "listening", levels: [0] });
    const { rerender } = render(<CallView />);
    expect(screen.getByLabelText("Stop speaking")).toBeDisabled();

    useVoice.setState({ speaking: true });
    rerender(<CallView />);
    const interrupt = screen.getByLabelText("Stop speaking");
    expect(interrupt).toBeEnabled();
    fireEvent.click(interrupt);
    expect(invoke).toHaveBeenCalledWith("tts_stop");
    expect(useVoice.getState().speaking).toBe(false);
  });
});

describe("Overlay review preview", () => {
  const listeners = new Map<string, EventCallback<unknown>>();
  const emit = (name: string, payload: unknown) => {
    act(() => listeners.get(name)?.({ event: name, id: 1, payload } as Event<unknown>));
  };
  const scrollTo = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollTo");
  // jsdom implements neither; the dropdown menus scroll the selected row into view.
  const scrollIntoView = Object.getOwnPropertyDescriptor(Element.prototype, "scrollIntoView");

  beforeEach(() => {
    listeners.clear();
    useSettings.setState({ settings: { dictation: { mode: "toggle", language: "", reviewBeforeInsert: true }, stt: { language: "auto" }, language: { entries: [] } } as never });
    Object.defineProperty(HTMLElement.prototype, "scrollTo", { configurable: true, value: vi.fn() });
    Object.defineProperty(Element.prototype, "scrollIntoView", { configurable: true, value: vi.fn() });
    vi.mocked(listen).mockImplementation(async (event, handler) => {
      listeners.set(event, handler as EventCallback<unknown>);
      return () => {};
    });
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
    useSettings.setState({ settings: null });
    vi.mocked(listen).mockImplementation(async () => () => {});
    if (scrollTo) Object.defineProperty(HTMLElement.prototype, "scrollTo", scrollTo);
    else Reflect.deleteProperty(HTMLElement.prototype, "scrollTo");
    if (scrollIntoView) Object.defineProperty(Element.prototype, "scrollIntoView", scrollIntoView);
    else Reflect.deleteProperty(Element.prototype, "scrollIntoView");
  });

  it("shows live text immediately and enables insert after the final transcript arrives", () => {
    render(<Overlay />);
    emit("dictation://state", { state: "listening" });
    emit("voice://event", { type: "partial", mode: "dictation", text: "live words" });
    fireEvent.click(screen.getByRole("button", { name: "Review" }));
    expect(invoke).toHaveBeenCalledWith("dictation_insert_now", { text: "live words" });

    emit("dictation://state", { state: "reviewing", text: "live words" });
    expect(screen.getByRole("textbox")).toHaveValue("live words");
    expect(screen.getByRole("button", { name: "Insert" })).toBeDisabled();
    expect(screen.getByText("Finalizing dictation; you can edit now and insert when ready.")).toBeInTheDocument();

    emit("dictation://state", { state: "transcribing" });
    emit("dictation://state", { state: "idle" });
    expect(screen.getByText("Transcribing…")).toBeInTheDocument();

    emit("dictation://state", {
      state: "review",
      result: { raw: "live words", inserted: "final words", corrected: false, correctionError: null },
    });
    expect(screen.getByRole("textbox")).toHaveValue("final words");
    expect(screen.getByRole("button", { name: "Insert" })).toBeEnabled();
    emit("dictation://state", { state: "idle" });
    expect(screen.getByRole("textbox")).toHaveValue("final words");
  });

  it("shows and updates the focused destination app tab while listening and reviewing", () => {
    render(<Overlay />);
    emit("dictation://state", { state: "listening" });
    emit("dictation://target", { targetApp: "Notepad" });
    expect(document.querySelector(".overlay-app-tab")).toHaveTextContent("Notepad");

    emit("dictation://target", { targetApp: "Visual Studio Code" });
    expect(document.querySelector(".overlay-app-tab")).toHaveTextContent("Visual Studio Code");

    emit("dictation://state", {
      state: "review",
      result: { raw: "spoken words", inserted: "spoken words", corrected: false, correctionError: null },
    });
    emit("dictation://target", { targetApp: "Word" });
    expect(document.querySelector(".overlay-app-tab")).toHaveTextContent("Word");
  });

  it("supports cancel, retry, and insert shortcuts without taking Alt+Enter", () => {
    render(<Overlay />);
    emit("dictation://state", {
      state: "review",
      result: { raw: "spoken words", inserted: "spoken words", corrected: false, correctionError: null },
      targetApp: "Notepad",
    });
    expect(invoke).toHaveBeenCalledWith("shortcuts_capture", { capturing: true, reviewShortcuts: true });
    expect(document.querySelector(".overlay-app-tab-prefix")).toHaveTextContent("Insert into:");
    expect(document.querySelector(".overlay-app-tab-app")).toHaveTextContent("Notepad");
    expect(document.querySelector(".overlay-app-tab")).toHaveAccessibleName("Insert into: Notepad");
    const editor = screen.getByRole("textbox");

    fireEvent.keyDown(editor, { key: "Enter", altKey: true });
    expect(invoke).not.toHaveBeenCalledWith("dictation_confirm", expect.anything());

    const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith("spoken words");

    emit("dictation://review-shortcut", "cancel");
    expect(invoke).toHaveBeenCalledWith("dictation_cancel");
    emit("dictation://review-shortcut", "retry");
    expect(invoke).toHaveBeenCalledWith("dictation_retry");

    emit("dictation://review-shortcut", "insert");
    expect(invoke).toHaveBeenCalledWith("dictation_confirm", { text: "spoken words" });

    fireEvent.keyDown(editor, { key: "Enter", ctrlKey: true });
    expect(invoke).toHaveBeenCalledWith("dictation_confirm", { text: "spoken words" });
    fireEvent.keyDown(editor, { key: "Enter", ctrlKey: true, shiftKey: true });
    expect(invoke).toHaveBeenCalledWith("dictation_retry");
    fireEvent.keyDown(editor, { key: "Escape" });
    expect(invoke).toHaveBeenCalledWith("dictation_cancel");
    if (clipboard) Object.defineProperty(navigator, "clipboard", clipboard);
    else Reflect.deleteProperty(navigator, "clipboard");
    emit("dictation://state", { state: "cancelled" });
    expect(invoke).toHaveBeenCalledWith("shortcuts_capture", { capturing: false, reviewShortcuts: false });
  });

  it("offers Copy when the destination window cannot be identified", () => {
    render(<Overlay />);
    emit("dictation://state", {
      state: "review",
      result: { raw: "spoken words", inserted: "spoken words", corrected: false, correctionError: null },
    });

    expect(screen.getByText("Insert target unavailable · Copy text instead")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy" })).toBeEnabled();
  });

  it("moves the spectrum bars while the microphone is picked up", () => {
    render(<Overlay />);
    emit("dictation://state", { state: "listening" });
    const bars = () => [...document.querySelectorAll<HTMLElement>(".overlay-head .voice-bars span")].map((s) => s.style.transform);
    const band = (v: number) => new Array(24).fill(v);

    emit("voice://event", { type: "level", mode: "dictation", value: 0.42, bands: band(0.1) });
    const quiet = bars();
    expect(quiet).toHaveLength(48);

    emit("voice://event", { type: "level", mode: "dictation", value: 0.72, bands: band(0.74) });
    const loud = bars();
    expect(loud).not.toEqual(quiet);
    expect(loud[0]).toBe("scaleY(0.74)");
  });

  it("preserves edits made before the final transcript arrives", () => {
    render(<Overlay />);
    emit("dictation://state", { state: "reviewing", text: "live words" });
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "my edit" } });
    emit("dictation://state", {
      state: "review",
      result: { raw: "live words", inserted: "final words", corrected: false, correctionError: null },
    });
    expect(screen.getByRole("textbox")).toHaveValue("my edit");
  });

  it("offers a profile pill only when profiles exist, and saves the pick", async () => {
    render(<Overlay />);
    emit("dictation://state", { state: "listening" });
    expect(screen.queryByRole("button", { name: /^Dictation profile:/ })).not.toBeInTheDocument();

    const many = (n: number) => ({
      dictation: { mode: "toggle", language: "", reviewBeforeInsert: true, activeProfile: "", profiles: Array.from({ length: n }, (_, i) => ({ id: `p${i}`, title: `Profile ${i}`, prompt: "" })) },
      stt: { language: "auto" },
      language: { entries: [] },
    });
    useSettings.setState({ settings: many(3) as never });
    const pill = await screen.findByRole("button", { name: "Dictation profile: No profile" });
    fireEvent.click(pill);
    // The menu is rendered after the window's clip region is widened.
    fireEvent.click(await screen.findByTitle("Profile 1"));
    expect(invoke).toHaveBeenCalledWith("dictation_set_profile", { profile: "p1" });
  });

  it("searches the profile list once it reaches the threshold", async () => {
    const profiles = Array.from({ length: 10 }, (_, i) => ({ id: `p${i}`, title: `Profile ${i}`, prompt: "" }));
    useSettings.setState({
      settings: { dictation: { mode: "toggle", language: "", reviewBeforeInsert: true, activeProfile: "", profiles }, stt: { language: "auto" }, language: { entries: [] } } as never,
    });
    render(<Overlay />);
    emit("dictation://state", { state: "listening" });
    fireEvent.click(await screen.findByRole("button", { name: "Dictation profile: No profile" }));

    const search = await screen.findByPlaceholderText("Search profiles…");
    expect(screen.getByTitle("Profile 9")).toBeInTheDocument();
    fireEvent.change(search, { target: { value: "profile 4" } });
    expect(screen.queryByTitle("Profile 9")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTitle("Profile 4"));
    expect(invoke).toHaveBeenCalledWith("dictation_set_profile", { profile: "p4" });
  });
});

describe("shortcuts", () => {
  const ev = (code: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

  it("enables the global shortcut permission in Tauri", () => {
    const fs = require("node:fs");
    const path = require("node:path");
    const capPath = path.join(process.cwd(), "src-tauri", "capabilities", "default.json");
    const cap = JSON.parse(fs.readFileSync(capPath, "utf8"));
    expect(cap.permissions).toContain("global-shortcut:default");
  });

  it("builds Tauri accelerators including modifier-only combos", () => {
    expect(acceleratorFromEvent(ev("Space", { ctrlKey: true }))).toBe("CommandOrControl+Space");
    expect(acceleratorFromEvent(ev("KeyD", { ctrlKey: true, altKey: true }))).toBe("CommandOrControl+Alt+D");
    expect(acceleratorFromEvent(ev("F9"))).toBe("F9");
    expect(acceleratorFromEvent(ev("KeyA"))).toBeNull(); // no modifier
    // Single modifiers rejected: Alt alone fires on every Alt press (Alt+Tab...).
    expect(acceleratorFromEvent(ev("ControlLeft", { ctrlKey: true }))).toBeNull();
    expect(acceleratorFromEvent(ev("AltLeft", { ctrlKey: true, altKey: true }))).toBe("CommandOrControl+Alt");
    expect(acceleratorFromEvent(ev("MetaLeft", { metaKey: true }))).toBeNull();
    expect(prettyAccelerator("CommandOrControl+Shift+Space")).toBe("Ctrl + Shift + Space");
  });
});
