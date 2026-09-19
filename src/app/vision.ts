// Whether the model the chat will use can read images.

import { ipc } from "./ipc";
import { useSettings } from "./settingsStore";
import { modelLabel } from "./strings";

/** Providers whose model list says which models take image input. */
const REPORTS_VISION = new Set(["lmstudio", "openrouter"]);

/**
 * The active model's name when it is known not to read images, else null.
 * Models whose provider does not report capabilities are given the benefit
 * of the doubt, so images are never blocked on a guess.
 */
export async function modelWithoutVision(): Promise<string | null> {
  const s = useSettings.getState().settings;
  if (!s || !REPORTS_VISION.has(s.ai.provider)) return null;
  try {
    const id = s.ai.modelMode === "manual" && s.ai.model ? s.ai.model : (await ipc.lmstudioAutoSelection())?.modelId;
    if (!id) return null;
    const model = (await ipc.lmstudioModels(false)).find((m) => m.id === id);
    if (!model || model.kind === "unknown" || model.vision) return null;
    return modelLabel(id, s.ai.modelAliases, 40);
  } catch {
    return null;
  }
}
