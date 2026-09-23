import { useEffect, useState } from "react";
import { Download, Plus, RotateCcw, Trash2, Upload } from "lucide-react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { ipc, toAppError } from "../../../app/ipc";
import { useSettings } from "../../../app/settingsStore";
import { t } from "../../../app/strings";
import type { Settings } from "../../../app/types";
import { Segmented, Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { ShortcutInput } from "../../../components/settings/ShortcutInput";

const DEFAULT_SUGGESTIONS = ["chat.suggestion1", "chat.suggestion2", "chat.suggestion3", "chat.suggestion4"].map((k) => t(k));

export function useS(): [Settings, (m: (d: Settings) => void) => void] {
  const s = useSettings((x) => x.settings)!;
  const update = useSettings((x) => x.update);
  return [s, (m) => void update(m)];
}

export function GeneralSection() {
  const [s, set] = useS();
  const g = s.general;
  return (
    <>
      <SectionHeader title={t("settings.sections.general")} />
      <Card>
        <Row label={t("settings.general.startWithOs")} hint={t("settings.general.startWithOsHint")} htmlFor="sw-os">
          <Switch id="sw-os" label={t("settings.general.startWithOs")} checked={g.startWithOs} onChange={(v) => set((d) => void (d.general.startWithOs = v))} />
        </Row>
        <Row label={t("settings.general.startMinimized")} htmlFor="sw-min">
          <Switch id="sw-min" label={t("settings.general.startMinimized")} checked={g.startMinimized} onChange={(v) => set((d) => void (d.general.startMinimized = v))} />
        </Row>
        <Row label={t("settings.general.alwaysOnTop")} htmlFor="sw-top">
          <Switch id="sw-top" label={t("settings.general.alwaysOnTop")} checked={g.alwaysOnTop} onChange={(v) => set((d) => void (d.general.alwaysOnTop = v))} />
        </Row>
        <Row label={t("settings.general.windowPosition")} hint={t("settings.general.windowPositionHint")} htmlFor="sel-pos">
          <select id="sel-pos" className="select" value={g.windowPosition} onChange={(e) => set((d) => void (d.general.windowPosition = e.target.value as Settings["general"]["windowPosition"]))}>
            {(["bottom-right", "bottom-left", "center", "custom"] as const).map((p) => (
              <option key={p} value={p}>
                {t(`settings.general.positions.${p}`)}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t("settings.general.windowSize")}>
          <span className="row-hint" style={{ margin: 0 }}>
            {g.window.width} × {g.window.height}
          </span>
        </Row>
        <Row label={t("settings.general.compact")} htmlFor="sw-compact">
          <Switch id="sw-compact" label={t("settings.general.compact")} checked={g.compact} onChange={(v) => void ipc.setCompact(v)} />
        </Row>
        <Row label={t("settings.general.developerMode")} hint={t("settings.general.developerModeHint")} htmlFor="sw-dev">
          <Switch id="sw-dev" label={t("settings.general.developerMode")} checked={g.developerMode} onChange={(v) => set((d) => void (d.general.developerMode = v))} />
        </Row>
      </Card>
      <SuggestionsCard />
      <BackupCard />
    </>
  );
}

/** Editable starter prompts shown on the empty chat screen. */
function SuggestionsCard() {
  const [s, set] = useS();
  const [list, setList] = useState(s.general.suggestedPrompts.length ? s.general.suggestedPrompts : DEFAULT_SUGGESTIONS);

  const commit = (next: string[]) => {
    setList(next);
    set((d) => void (d.general.suggestedPrompts = next.map((x) => x.trim()).filter(Boolean)));
  };

  return (
    <Card
      title={t("settings.general.suggestions")}
      actions={
        <button className="btn btn-sm" onClick={() => commit([...list, ""])}>
          <Plus size={13} /> {t("settings.general.suggestionsAdd")}
        </button>
      }
    >
      <p className="row-hint" style={{ margin: "4px 0 10px" }}>
        {t("settings.general.suggestionsHint")}
      </p>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {list.map((text, i) => (
          <div key={i} style={{ display: "flex", gap: 6 }}>
            <input
              className="input"
              dir="auto"
              value={text}
              onChange={(e) => setList(list.map((x, j) => (j === i ? e.target.value : x)))}
              onBlur={() => commit(list)}
            />
            <button className="icon-btn" aria-label={t("app.remove")} title={t("app.remove")} onClick={() => commit(list.filter((_, j) => j !== i))}>
              <Trash2 size={14} />
            </button>
          </div>
        ))}
      </div>
      {s.general.suggestedPrompts.length > 0 && (
        <button className="btn btn-sm" style={{ marginTop: 10 }} onClick={() => commit(DEFAULT_SUGGESTIONS)}>
          <RotateCcw size={13} /> {t("settings.general.suggestionsReset")}
        </button>
      )}
    </Card>
  );
}

const BACKUP_FILTER = [{ name: "Open Local Assistant backup", extensions: ["labackup"] }];

/** Encrypted settings export/import. */
function BackupCard() {
  const update = useSettings((x) => x.update);
  const [includeKeys, setIncludeKeys] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);

  const doExport = async () => {
    const date = new Date().toISOString().slice(0, 10);
    const path = await saveDialog({ defaultPath: `local-assistant-settings-${date}.labackup`, filters: BACKUP_FILTER });
    if (!path) return;
    try {
      await ipc.settingsExport(path, includeKeys);
      setResult({ ok: true, text: t("settings.backup.exported") });
    } catch (e) {
      setResult({ ok: false, text: toAppError(e).detail });
    }
  };

  const doImport = async () => {
    const path = await openDialog({ multiple: false, filters: BACKUP_FILTER });
    if (!path || Array.isArray(path)) return;
    try {
      const merged = await ipc.settingsImport(path);
      // Saved like any edit, so shortcuts, provider, window and so on apply now.
      await update((d) => void Object.assign(d, merged));
      setResult({ ok: true, text: t("settings.backup.imported") });
    } catch (e) {
      setResult({ ok: false, text: toAppError(e).detail });
    }
  };

  return (
    <Card title={t("settings.backup.title")}>
      <p className="row-hint" style={{ margin: "8px 0 4px" }}>
        {t("settings.backup.intro")}
      </p>
      <Row label={t("settings.backup.includeKeys")} hint={t("settings.backup.includeKeysHint")} htmlFor="sw-bk-keys">
        <Switch id="sw-bk-keys" label={t("settings.backup.includeKeys")} checked={includeKeys} onChange={setIncludeKeys} />
      </Row>
      <Row label={t("settings.backup.actions")}>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-sm" onClick={() => void doExport()}>
            <Download size={13} /> {t("settings.backup.export")}
          </button>
          <button className="btn btn-sm" onClick={() => void doImport()}>
            <Upload size={13} /> {t("settings.backup.import")}
          </button>
        </div>
      </Row>
      {result && (
        <div className={`notice ${result.ok ? "info" : "err"}`} role="status" style={{ marginTop: 8 }}>
          {result.text}
        </div>
      )}
    </Card>
  );
}

export function AppearanceSection() {
  const [s, set] = useS();
  const g = s.general;
  return (
    <>
      <SectionHeader title={t("settings.sections.appearance")} />
      <Card>
        <Row label={t("settings.appearance.theme")}>
          <Segmented
            label={t("settings.appearance.theme")}
            value={g.theme}
            options={[
              { value: "system", label: t("settings.appearance.system") },
              { value: "light", label: t("settings.appearance.light") },
              { value: "dark", label: t("settings.appearance.dark") },
            ]}
            onChange={(v) => set((d) => void (d.general.theme = v))}
          />
        </Row>
        <Row label={t("settings.appearance.accent")}>
          <div className="swatches" role="radiogroup" aria-label={t("settings.appearance.accent")}>
            {(["teal", "blue", "green", "amber", "rose", "slate"] as const).map((c) => (
              <button
                key={c}
                type="button"
                role="radio"
                aria-checked={g.accent === c}
                aria-label={t(`settings.appearance.accents.${c}`)}
                title={t(`settings.appearance.accents.${c}`)}
                className={`swatch ${c}${g.accent === c ? " on" : ""}`}
                onClick={() => set((d) => void (d.general.accent = c))}
              />
            ))}
          </div>
        </Row>
        <Row label={t("settings.appearance.assistantName")} hint={t("settings.appearance.assistantNameHint")} htmlFor="in-name">
          <input
            id="in-name"
            className="input"
            style={{ maxWidth: 260 }}
            maxLength={40}
            defaultValue={g.assistantName}
            onBlur={(e) => {
              const v = e.target.value.trim() || "Local Assistant";
              if (v !== g.assistantName) set((d) => void (d.general.assistantName = v));
            }}
            onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          />
        </Row>
        <Row label={t("settings.appearance.highContrast")} htmlFor="sw-hc">
          <Switch id="sw-hc" label={t("settings.appearance.highContrast")} checked={g.highContrast} onChange={(v) => set((d) => void (d.general.highContrast = v))} />
        </Row>
        <Row label={t("settings.appearance.fontSize")} htmlFor="rng-font">
          <input id="rng-font" type="range" min={0.8} max={1.6} step={0.05} value={g.fontScale} onChange={(e) => set((d) => void (d.general.fontScale = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{Math.round(g.fontScale * 100)}%</span>
        </Row>
      </Card>
    </>
  );
}

export function ShortcutsSection() {
  const [s, set] = useS();
  const [errors, setErrors] = useState<string[]>([]);
  useEffect(() => {
    const id = setTimeout(() => void ipc.shortcutErrors().then(setErrors), 300);
    return () => clearTimeout(id);
  }, [s.general.globalShortcut, s.general.pushToTalkShortcut, s.dictation.shortcut, s.dictation.enabled, s.stt.pushToTalk]);
  return (
    <>
      <SectionHeader title={t("settings.sections.shortcuts")} intro={t("settings.shortcuts.hint")} />
      {errors.length > 0 && (
        <div className="notice warn" style={{ maxWidth: 820, marginBottom: 12 }} role="alert">
          <div>
            {t("settings.shortcuts.errors")}
            <ul style={{ margin: "4px 0 0", paddingInlineStart: 18 }}>
              {errors.map((e) => (
                <li key={e}>{e}</li>
              ))}
            </ul>
          </div>
        </div>
      )}
      <Card>
        <Row label={t("settings.shortcuts.openAssistant")}>
          <ShortcutInput label={t("settings.shortcuts.openAssistant")} value={s.general.globalShortcut} defaultValue="CommandOrControl+Space" onChange={(v) => set((d) => void (d.general.globalShortcut = v))} />
        </Row>
        <Row label={t("settings.shortcuts.pushToTalk")}>
          <ShortcutInput label={t("settings.shortcuts.pushToTalk")} value={s.general.pushToTalkShortcut} defaultValue="CommandOrControl+Shift+Space" onChange={(v) => set((d) => void (d.general.pushToTalkShortcut = v))} />
          <Switch label={t("settings.speech.pushToTalk")} checked={s.stt.pushToTalk} onChange={(v) => set((d) => void (d.stt.pushToTalk = v))} />
        </Row>
        <Row label={t("settings.shortcuts.dictation")}>
          <ShortcutInput label={t("settings.shortcuts.dictation")} value={s.dictation.shortcut} defaultValue="CommandOrControl+Alt+Space" onChange={(v) => set((d) => void (d.dictation.shortcut = v))} />
          <Switch label={t("settings.dictation.enable")} checked={s.dictation.enabled} onChange={(v) => set((d) => void (d.dictation.enabled = v))} />
        </Row>
      </Card>
    </>
  );
}
