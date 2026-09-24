import { beforeAll, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { VoiceEvent } from "../app/types";
import { useVoice } from "../app/voiceStore";

async function voiceEvents() {
  await useVoice.getState().subscribe();
  const call = vi.mocked(listen).mock.calls.find(([name]) => name === "voice://event")!;
  const handler = call[1] as (e: { payload: VoiceEvent }) => void;
  return (payload: VoiceEvent) => handler({ payload });
}

const state = (session: number, s: "listening" | "idle"): VoiceEvent => ({ type: "state", mode: "pushToTalk", session, state: s, device: null, streaming: false });

describe("listening sessions", () => {
  // Listeners register once per window, so the handler is captured once.
  let emit: (payload: VoiceEvent) => void;
  beforeAll(async () => {
    emit = await voiceEvents();
  });

  it("a late idle of the previous session does not end the new one", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(1);
    await useVoice.getState().startPushToTalk();
    expect(useVoice.getState().phase).toBe("listening");

    // Starting again stops session 1; its idle arrives after session 2 began.
    vi.mocked(invoke).mockResolvedValueOnce(2);
    await useVoice.getState().startPushToTalk();
    emit(state(1, "idle"));
    expect(useVoice.getState().phase).toBe("listening");
    expect(useVoice.getState().mode).toBe("pushToTalk");

    emit(state(2, "idle"));
    expect(useVoice.getState().phase).toBe("idle");
    expect(useVoice.getState().mode).toBeNull();
  });

  it("a session that ended before its start returned stays ended", async () => {
    let resolve!: (n: number) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((r) => (resolve = r as (n: number) => void)));
    const started = useVoice.getState().startPushToTalk();
    emit(state(3, "listening"));
    emit(state(3, "idle"));
    resolve(3);
    await started;
    expect(useVoice.getState().phase).toBe("idle");
  });
});
