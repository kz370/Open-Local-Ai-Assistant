import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useChat } from "../app/chatStore";
import { useSettings } from "../app/settingsStore";
import { Composer } from "../components/chat/Composer";
import type { ServerStatus } from "../app/types";

const server = (id: string, name: string, enabled: boolean): ServerStatus => ({
  config: { id, name, description: "", transport: "stdio", command: "x", args: [], env: {}, url: null, headers: {}, enabled, source: "user", createdAt: "" },
  state: enabled ? "connected" : "disabled",
  error: null,
  tools: [],
  internet: false,
});

/** Fires a backend event through the listener the component registered. */
function emit(event: string) {
  const call = vi.mocked(listen).mock.calls.find(([name]) => name === event);
  if (!call) throw new Error(`nothing is listening to ${event}`);
  (call[1] as (e: unknown) => void)({ event, id: 0, payload: null });
}

describe("MCP dropdown", () => {
  beforeEach(() => {
    useSettings.setState({ settings: { general: {}, search: { enabled: false }, tts: {}, ai: {} } as never });
    useChat.setState({ draft: "", turnId: null, sessionMcpEnabled: {}, sessionWebSearch: false });
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockClear();
  });

  it("lists only servers enabled in Settings, off by default, and toggles one for this chat", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) =>
      cmd === "mcp_list" ? [server("s1", "Files", true), server("s2", "Git", true), server("s3", "Old", false)] : undefined,
    );
    render(<Composer onVoiceSetup={() => {}} />);

    const chip = await screen.findByLabelText("MCP tools");
    await waitFor(() => expect(chip).toHaveTextContent("0/2"));
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.click(chip);
    await waitFor(() => expect(screen.getByRole("dialog", { name: "MCP tools for this chat" })).toBeInTheDocument());
    expect(screen.getByText("Files")).toBeInTheDocument();
    expect(screen.getByText("Git")).toBeInTheDocument();
    expect(screen.queryByText("Old")).toBeNull();

    const toggle = screen.getByLabelText("Use Files in this chat");
    expect(toggle).toHaveAttribute("aria-checked", "false");
    fireEvent.click(toggle);
    expect(useChat.getState().sessionMcpEnabled).toEqual({ s1: true, s2: false, s3: false });
    await waitFor(() => expect(chip).toHaveTextContent("1/2"));
    // Per-chat choice only: nothing is written back to the server config.
    expect(invoke).not.toHaveBeenCalledWith("mcp_set_enabled", expect.anything());
  });

  it("hides the button when no MCP server is enabled", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => (cmd === "mcp_list" ? [server("s1", "Files", false)] : undefined));
    const { container } = render(<Composer onVoiceSetup={() => {}} />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("mcp_list"));
    expect(screen.queryByLabelText("MCP tools")).toBeNull();
    expect(container.querySelector(".composer-sep")).toBeNull();
  });

  it("picks up a server enabled in Settings mid-conversation without a new chat", async () => {
    let enabled = false;
    const turns: Record<string, boolean>[] = [];
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "mcp_list") return enabled ? [server("s1", "Files", true)] : [];
      if (cmd === "chat_send") turns.push((args as { input: { mcpEnabled: Record<string, boolean> } }).input.mcpEnabled);
      return undefined;
    });
    render(<Composer onVoiceSetup={() => {}} />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("mcp_list"));
    expect(screen.queryByLabelText("MCP tools")).toBeNull();

    // First turn of the conversation: no MCP server is enabled in Settings.
    await useChat.getState().send("what is the weather?");
    expect(turns).toEqual([{}]);

    // The user enables the server in Settings mid-conversation.
    enabled = true;
    emit("mcp://changed");
    fireEvent.click(await screen.findByLabelText("MCP tools"));
    const toggle = await screen.findByLabelText("Use Files in this chat");
    fireEvent.click(toggle);

    // The first reply finished; the conversation is still the same one.
    useChat.setState({ turnId: null });
    await useChat.getState().send("and now the time?");
    expect(turns[1]).toEqual({ s1: true });
    // Same conversation, no newConversation() needed.
    expect(useChat.getState().conversationId).toBeNull();
  });

  it("opens the MCP settings section from the gear in the dropdown", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => (cmd === "mcp_list" ? [server("s1", "Files", true)] : undefined));
    render(<Composer onVoiceSetup={() => {}} />);
    fireEvent.click(await screen.findByLabelText("MCP tools"));
    await waitFor(() => expect(screen.getByRole("dialog", { name: "MCP tools for this chat" })).toBeInTheDocument());
    fireEvent.click(screen.getByLabelText("Manage MCP servers"));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("open_settings_window", { section: "mcp" }));
    // The dropdown closes; no server state is written from the chat window.
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(invoke).not.toHaveBeenCalledWith("mcp_set_enabled", expect.anything());
  });

  it("sends the per-server switches on every turn and keeps them across a new chat", async () => {
    const turns: Record<string, boolean>[] = [];
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "mcp_list") return [server("s1", "Files", true)];
      if (cmd === "chat_send") turns.push((args as { input: { mcpEnabled: Record<string, boolean> } }).input.mcpEnabled);
      return undefined;
    });
    render(<Composer onVoiceSetup={() => {}} />);
    fireEvent.click(await screen.findByLabelText("MCP tools"));
    await waitFor(() => expect(screen.getByLabelText("Use Files in this chat")).toBeInTheDocument());
    fireEvent.click(screen.getByLabelText("Use Files in this chat"));

    await useChat.getState().send("hi");
    expect(turns).toEqual([{ s1: true }]);

    useChat.getState().newConversation();
    expect(useChat.getState().sessionMcpEnabled).toEqual({ s1: true });
    await useChat.getState().send("again");
    expect(turns[1]).toEqual({ s1: true });
  });

  it("does not re-enable a server the user switched off when the list refreshes", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => (cmd === "mcp_list" ? [server("s1", "Files", true)] : undefined));
    render(<Composer onVoiceSetup={() => {}} />);
    fireEvent.click(await screen.findByLabelText("MCP tools"));
    await waitFor(() => expect(screen.getByLabelText("Use Files in this chat")).toBeInTheDocument());
    fireEvent.click(screen.getByLabelText("Use Files in this chat"));
    // The backend emits mcp://changed after every connect, save and delete.
    emit("mcp://changed");
    emit("mcp://changed");
    expect(useChat.getState().sessionMcpEnabled).toEqual({ s1: true });
  });
});
