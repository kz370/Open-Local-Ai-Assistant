import { useState } from "react";
import { t } from "../../app/strings";

const CODE_MAP: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Backquote: "Backquote",
  Minus: "Minus",
  Equal: "Equal",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
};

/** Converts a keyboard event into a Tauri accelerator string, or null if incomplete. */
export function acceleratorFromEvent(e: Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
  let key: string | null = null;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.code)) key = e.code;
  else if (/^Numpad[0-9]$/.test(e.code)) key = `Num${e.code.slice(6)}`;
  else if (CODE_MAP[e.code]) key = CODE_MAP[e.code];
  if (!key) return null;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("CommandOrControl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  const isFunctionKey = /^F\d+$/.test(key);
  if (!mods.length && !isFunctionKey) return null;
  return [...mods, key].join("+");
}

export function prettyAccelerator(acc: string): string {
  return acc
    .split("+")
    .map((p) => (p === "CommandOrControl" ? "Ctrl" : p === "Super" ? "Win" : p))
    .join(" + ");
}

export function ShortcutInput(props: { value: string; defaultValue: string; onChange: (v: string) => void; label: string }) {
  const [recording, setRecording] = useState(false);
  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
      <button
        type="button"
        className={`input kbd-input${recording ? " recording" : ""}`}
        aria-label={`${props.label}: ${prettyAccelerator(props.value)}`}
        onClick={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={(e) => {
          if (!recording) return;
          e.preventDefault();
          e.stopPropagation();
          if (e.key === "Escape") {
            setRecording(false);
            return;
          }
          const acc = acceleratorFromEvent(e.nativeEvent);
          if (acc) {
            props.onChange(acc);
            setRecording(false);
          }
        }}
      >
        {recording ? t("settings.shortcuts.record") : prettyAccelerator(props.value) || "—"}
      </button>
      <button type="button" className="btn btn-sm" onClick={() => props.onChange(props.defaultValue)} disabled={props.value === props.defaultValue}>
        {t("settings.shortcuts.reset")}
      </button>
    </div>
  );
}
