// UI strings (English only). Keys are dotted paths into strings.en.json.

import en from "./strings.en.json";

type Dict = { [k: string]: string | Dict };

function lookup(dict: Dict, key: string): string | undefined {
  let cur: string | Dict | undefined = dict;
  for (const part of key.split(".")) {
    if (typeof cur !== "object" || cur === null) return undefined;
    cur = cur[part];
  }
  return typeof cur === "string" ? cur : undefined;
}

export function t(key: string, vars?: Record<string, string | number>): string {
  const s = lookup(en as Dict, key) ?? key;
  if (!vars) return s;
  return s.replace(/\{\{(\w+)\}\}/g, (_, name: string) => (name in vars ? String(vars[name]) : `{{${name}}}`));
}

export function hasString(key: string): boolean {
  return lookup(en as Dict, key) !== undefined;
}

export function errorMessage(code: string): string {
  return hasString(`errors.${code}`) ? t(`errors.${code}`) : t("errors.other");
}

export function formatBytes(bytes: number): string {
  if (!bytes) return "0 MB";
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  return `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}

export function languageName(code: string | null | undefined): string {
  return code && hasString(`languages.${code}`) ? t(`languages.${code}`) : code ?? "";
}

/** Display name for an LM Studio model: user alias, else the id without publisher, truncated. */
export function modelLabel(id: string | null | undefined, aliases: Record<string, string> | undefined, max = 28): string {
  if (!id) return "";
  const alias = aliases?.[id]?.trim();
  if (alias) return alias;
  const short = id.includes("/") ? id.slice(id.lastIndexOf("/") + 1) : id;
  return short.length > max ? `${short.slice(0, max - 1)}…` : short;
}
