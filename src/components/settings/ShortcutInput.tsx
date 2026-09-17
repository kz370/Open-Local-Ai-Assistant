import { useEffect, useRef, useState } from "react";
import { ipc } from "../../app/ipc";
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
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("CommandOrControl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");

  const isModifierOnly = ["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight", "OSLeft", "OSRight", "SuperLeft", "SuperRight"].includes(e.code);

  let key: string | null = null;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.code)) key = e.code;
  else if (/^Numpad[0-9]$/.test(e.code)) key = `Num${e.code.slice(6)}`;
  else if (CODE_MAP[e.code]) key = CODE_MAP[e.code];

  if (isModifierOnly) {
    // Single modifiers (e.g. "Alt" alone) fire on every normal Alt press
    // (Alt+Tab, Alt+F4, menu focus). Require at least 2 modifiers.
    return mods.length >= 2 ? mods.join("+") : null;
  }
  if (!key) return null;
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

function isModifierCode(code: string): boolean {
  return ["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight", "OSLeft", "OSRight", "SuperLeft", "SuperRight"].includes(code);
}

function modifierKeyName(code: string): string | null {
  switch (code) {
    case "ControlLeft":
    case "ControlRight":
      return "CommandOrControl";
    case "AltLeft":
    case "AltRight":
      return "Alt";
    case "ShiftLeft":
    case "ShiftRight":
      return "Shift";
    case "MetaLeft":
    case "MetaRight":
    case "OSLeft":
    case "OSRight":
    case "SuperLeft":
    case "SuperRight":
      return "Super";
    default:
      return null;
  }
}

export function ShortcutInput(props: { value: string; defaultValue: string; onChange: (v: string) => void; label: string }) {
  const [recording, setRecording] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);
  const activeModifiersRef = useRef<Set<string>>(new Set());

  // While recording, the app releases its global shortcuts so combinations
  // like Ctrl+Space actually reach this window, and a window-level listener
  // catches keys the button would not receive.
  useEffect(() => {
    if (!recording) {
      activeModifiersRef.current.clear();
      return;
    }
    void ipc.shortcutsCapture(true);
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        activeModifiersRef.current.clear();
        setRecording(false);
        return;
      }
      if (isModifierCode(e.code)) {
        const name = modifierKeyName(e.code);
        if (name) activeModifiersRef.current.add(name);
        return;
      }
      const acc = acceleratorFromEvent(e);
      if (acc) {
        activeModifiersRef.current.clear();
        props.onChange(acc);
        setRecording(false);
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (!isModifierCode(e.code)) return;
      const name = modifierKeyName(e.code);
      if (!name) return;
      if (activeModifiersRef.current.has(name)) {
        const combined = [...activeModifiersRef.current];
        activeModifiersRef.current.clear();
        // Require 2+ modifiers: single Alt/Ctrl/Shift/Super alone would
        // trigger on every normal press of that key. Keep recording.
        if (combined.length >= 2) {
          props.onChange(combined.join("+"));
          setRecording(false);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
      activeModifiersRef.current.clear();
      void ipc.shortcutsCapture(false);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recording]);

  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
      <button
        ref={btnRef}
        type="button"
        className={`input kbd-input${recording ? " recording" : ""}`}
        aria-label={`${props.label}: ${prettyAccelerator(props.value)}`}
        onClick={() => setRecording((v) => !v)}
        onBlur={() => setRecording(false)}
      >
        {recording ? t("settings.shortcuts.record") : prettyAccelerator(props.value) || "—"}
      </button>
      <button type="button" className="btn btn-sm" onClick={() => props.onChange(props.defaultValue)} disabled={props.value === props.defaultValue}>
        {t("settings.shortcuts.reset")}
      </button>
    </div>
  );
}
