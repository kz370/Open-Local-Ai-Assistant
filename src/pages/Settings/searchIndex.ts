import en from "../../app/strings.en.json";

type Dict = { [k: string]: string | Dict };

export interface SettingEntry {
  text: string;
  sectionId: string;
}

const SKIP_KEYS = new Set(["title", "sections", "groups", "quickSearch"]);

function flatten(node: Dict, sectionId: string, out: SettingEntry[]) {
  for (const [key, val] of Object.entries(node)) {
    if (typeof val === "string") {
      // *Hint strings are descriptions of the sibling setting, not a distinct one to jump to.
      if (key.endsWith("Hint") || val.length < 2) continue;
      out.push({ text: val, sectionId });
    } else if (val && typeof val === "object") {
      flatten(val, sectionId, out);
    }
  }
}

/** Builds a flat, searchable list of every setting label/title, mapped back to its section. */
export function buildSettingsIndex(sectionIds: string[]): SettingEntry[] {
  const settingsDict = (en as { settings: Dict }).settings;
  const out: SettingEntry[] = [];
  for (const [key, val] of Object.entries(settingsDict)) {
    if (SKIP_KEYS.has(key) || typeof val !== "object" || val === null) continue;
    const sectionId = sectionIds.includes(key) ? key : "general";
    flatten(val as Dict, sectionId, out);
  }
  const seen = new Set<string>();
  return out.filter((e) => {
    const k = `${e.sectionId}::${e.text}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}
