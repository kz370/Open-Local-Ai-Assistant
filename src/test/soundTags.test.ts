import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { stripSoundTags } from "../app/soundTags";
import { t } from "../app/strings";

describe("sound tags", () => {
  it("are hidden from shown and copied text", () => {
    expect(stripSoundTags("So funny! <laugh> Next.")).toBe("So funny! Next.");
    expect(stripSoundTags("Wait <breath>, ok")).toBe("Wait, ok");
    expect(stripSoundTags("ضحكت <Laugh>،")).toBe("ضحكت،");
    expect(stripSoundTags("if a < b and c > d")).toBe("if a < b and c > d");
  });

  it("settings show the same default instruction the model gets", () => {
    const rust = readFileSync("src-tauri/src/services/chat/prompt.rs", "utf8");
    const m = rust.match(/DEFAULT_EXPRESSIVE_INSTRUCTION: &str = "([\s\S]*?)";/);
    expect(m).not.toBeNull();
    const text = m![1].replace(/\\r?\n\s*/g, "");
    expect(t("settings.voice.expressiveInstructionDefault")).toBe(text);
  });
});
