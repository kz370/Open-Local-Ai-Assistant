import { useEffect, useState } from "react";
import { Copy, Trash2 } from "lucide-react";
import { ipc, on } from "../../../app/ipc";
import { modelLabel, t } from "../../../app/strings";
import type { DictationEntry, InstalledModel } from "../../../app/types";
import { Segmented, Switch, textDir } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { ShortcutInput } from "../../../components/settings/ShortcutInput";
import { useLmModels } from "./Ai";
import { useS } from "./Basic";

function HistoryCard({ enabled, onToggle }: { enabled: boolean; onToggle: (v: boolean) => void }) {
  const [items, setItems] = useState<DictationEntry[]>([]);
  const [copied, setCopied] = useState<string | null>(null);
  const load = () => void ipc.dictationHistory().then(setItems).catch(() => undefined);
  useEffect(() => {
    load();
    const sub = on("dictation://history", load);
    return () => void sub.then((u) => u());
  }, []);
  const copy = (e: DictationEntry) => {
    void navigator.clipboard.writeText(e.text).then(() => {
      setCopied(e.id);
      setTimeout(() => setCopied((c) => (c === e.id ? null : c)), 1500);
    });
  };
  const remove = (id: string) => void ipc.dictationHistoryDelete(id).then(load);
  const clear = () => void ipc.dictationHistoryClear().then(load);

  return (
    <Card
      title={t("settings.dictation.history")}
      actions={
        items.length > 0 && (
          <button className="btn btn-sm btn-danger" onClick={clear}>
            {t("settings.dictation.historyClear")}
          </button>
        )
      }
    >
      <Row label={t("settings.dictation.historyEnabled")} hint={t("settings.dictation.historyEnabledHint")} htmlFor="sw-hist">
        <Switch id="sw-hist" label={t("settings.dictation.historyEnabled")} checked={enabled} onChange={onToggle} />
      </Row>
      {items.length === 0 ? (
        <p className="settings-intro" style={{ margin: "8px 0 0" }}>
          {t("settings.dictation.historyEmpty")}
        </p>
      ) : (
        <div className="history-list">
          {items.map((e) => (
            <div key={e.id} className="history-item">
              <div className="history-body">
                <div className="history-text" dir={textDir(e.text)}>
                  {e.text}
                </div>
                <div className="history-meta">
                  <span>{new Date(e.createdAt).toLocaleString()}</span>
                  {!e.inserted && <span className="badge err">{t("settings.dictation.historyNotInserted")}</span>}
                </div>
              </div>
              <button className="icon-btn" title={copied === e.id ? t("settings.dictation.historyCopied") : t("settings.dictation.historyCopy")} aria-label={t("settings.dictation.historyCopy")} onClick={() => copy(e)}>
                <Copy size={14} />
              </button>
              <button className="icon-btn danger" title={t("settings.dictation.historyDelete")} aria-label={t("settings.dictation.historyDelete")} onClick={() => remove(e.id)}>
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}

export function DictationSection() {
  const [s, set] = useS();
  const d = s.dictation;
  const { models, error } = useLmModels();
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  useEffect(() => void ipc.modelsInstalled().then(setInstalled), []);
  const hasStt = installed.some((m) => m.kind === "stt");
  const chatModels = [...models.filter((m) => m.kind !== "embedding")].sort((a, b) => (a.sizeBytes ?? 0) - (b.sizeBytes ?? 0));

  return (
    <>
      <SectionHeader title={t("settings.sections.dictation")} intro={t("settings.dictation.intro")} />
      {!hasStt && (
        <div className="notice warn" style={{ maxWidth: 820, marginBottom: 12 }}>
          <span style={{ flex: 1 }}>{t("settings.dictation.requiresStt")}</span>
          <button className="btn btn-sm" onClick={() => (location.hash = "#/settings/speech")}>
            {t("voice.setupAction")}
          </button>
        </div>
      )}
      <Card>
        <Row label={t("settings.dictation.enable")} htmlFor="sw-dict">
          <Switch id="sw-dict" label={t("settings.dictation.enable")} checked={d.enabled} onChange={(v) => set((x) => void (x.dictation.enabled = v))} />
        </Row>
        <Row label={t("settings.dictation.shortcut")}>
          <ShortcutInput label={t("settings.dictation.shortcut")} value={d.shortcut} defaultValue="CommandOrControl+Alt+Space" onChange={(v) => set((x) => void (x.dictation.shortcut = v))} />
        </Row>
        <Row label={t("settings.dictation.mode")}>
          <Segmented
            label={t("settings.dictation.mode")}
            value={d.mode}
            options={[
              { value: "hold", label: t("settings.dictation.hold") },
              { value: "toggle", label: t("settings.dictation.toggle") },
            ]}
            onChange={(v) => set((x) => void (x.dictation.mode = v))}
          />
        </Row>
        <Row label={t("settings.dictation.insertMethod")}>
          <Segmented
            label={t("settings.dictation.insertMethod")}
            value={d.insertMethod}
            options={[
              { value: "type", label: t("settings.dictation.type") },
              { value: "paste", label: t("settings.dictation.paste") },
            ]}
            onChange={(v) => set((x) => void (x.dictation.insertMethod = v))}
          />
        </Row>
        <Row label={t("settings.dictation.trailingSpace")} htmlFor="sw-space">
          <Switch id="sw-space" label={t("settings.dictation.trailingSpace")} checked={d.addTrailingSpace} onChange={(v) => set((x) => void (x.dictation.addTrailingSpace = v))} />
        </Row>
        <Row label={t("settings.dictation.language")} hint={t("settings.dictation.languageHint")} htmlFor="sel-dict-lang">
          <select id="sel-dict-lang" className="select" value={d.language || s.stt.language} onChange={(e) => set((x) => void (x.dictation.language = e.target.value))}>
            <option value="auto">{t("app.automatic")}</option>
            {s.language.entries.map((e) => (
              <option key={e.code} value={e.code}>
                {e.builtIn ? t(`languages.${e.code}`) : e.displayName}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t("settings.dictation.review")} hint={t("settings.dictation.reviewHint")} htmlFor="sw-review">
          <Switch id="sw-review" label={t("settings.dictation.review")} checked={d.reviewBeforeInsert} onChange={(v) => set((x) => void (x.dictation.reviewBeforeInsert = v))} />
        </Row>
        <Row label={t("settings.speech.micOnly")} hint={t("settings.speech.micOnlyHint")} htmlFor="sw-miconly-dict">
          <Switch id="sw-miconly-dict" label={t("settings.speech.micOnly")} checked={s.stt.micOnly} onChange={(v) => set((x) => void (x.stt.micOnly = v))} />
        </Row>
        <Row label={t("settings.speech.isolateSystemAudio")} hint={t("settings.speech.isolateSystemAudioHint")} htmlFor="sw-isolate-dict">
          <Switch id="sw-isolate-dict" label={t("settings.speech.isolateSystemAudio")} checked={s.stt.isolateSystemAudio} onChange={(v) => set((x) => void (x.stt.isolateSystemAudio = v))} />
        </Row>
        <Row label={t("settings.dictation.overlayPosition")} hint={t("settings.dictation.overlayPositionHint")}>
          <button className="btn btn-sm" onClick={() => void ipc.dictationResetOverlayPosition()}>
            {t("overlay.resetPosition")}
          </button>
        </Row>
      </Card>
      <Card title={t("settings.dictation.correction")}>
        <Row label={t("settings.dictation.correction")} hint={t("settings.dictation.correctionHint")} htmlFor="sw-corr">
          <Switch id="sw-corr" label={t("settings.dictation.correction")} checked={d.correctionEnabled} onChange={(v) => set((x) => void (x.dictation.correctionEnabled = v))} />
        </Row>
        <Row label={t("settings.dictation.correctionModel")} htmlFor="sel-corr">
          <select id="sel-corr" className="select" value={d.correctionModel ?? ""} disabled={!d.correctionEnabled} onChange={(e) => set((x) => void (x.dictation.correctionModel = e.target.value || null))}>
            <option value="">{t("settings.dictation.chooseModel")}</option>
            {chatModels.map((m) => (
              <option key={m.id} value={m.id}>
                {modelLabel(m.id, s.ai.modelAliases, 48)}
                {m.params ? ` (${m.params})` : ""}
                {m.loaded ? " • loaded" : ""}
              </option>
            ))}
          </select>
          {error && <span className="badge err">{t("status.disconnected")}</span>}
        </Row>
      </Card>
      <HistoryCard enabled={d.historyEnabled} onToggle={(v) => set((x) => void (x.dictation.historyEnabled = v))} />
    </>
  );
}
