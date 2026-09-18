import { useEffect, useState } from "react";
import { ipc } from "../../../app/ipc";
import { modelLabel, t } from "../../../app/strings";
import type { InstalledModel } from "../../../app/types";
import { Segmented, Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { ShortcutInput } from "../../../components/settings/ShortcutInput";
import { useLmModels } from "./Ai";
import { useS } from "./Basic";

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
        <Row label={t("settings.speech.micOnly")} hint={t("settings.speech.micOnlyHint")} htmlFor="sw-miconly-dict">
          <Switch id="sw-miconly-dict" label={t("settings.speech.micOnly")} checked={s.stt.micOnly} onChange={(v) => set((x) => void (x.stt.micOnly = v))} />
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
    </>
  );
}
