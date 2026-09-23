import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { errorMessage, formatBytes, hasString, t } from "../app/strings";

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    if (statSync(p).isDirectory()) return f === "test" ? [] : sourceFiles(p);
    return /\.(tsx?)$/.test(f) ? [p] : [];
  });
}

describe("strings", () => {
  it("interpolates variables", () => {
    expect(t("tools.confirmBody", { tool: "write_file", server: "FS" })).toBe("The assistant wants to use “write_file” from FS.");
    expect(t("missing.key")).toBe("missing.key");
  });

  it("maps backend error codes to friendly messages", () => {
    expect(errorMessage("lmstudio_unavailable", { provider: "LM Studio" })).toBe(
      "Unable to connect to LM Studio. If this is LM Studio, make sure its local server is running; for a hosted provider, check your API key and network connection.",
    );
    expect(errorMessage("something_new")).toBe(t("errors.other"));
  });

  it("every literal t() key used in the UI exists", () => {
    const missing: string[] = [];
    for (const file of sourceFiles(join(__dirname, ".."))) {
      const src = readFileSync(file, "utf8");
      for (const m of src.matchAll(/\bt\(\s*"([a-zA-Z0-9_.]+)"/g)) {
        if (!hasString(m[1])) missing.push(`${file}: ${m[1]}`);
      }
    }
    expect(missing).toEqual([]);
  });

  it("covers dynamic key families", () => {
    for (const r of ["already_loaded", "fits_gpu", "fits_ram", "partial_offload", "practical_size", "tool_use", "too_large", "small_context", "low_precision", "specialized", "unknown_size"]) {
      expect(hasString(`settings.ai.reasons.${r}`)).toBe(true);
    }
    for (const c of ["search", "fetch", "read", "write", "execute", "other"]) expect(hasString(`settings.mcp.category.${c}`)).toBe(true);
    for (const s of ["general", "ai", "speech", "voice", "memory", "language", "dictation", "search", "mcp", "privacy", "about", "appearance", "shortcuts", "diagnostics"]) {
      expect(hasString(`settings.sections.${s}`)).toBe(true);
    }
    for (const code of ["lmstudio_unavailable", "stt_unavailable", "tts_unavailable", "mcp", "audio", "timeout", "cancelled"]) expect(hasString(`errors.${code}`)).toBe(true);
  });

  it("formats sizes", () => {
    expect(formatBytes(0)).toBe("0 MB");
    expect(formatBytes(67_200_000)).toBe("64 MB");
    expect(formatBytes(1_100_000_000)).toBe("1.0 GB");
    expect(formatBytes(1_036_600_000)).toBe("989 MB");
  });
});
