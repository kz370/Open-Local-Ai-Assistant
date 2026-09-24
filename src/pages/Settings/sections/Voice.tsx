import { useEffect, useState } from "react";
import { ExternalLink, FolderOpen, FolderPlus, Plus, Play, Square, Trash2 } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ipc, on, toAppError } from "../../../app/ipc";
import { formatBytes, t } from "../../../app/strings";
import type { AppErrorPayload, AppInfo, AudioDevice, GpuStatus, IncompatibleModel, InstalledModel, LanguageEntry, VoiceEvent, VoiceInfo } from "../../../app/types";
import { ErrorNotice, openExternal, Segmented, Switch } from "../../../components/common/controls";
import { GpuCard } from "../../../components/settings/GpuCard";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { ModelManager } from "../../../components/settings/ModelManager";
import { LevelMeter } from "../../../components/voice/LevelMeter";
import { useS } from "./Basic";

function useDevices() {
  const [devices, setDevices] = useState<{ inputs: AudioDevice[]; outputs: AudioDevice[] }>({ inputs: [], outputs: [] });
  useEffect(() => {
    void ipc.audioDevices().then(setDevices);
  }, []);
  return devices;
}

function useInstalled() {
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  useEffect(() => {
    const load = () => void ipc.modelsInstalled().then(setInstalled);
    load();
    const sub = on("models://changed", load);
    return () => void sub.then((u) => u());
  }, []);
  return installed;
}

function MicTest({ microphone }: { microphone: string | null }) {
  const [testing, setTesting] = useState(false);
  const [levels, setLevels] = useState<number[]>(new Array(32).fill(0));
  const [device, setDevice] = useState<string | null>(null);
  const [error, setError] = useState<AppErrorPayload | null>(null);

  useEffect(() => {
    const sub = on<VoiceEvent>("voice://event", (ev) => {
      if (ev.mode !== "test") return;
      if (ev.type === "level") setLevels((l) => [...l.slice(1), ev.value]);
      if (ev.type === "state") {
        if (ev.state === "idle") setTesting(false);
        if (ev.device) setDevice(ev.device);
      }
    });
    return () => {
      void sub.then((u) => u());
      void ipc.voiceStatus().then((m) => { if (m === "test") void ipc.voiceStop(true); });
    };
  }, []);

  useEffect(() => {
    if (testing) void ipc.voiceStop(true).then(() => setTesting(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [microphone]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6, width: "100%" }}>
      <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
        <button
          className="btn btn-sm"
          onClick={async () => {
            setError(null);
            if (testing) {
              await ipc.voiceStop(true);
              setTesting(false);
            } else {
              try {
                await ipc.voiceStart("test");
                setTesting(true);
              } catch (e) {
                setError(toAppError(e));
              }
            }
          }}
        >
          {testing ? <Square size={12} /> : <Play size={12} />} {testing ? t("settings.speech.stopTest") : t("settings.speech.testMic")}
        </button>
        {testing && <LevelMeter levels={levels} max={24} label={t("voice.level")} />}
        {testing && device && <span className="row-hint">{device}</span>}
      </div>
      {error && <ErrorNotice error={error} />}
    </div>
  );
}

/** Shows where models are loaded from and lets the user add their own folders. */
function ModelFolders({ installed }: { installed: InstalledModel[] }) {
  const [s, set] = useS();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [unusable, setUnusable] = useState<IncompatibleModel[]>([]);
  useEffect(() => void ipc.appInfo().then(setInfo), []);
  useEffect(() => {
    void ipc.modelsIncompatible().then((x) => setUnusable(x ?? []));
  }, [s.stt.extraModelDirs.join("|")]);

  const addFolder = async () => {
    const picked = await openDialog({ directory: true, multiple: false, title: t("settings.speech.addFolder") });
    if (!picked || Array.isArray(picked)) return;
    if (s.stt.extraModelDirs.includes(picked)) return;
    set((d) => void d.stt.extraModelDirs.push(picked));
  };

  return (
    <Card
      title={t("settings.speech.folders")}
      actions={
        <>
          <button className="btn btn-sm" onClick={() => void ipc.openFolder("models")}>
            <FolderOpen size={13} /> {t("settings.models.openFolder")}
          </button>
          <button className="btn btn-sm btn-primary" onClick={() => void addFolder()}>
            <FolderPlus size={13} /> {t("settings.speech.addFolder")}
          </button>
        </>
      }
    >
      <p className="row-hint" style={{ margin: "10px 0 6px" }}>
        {t("settings.speech.foldersHint")}
      </p>
      <div className="folder-list">
        <div className="folder-item">
          <span className="badge">{t("settings.speech.appFolder")}</span>
          <span className="folder-path" title={info?.paths.modelsDir}>
            {info?.paths.modelsDir ?? "…"}
          </span>
          <span />
        </div>
        {s.stt.extraModelDirs.length === 0 && <p className="row-hint">{t("settings.speech.noCustom")}</p>}
        {s.stt.extraModelDirs.map((dir) => (
          <div className="folder-item" key={dir}>
            <span className="badge">{t("settings.models.custom")}</span>
            <span className="folder-path" title={dir}>
              {dir}
            </span>
            <button
              className="icon-btn danger"
              aria-label={`${t("settings.speech.removeFolder")}: ${dir}`}
              title={t("settings.speech.removeFolder")}
              onClick={() => set((d) => void (d.stt.extraModelDirs = d.stt.extraModelDirs.filter((x) => x !== dir)))}
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}
      </div>
      {unusable.length > 0 && (
        <div className="row stack">
          <span className="row-label">
            {t("settings.speech.unsupported")}
            <span className="row-hint">{t("settings.speech.unsupportedHint")}</span>
          </span>
          <div className="folder-list">
            {unusable.map((m) => (
              <div className="folder-item" key={m.path}>
                <span className="badge warn">{m.name}</span>
                <span className="folder-path" title={m.path}>
                  {m.path}
                </span>
                <span className="row-hint" style={{ margin: 0, whiteSpace: "nowrap" }}>
                  {m.reason}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
      <div className="row stack">
        <span className="row-label">{t("settings.speech.detected")}</span>
        <div className="model-list">
          {installed.length === 0 && <p className="row-hint">{t("settings.speech.noModel")}</p>}
          {installed.map((m) => (
            <div className="model-item" key={m.path}>
              <span className="badge">{m.kind.toUpperCase()}</span>
              <div style={{ minWidth: 0 }}>
                <div className="model-name">{m.name}</div>
                <div className="model-meta">
                  {m.family && <span>{m.family}</span>}
                  {m.streaming && <span className="badge accent">{t("settings.speech.streaming")}</span>}
                  {m.gender && <span className="badge">{t(`settings.voice.gender${m.gender === "female" ? "Female" : m.gender === "male" ? "Male" : "Any"}`)}</span>}
                  {m.sizeBytes > 0 && <span>{formatBytes(m.sizeBytes)}</span>}
                </div>
                <div className="folder-path" title={m.path}>
                  {t("settings.speech.loadedFrom")}: {m.path}
                </div>
              </div>
              <span />
            </div>
          ))}
        </div>
      </div>
    </Card>
  );
}

function useGpuStatus() {
  const [status, setStatus] = useState<GpuStatus | null>(null);
  useEffect(() => {
    const refresh = () => void ipc.gpuStatus().then(setStatus);
    refresh();
    const sub = on("gpu://changed", refresh);
    return () => void sub.then((u) => u());
  }, []);
  return status;
}

function languageLabel(e: LanguageEntry) {
  return e.builtIn ? t(`languages.${e.code}`) : e.displayName;
}

export function SpeechSection() {
  const [s, set] = useS();
  const devices = useDevices();
  const installed = useInstalled();
  const gpu = useGpuStatus();
  const sttModels = installed.filter((m) => m.kind === "stt");
  return (
    <>
      <SectionHeader title={t("settings.sections.speech")} />
      <Card>
        <Row label={t("settings.speech.provider")}>
          <span className="badge ok">{t("settings.speech.local")}</span>
        </Row>
        <Row label={t("settings.speech.model")} htmlFor="sel-stt">
          <select id="sel-stt" className="select" value={s.stt.model} onChange={(e) => set((d) => void (d.stt.model = e.target.value))}>
            <option value="auto">{t("app.automatic")}</option>
            {sttModels.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
                {m.family ? ` — ${m.family}` : ""}
                {m.streaming ? ` (${t("settings.speech.streaming")})` : ""}
              </option>
            ))}
          </select>
          {!sttModels.length && <span className="badge warn">{t("settings.speech.noModel")}</span>}
        </Row>
        <Row label={t("settings.speech.microphone")} htmlFor="sel-mic">
          <select id="sel-mic" className="select" value={s.stt.microphone ?? ""} onChange={(e) => set((d) => void (d.stt.microphone = e.target.value || null))}>
            <option value="">{t("app.automatic")}</option>
            {devices.inputs.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t("settings.speech.testMic")}>
          <MicTest microphone={s.stt.microphone} />
        </Row>
        <Row label={t("settings.speech.hardware")}>
          {gpu?.supported ? (
            <select id="sel-stt-hw" className="select" value={s.stt.hardware} onChange={(e) => set((d) => void (d.stt.hardware = e.target.value as "auto" | "cpu"))}>
              <option value="auto">{t("app.automatic")}</option>
              <option value="cpu">{t("settings.speech.cpu")}</option>
            </select>
          ) : (
            <span className="badge">
              {t("app.automatic")} · {t("settings.speech.cpu")}
            </span>
          )}
        </Row>
        <Row label={t("settings.speech.autoSubmit")} hint={t("settings.speech.autoSubmitHint")} htmlFor="sw-auto">
          <Switch id="sw-auto" label={t("settings.speech.autoSubmit")} checked={s.stt.autoSubmit} onChange={(v) => set((d) => void (d.stt.autoSubmit = v))} />
        </Row>
        <Row label={t("settings.speech.pushToTalk")} htmlFor="sw-ptt">
          <Switch id="sw-ptt" label={t("settings.speech.pushToTalk")} checked={s.stt.pushToTalk} onChange={(v) => set((d) => void (d.stt.pushToTalk = v))} />
        </Row>
      </Card>
      <Card title={t("chat.handsFree")}>
        <Row label={t("settings.speech.callView")} hint={t("settings.speech.callViewHint")} htmlFor="sw-call">
          <Switch id="sw-call" label={t("settings.speech.callView")} checked={s.stt.callView} onChange={(v) => set((d) => void (d.stt.callView = v))} />
        </Row>
        <Row label={t("settings.speech.vadThreshold")} htmlFor="rng-vad">
          <input id="rng-vad" type="range" min={0.2} max={0.9} step={0.05} value={s.stt.vadThreshold} onChange={(e) => set((d) => void (d.stt.vadThreshold = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{s.stt.vadThreshold.toFixed(2)}</span>
        </Row>
        <Row label={t("settings.speech.silence")} htmlFor="rng-sil">
          <input id="rng-sil" type="range" min={300} max={2500} step={100} value={s.stt.silenceMs} onChange={(e) => set((d) => void (d.stt.silenceMs = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{s.stt.silenceMs} ms</span>
        </Row>
        <Row label={t("settings.speech.autoStop")} hint={t("settings.speech.autoStopHint")} htmlFor="rng-autostop">
          <input id="rng-autostop" type="range" min={0} max={60} step={1} value={s.stt.autoStopSilenceSecs} onChange={(e) => set((d) => void (d.stt.autoStopSilenceSecs = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{s.stt.autoStopSilenceSecs === 0 ? t("settings.speech.never") : t("settings.speech.seconds", { value: s.stt.autoStopSilenceSecs })}</span>
        </Row>
        <Row label={t("settings.speech.handsFreeTimeout")} htmlFor="rng-hftimeout">
          <input id="rng-hftimeout" type="range" min={0} max={900} step={15} value={s.stt.handsFreeTimeoutSecs} onChange={(e) => set((d) => void (d.stt.handsFreeTimeoutSecs = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{s.stt.handsFreeTimeoutSecs === 0 ? t("settings.speech.never") : t("settings.speech.seconds", { value: s.stt.handsFreeTimeoutSecs })}</span>
        </Row>
      </Card>
      <ModelManager kinds={["stt", "vad"]} title={t("settings.speech.models")} />
      <ModelFolders installed={installed} />
    </>
  );
}

function useVoices() {
  const [voices, setVoices] = useState<VoiceInfo[]>([]);
  useEffect(() => {
    const load = () => void ipc.ttsVoices().then(setVoices);
    load();
    const subs = [on("models://changed", load)];
    return () => subs.forEach((s) => void s.then((u) => u()));
  }, []);
  return voices;
}

const VOICE_LIST_URL = "https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/index.html";
const VOICE_DOWNLOAD_URL = "https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models";

/** Shown for a language the app ships no voice for: where to get one and how to add it. */
function NoVoiceHelp({ entry }: { entry: LanguageEntry }) {
  const [, set] = useS();
  const name = entry.displayName || entry.code;
  const addFolder = async () => {
    const picked = await openDialog({ directory: true, multiple: false, title: t("settings.speech.addFolder") });
    if (!picked || Array.isArray(picked)) return;
    set((d) => void (d.stt.extraModelDirs.includes(picked) || d.stt.extraModelDirs.push(picked)));
  };
  return (
    <div className="notice info" style={{ maxWidth: 820, margin: "8px 0", flexDirection: "column", alignItems: "stretch", gap: 6 }}>
      <strong>{t("settings.voice.noVoiceTitle", { language: name })}</strong>
      <ol style={{ margin: 0, paddingInlineStart: 20, display: "grid", gap: 2 }}>
        <li>{t("settings.voice.noVoiceStep1", { code: entry.code })}</li>
        <li>{t("settings.voice.noVoiceStep2")}</li>
        <li>{t("settings.voice.noVoiceStep3")}</li>
      </ol>
      <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        <button className="btn btn-sm" onClick={() => openExternal(VOICE_LIST_URL)}>
          <ExternalLink size={12} /> {t("settings.voice.noVoiceList")}
        </button>
        <button className="btn btn-sm" onClick={() => openExternal(VOICE_DOWNLOAD_URL)}>
          <ExternalLink size={12} /> {t("settings.voice.noVoiceDownloads")}
        </button>
        <button className="btn btn-sm btn-primary" onClick={() => void addFolder()}>
          <FolderPlus size={12} /> {t("settings.speech.addFolder")}
        </button>
      </div>
    </div>
  );
}

export function VoiceSection() {
  const [s, set] = useS();
  const devices = useDevices();
  const voices = useVoices();
  const gpu = useGpuStatus();
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const [activeLang, setActiveLang] = useState(s.language.entries[0]?.code ?? "en");

  useEffect(() => {
    if (!s.language.entries.some((e) => e.code === activeLang) && s.language.entries[0]) {
      setActiveLang(s.language.entries[0].code);
    }
  }, [s.language.entries, activeLang]);

  const entry = s.language.entries.find((e) => e.code === activeLang) ?? s.language.entries[0];
  // "*" = a multilingual voice. Speech only routes the built-in languages.
  const list = voices.filter((v) => v.language === entry?.code || (entry?.builtIn && v.language === "*"));

  const updateEntry = (code: string, patch: Partial<LanguageEntry>) =>
    set((d) => {
      const target = d.language.entries.find((e) => e.code === code);
      if (target) Object.assign(target, patch);
    });

  return (
    <>
      <SectionHeader title={t("settings.sections.voice")} />
      <GpuCard />
      <Card>
        <Row label={t("settings.voice.provider")}>
          <span className="badge ok">{t("settings.speech.local")}</span>
        </Row>
        <Row label={t("settings.voice.speakResponses")} htmlFor="sw-speak">
          <Switch id="sw-speak" label={t("settings.voice.speakResponses")} checked={s.tts.speakResponses} onChange={(v) => set((d) => void (d.tts.speakResponses = v))} />
        </Row>
        <Row label={t("settings.voice.speakAfterReply")} hint={t("settings.voice.speakAfterReplyHint")} htmlFor="sw-after-reply">
          <Switch id="sw-after-reply" label={t("settings.voice.speakAfterReply")} checked={s.tts.speakAfterReply} onChange={(v) => set((d) => void (d.tts.speakAfterReply = v))} />
        </Row>
        <Row label={t("settings.voice.gender")} hint={t("settings.voice.genderHint")}>
          <Segmented
            label={t("settings.voice.gender")}
            value={s.tts.preferredGender}
            options={[
              { value: "any" as const, label: t("settings.voice.genderAny") },
              { value: "female" as const, label: t("settings.voice.genderFemale") },
              { value: "male" as const, label: t("settings.voice.genderMale") },
            ]}
            onChange={(v) => set((d) => void (d.tts.preferredGender = v))}
          />
        </Row>
        <Row label={t("settings.voice.language")}>
          <Segmented label={t("settings.voice.language")} value={activeLang} options={s.language.entries.map((e) => ({ value: e.code, label: languageLabel(e) }))} onChange={setActiveLang} />
          <button className="btn btn-sm" onClick={() => (location.hash = "#/settings/language")}>
            <Plus size={12} /> {t("settings.voice.manageLanguages")}
          </button>
        </Row>
        {entry && !entry.builtIn && <p className="row-hint">{t("settings.voice.unsupportedLanguageHint")}</p>}
        {entry && !entry.builtIn && list.length === 0 && <NoVoiceHelp entry={entry} />}
        {entry && (
          <Row label={t("settings.voice.voiceFor", { language: languageLabel(entry) })} htmlFor="sel-voice">
            <select id="sel-voice" className="select" value={entry.ttsVoice} onChange={(e) => updateEntry(entry.code, { ttsVoice: e.target.value })} disabled={!list.length} style={{ maxWidth: 260 }}>
              <option value="auto">{list.length ? t("app.automatic") : t("settings.voice.none")}</option>
              {list.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                  {v.gender && !v.name.toLowerCase().includes(v.gender) ? ` — ${t(`settings.voice.gender${v.gender === "female" ? "Female" : "Male"}`)}` : ""}
                </option>
              ))}
            </select>
            {gpu?.supported && entry.ttsVoice !== "auto" && (
              <select
                className="select"
                aria-label={t("settings.speech.hardware")}
                title={t("settings.speech.hardware")}
                value={s.tts.voiceHardware[entry.ttsVoice] ?? "auto"}
                onChange={(e) =>
                  set((d) => {
                    if (e.target.value === "auto") delete d.tts.voiceHardware[entry.ttsVoice];
                    else d.tts.voiceHardware[entry.ttsVoice] = e.target.value;
                  })
                }
                style={{ maxWidth: 110 }}
              >
                <option value="auto">{t("app.automatic")}</option>
                <option value="cpu">{t("settings.speech.cpu")}</option>
              </select>
            )}
            <button
              className="btn btn-sm"
              disabled={!list.length}
              onClick={() => {
                setError(null);
                void ipc.ttsTest(entry.code).catch((e) => setError(toAppError(e)));
              }}
            >
              <Play size={12} /> {t("settings.voice.testVoice")}
            </button>
          </Row>
        )}
        <Row label={t("settings.voice.speed")} htmlFor="rng-speed">
          <input id="rng-speed" type="range" min={0.5} max={2} step={0.05} value={s.tts.speed} onChange={(e) => set((d) => void (d.tts.speed = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{s.tts.speed.toFixed(2)}x</span>
        </Row>
        <Row label={t("settings.voice.volume")} htmlFor="rng-vol">
          <input id="rng-vol" type="range" min={0} max={1} step={0.05} value={s.tts.volume} onChange={(e) => set((d) => void (d.tts.volume = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{Math.round(s.tts.volume * 100)}%</span>
        </Row>
        <Row label={t("settings.voice.output")} htmlFor="sel-out">
          <select id="sel-out" className="select" value={s.tts.outputDevice ?? ""} onChange={(e) => set((d) => void (d.tts.outputDevice = e.target.value || null))}>
            <option value="">{t("app.automatic")}</option>
            {devices.outputs.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>
          <button className="btn btn-sm" onClick={() => void ipc.ttsStop()}>
            <Square size={12} /> {t("settings.voice.stop")}
          </button>
        </Row>
        {error && (
          <div style={{ paddingBottom: 12 }}>
            <ErrorNotice error={error} />
          </div>
        )}
      </Card>
      <ModelManager kinds={["tts"]} title={t("settings.voice.models")} languageFilter={entry?.code} />
      <p className="settings-intro">{t("settings.voice.localHint")}</p>
    </>
  );
}
