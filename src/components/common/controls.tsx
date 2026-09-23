import { useEffect, useRef, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { providerLabel } from "../../app/providers";
import { useSettings } from "../../app/settingsStore";
import { errorMessage, t } from "../../app/strings";
import type { AppErrorPayload } from "../../app/types";

export function Switch(props: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean; id?: string }) {
  return (
    <button
      id={props.id}
      type="button"
      role="switch"
      className="switch"
      aria-checked={props.checked}
      aria-label={props.label}
      disabled={props.disabled}
      onClick={() => props.onChange(!props.checked)}
    />
  );
}

export function Segmented<T extends string>(props: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <div className="segmented" role="radiogroup" aria-label={props.label}>
      {props.options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={props.value === o.value} onClick={() => props.onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Dialog(props: { title: string; children: ReactNode; actions: ReactNode; onClose: () => void; labelledBy?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>("button, input, select, textarea");
    first?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") props.onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      prev?.focus?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && props.onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-label={props.title} ref={ref}>
        <h2>{props.title}</h2>
        {props.children}
        <div className="dialog-actions">{props.actions}</div>
      </div>
    </div>
  );
}

export function ErrorNotice(props: { error: AppErrorPayload; actions?: ReactNode }) {
  const provider = useSettings((s) => s.settings?.ai.provider);
  return (
    <div className="notice err" role="alert">
      <div style={{ flex: 1 }}>
        <div>{errorMessage(props.error.code, { provider: providerLabel(provider) })}</div>
        {props.error.detail && (
          <details className="tech">
            <summary>{t("errors.details")}</summary>
            <pre>{props.error.detail}</pre>
          </details>
        )}
        {props.actions && <div style={{ display: "flex", gap: 6, marginTop: 8 }}>{props.actions}</div>}
      </div>
    </div>
  );
}

export function openExternal(url: string) {
  if (/^https?:\/\//i.test(url)) openUrl(url).catch((e) => console.warn("could not open link", url, e));
}

/** Detects the base direction of text for correct Arabic/English rendering. */
export function textDir(text: string, language?: string | null): "rtl" | "ltr" | "auto" {
  if (language === "ar") return "rtl";
  if (language === "en" || language === "de") return "ltr";
  const firstStrong = text.match(/[֐-ࣿיִ-﷿ﹰ-﻿]|[A-Za-zÀ-ɏ]/);
  if (!firstStrong) return "auto";
  return /[A-Za-zÀ-ɏ]/.test(firstStrong[0]) ? "ltr" : "rtl";
}
