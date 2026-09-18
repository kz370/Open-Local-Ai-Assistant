import { useEffect, useState } from "react";
import { Play, Square } from "lucide-react";
import { WHISPER_LANGUAGES } from "../../../app/captionLanguages";
import { ipc, on, toAppError } from "../../../app/ipc";
import { t } from "../../../app/strings";
import type { AppErrorPayload, AudioDevice, CaptionEvent, InstalledModel } from "../../../app/types";
import { ErrorNotice, Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { ShortcutInput } from "../../../components/settings/ShortcutInput";
import { captionStyle } from "../../Captions/Captions";
import { useS } from "./Basic";

const WEIGHTS = [300, 400, 500, 600, 700, 900];
const STYLE_DEFAULTS = { fontSize: 28, fontWeight: 600, italic: false, textColor: "#ffffff", backgroundColor: "#000000", backgroundOpacity: 0.6, maxLines: 2 };

export function CaptionsSection() {
  const [s, set] = useS();
  const c = s.captions;
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  const [outputs, setOutputs] = useState<AudioDevice[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<AppErrorPayload | null>(null);

  useEffect(() => {
    void ipc.modelsInstalled().then(setInstalled);
    void ipc.audioDevices().then((d) => setOutputs(d.outputs));
    void ipc.captionsStatus().then(setRunning);
    const sub = on<CaptionEvent>("captions://event", (e) => {
      if (e.type === "state") setRunning(e.state !== "idle");
      if (e.type === "state" && e.state === "starting") setError(null);
      if (e.type === "error") setError({ code: e.code, detail: e.detail });
    });
    return () => void sub.then((u) => u());
  }, []);

  const sttModels = installed.filter((m) => m.kind === "stt");
  const ready = sttModels.length > 0 && installed.some((m) => m.kind === "vad" || m.streaming);

  const toggle = async () => {
    setError(null);
    try {
      if (running) {
        await ipc.captionsStop();
        setRunning(false);
      } else {
        await ipc.captionsStart();
        setRunning(true);
      }
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <>
      <SectionHeader title={t("settings.sections.captions")} intro={t("settings.captions.intro")} />
      {!ready && (
        <div className="notice warn" style={{ maxWidth: 820, marginBottom: 12 }}>
          <span style={{ flex: 1 }}>{t("settings.captions.requiresStt")}</span>
          <button className="btn btn-sm" onClick={() => (location.hash = "#/settings/speech")}>
            {t("voice.setupAction")}
          </button>
        </div>
      )}
      {error && (
        <div style={{ maxWidth: 820, marginBottom: 12 }}>
          <ErrorNotice error={error} />
        </div>
      )}
      <Card>
        <Row label={t("settings.captions.control")}>
          <button type="button" className={`btn${running ? "" : " btn-primary"}`} onClick={() => void toggle()}>
            {running ? <Square size={14} /> : <Play size={14} />}
            {running ? t("settings.captions.stop") : t("settings.captions.start")}
          </button>
          <span className={`badge${running ? " ok" : ""}`}>{running ? t("settings.captions.running") : t("settings.captions.stopped")}</span>
        </Row>
        <Row label={t("settings.captions.shortcut")} hint={t("settings.captions.shortcutHint")}>
          <ShortcutInput label={t("settings.captions.shortcut")} value={c.shortcut} defaultValue="CommandOrControl+Alt+C" onChange={(v) => set((d) => void (d.captions.shortcut = v))} />
        </Row>
        <Row label={t("settings.captions.language")} hint={t("settings.captions.languageHint")} htmlFor="sel-cap-lang">
          <select id="sel-cap-lang" className="select" value={c.language} onChange={(e) => set((d) => void (d.captions.language = e.target.value))}>
            <option value="auto">{t("settings.captions.languageAuto")}</option>
            {WHISPER_LANGUAGES.map(([code, name]) => (
              <option key={code} value={code}>
                {name}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t("settings.captions.translate")} hint={t("settings.captions.translateHint")} htmlFor="sw-cap-tr">
          <Switch id="sw-cap-tr" label={t("settings.captions.translate")} checked={c.translate} onChange={(v) => set((d) => void (d.captions.translate = v))} />
        </Row>
        <Row label={t("settings.captions.model")} hint={t("settings.captions.modelHint")} htmlFor="sel-cap-model">
          <select id="sel-cap-model" className="select" value={c.model} onChange={(e) => set((d) => void (d.captions.model = e.target.value))}>
            <option value="auto">{t("settings.captions.modelAuto")}</option>
            {sttModels.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t("settings.captions.source")} htmlFor="sel-cap-src">
          <select id="sel-cap-src" className="select" value={c.audioSource ?? ""} onChange={(e) => set((d) => void (d.captions.audioSource = e.target.value || null))}>
            <option value="">{t("settings.captions.sourceDefault")}</option>
            {outputs.map((o) => (
              <option key={o.id} value={o.id}>
                {o.name}
              </option>
            ))}
          </select>
        </Row>
      </Card>
      <Card
        title={t("settings.captions.style")}
        actions={
          <button type="button" className="btn btn-sm" onClick={() => set((d) => void Object.assign(d.captions, STYLE_DEFAULTS))}>
            {t("settings.captions.resetStyle")}
          </button>
        }
      >
        <Row label={t("settings.captions.fontSize")} htmlFor="rng-cap-size">
          <input id="rng-cap-size" type="range" min={14} max={72} step={1} value={c.fontSize} onChange={(e) => set((d) => void (d.captions.fontSize = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{c.fontSize} px</span>
        </Row>
        <Row label={t("settings.captions.fontWeight")} htmlFor="sel-cap-weight">
          <select id="sel-cap-weight" className="select" value={c.fontWeight} onChange={(e) => set((d) => void (d.captions.fontWeight = Number(e.target.value)))}>
            {WEIGHTS.map((w) => (
              <option key={w} value={w}>
                {t(`settings.captions.weights.${w}`)}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t("settings.captions.italic")} htmlFor="sw-cap-italic">
          <Switch id="sw-cap-italic" label={t("settings.captions.italic")} checked={c.italic} onChange={(v) => set((d) => void (d.captions.italic = v))} />
        </Row>
        <Row label={t("settings.captions.textColor")} htmlFor="col-cap-text">
          <input id="col-cap-text" type="color" className="color-input" value={c.textColor} onChange={(e) => set((d) => void (d.captions.textColor = e.target.value))} />
          <span className="range-value">{c.textColor}</span>
        </Row>
        <Row label={t("settings.captions.backgroundColor")} htmlFor="col-cap-bg">
          <input id="col-cap-bg" type="color" className="color-input" value={c.backgroundColor} onChange={(e) => set((d) => void (d.captions.backgroundColor = e.target.value))} />
          <span className="range-value">{c.backgroundColor}</span>
        </Row>
        <Row label={t("settings.captions.backgroundOpacity")} htmlFor="rng-cap-op">
          <input id="rng-cap-op" type="range" min={0} max={1} step={0.05} value={c.backgroundOpacity} onChange={(e) => set((d) => void (d.captions.backgroundOpacity = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{Math.round(c.backgroundOpacity * 100)}%</span>
        </Row>
        <Row label={t("settings.captions.maxLines")} htmlFor="rng-cap-lines">
          <input id="rng-cap-lines" type="range" min={1} max={6} step={1} value={c.maxLines} onChange={(e) => set((d) => void (d.captions.maxLines = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{c.maxLines}</span>
        </Row>
        <Row label={t("settings.captions.preview")} stack>
          <div className="caption-preview-stage" style={{ width: "100%" }}>
            <div className="caption-preview" style={captionStyle(c)}>
              {t("settings.captions.previewText")}
            </div>
          </div>
        </Row>
      </Card>
    </>
  );
}
